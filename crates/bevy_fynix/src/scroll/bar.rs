//! The bars of a scroll area.
//!
//! A bar has no `Node`: it is placed after the layout, against the
//! area and not the content scrolling in it.

use bevy::app::{App, PostUpdate};
use bevy::camera::visibility::Visibility;
use bevy::color::{Alpha, Color};
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::{Affine2, Vec2};
use bevy::picking::events::{Drag, DragEnd, Pointer, Press};
use bevy::picking::hover::Hovered;
use bevy::ui::{
    BackgroundColor, BorderRadius, ComputedNode,
    ComputedUiRenderTargetInfo, ComputedUiTargetCamera, FocusPolicy,
    Node, OverflowAxis, OverrideClip, ScrollPosition,
    UiGlobalTransform, UiSystems, UiTransform, ZIndex,
    ui_layout_system,
};

use super::{ScrollGoal, overflow};
use crate::backend::Kept;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ScrollbarStyle>()
        .add_observer(add_thumbs)
        .add_observer(on_thumb_press)
        .add_observer(on_thumb_drag)
        .add_observer(on_thumb_drag_end)
        .add_systems(
            PostUpdate,
            (
                keep_room.in_set(UiSystems::Prepare),
                place_thumbs
                    .in_set(UiSystems::Layout)
                    .after(ui_layout_system),
            ),
        );
}

/// When a scroll area shows its bars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    /// Never. The wheel still scrolls.
    Hidden,
    /// While there is something to scroll that way.
    #[default]
    WhenNeeded,
    Always,
}

/// On a scroll area: the style of its own bars, in place of the
/// [`ScrollbarStyle`] every other area shares.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct OwnScrollbar(pub ScrollbarStyle);

/// How the bars of scroll areas look and where they go.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub visibility: ScrollbarVisibility,
    /// Drawn over the content, taking no room. Otherwise the area
    /// keeps room for a bar along each edge it scrolls by, needed or
    /// not, so the content does not shift when one appears.
    pub floating: bool,
    /// A bar's thickness.
    pub width: f32,
    /// The gap around a bar: to the edge of the area, and at each
    /// end of its track.
    pub inset: f32,
    /// The shortest a bar gets, however much there is to scroll.
    pub min_length: f32,
    pub rest: Color,
    /// A bar under the pointer, or dragged.
    pub hot: Color,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            visibility: ScrollbarVisibility::WhenNeeded,
            floating: true,
            width: 6.0,
            inset: 2.0,
            min_length: 24.0,
            rest: Color::WHITE.with_alpha(0.16),
            hot: Color::WHITE.with_alpha(0.4),
        }
    }
}

impl ScrollbarStyle {
    /// The style of an area: its `own` when it has one.
    fn or<'a>(&'a self, own: Option<&'a OwnScrollbar>) -> &'a Self {
        own.map_or(self, |own| &own.0)
    }

    /// The room an area keeps along an edge for a bar.
    fn room(&self) -> f32 {
        let shown = self.visibility != ScrollbarVisibility::Hidden;
        if shown && !self.floating {
            self.width + 2.0 * self.inset
        } else {
            0.0
        }
    }
}

/// The way a bar scrolls its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    Across,
    Down,
}

impl Way {
    /// `along` this way and `beside` it, as a vector.
    fn vec(self, along: f32, beside: f32) -> Vec2 {
        match self {
            Self::Across => Vec2::new(along, beside),
            Self::Down => Vec2::new(beside, along),
        }
    }

    /// The part of `vec` along this way.
    fn of(self, vec: Vec2) -> f32 {
        match self {
            Self::Across => vec.x,
            Self::Down => vec.y,
        }
    }
}

/// A bar of the scroll area it is a child of.
#[derive(Component)]
#[require(
    Kept,
    ComputedNode,
    ComputedUiTargetCamera,
    ComputedUiRenderTargetInfo,
    UiTransform,
    BackgroundColor,
    FocusPolicy::Block,
    Visibility,
    ZIndex(1),
    OverrideClip,
    Hovered
)]
struct ScrollThumb {
    way: Way,
    dragged: bool,
}

fn add_thumbs(added: On<Add, ScrollGoal>, mut commands: Commands) {
    for way in [Way::Across, Way::Down] {
        commands.spawn((
            ScrollThumb {
                way,
                dragged: false,
            },
            ChildOf(added.entity),
        ));
    }
}

/// Keeps room in each area for the bars that take some.
fn keep_room(
    style: Res<ScrollbarStyle>,
    mut areas: Query<
        (&mut Node, Option<&OwnScrollbar>),
        With<ScrollGoal>,
    >,
) {
    for (mut node, own) in &mut areas {
        let room = style.or(own).room();
        if node.scrollbar_width != room {
            node.scrollbar_width = room;
        }
    }
}

/// Where a bar sits along its area, in the pixels of the area's
/// [`ComputedNode`].
#[derive(Debug, PartialEq)]
struct Bar {
    length: f32,
    /// From the start of the area to the start of the bar.
    at: f32,
    /// How far the content scrolls as the bar moves a pixel.
    gearing: f32,
}

impl Bar {
    /// `None` when the area shows no bar this way.
    fn of(
        node: &Node,
        area: &ComputedNode,
        style: &ScrollbarStyle,
        way: Way,
    ) -> Option<Self> {
        let scrolls = OverflowAxis::Scroll
            == match way {
                Way::Across => node.overflow.x,
                Way::Down => node.overflow.y,
            };
        // What the content has to itself, without the room kept for
        // the other bar.
        let visible = way.of(area.size() - area.scrollbar_size);
        let range = (way.of(area.content_size()) - visible).max(0.0);
        let shown = match style.visibility {
            ScrollbarVisibility::Hidden => false,
            ScrollbarVisibility::WhenNeeded => range >= 1.0,
            ScrollbarVisibility::Always => true,
        };
        let scale = area.inverse_scale_factor().recip();
        let inset = style.inset * scale;
        let track = visible - 2.0 * inset;
        if !(scrolls && shown && track > 0.0 && style.width > 0.0) {
            return None;
        }
        let length = (track * visible / (visible + range))
            .max(style.min_length * scale)
            .min(track);
        let travel = track - length;
        let (at, gearing) = if range > 0.0 && travel > 0.0 {
            let scrolled =
                way.of(area.scroll_position).clamp(0.0, range);
            (scrolled / range * travel, range / travel)
        } else {
            (0.0, 0.0)
        };
        Some(Self {
            length,
            at: inset + at,
            gearing,
        })
    }
}

/// What placing a bar reads and writes of it.
type Thumb = (
    &'static ScrollThumb,
    &'static ChildOf,
    &'static Hovered,
    &'static ComputedUiRenderTargetInfo,
    &'static mut ComputedNode,
    &'static mut UiGlobalTransform,
    &'static mut BackgroundColor,
    &'static mut Visibility,
);

fn place_thumbs(
    style: Res<ScrollbarStyle>,
    areas: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&OwnScrollbar>,
    )>,
    mut thumbs: Query<Thumb, Without<Node>>,
) {
    for (
        thumb,
        parent,
        hovered,
        target,
        mut computed,
        mut transform,
        mut fill,
        mut visibility,
    ) in &mut thumbs
    {
        let placed = areas.get(parent.parent()).ok().and_then(
            |(node, area, at, own)| {
                let style = style.or(own);
                Bar::of(node, area, style, thumb.way)
                    .map(|bar| (bar, area, at, style))
            },
        );
        let Some((bar, area, at, style)) = placed else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);

        let scale = area.inverse_scale_factor().recip();
        let thick = style.width * scale;
        let size = thumb.way.vec(bar.length, thick);
        if computed.size != size {
            computed.size = size;
            computed.unrounded_size = size;
            computed.inverse_scale_factor =
                area.inverse_scale_factor();
            computed.border_radius = BorderRadius::MAX.resolve(
                scale,
                size,
                target.physical_size().as_vec2(),
            );
        }
        // From the middle of the area to the middle of the bar,
        // which lies along the far edge.
        let half = area.size() / 2.0;
        let (along, beside) = match thumb.way {
            Way::Across => (half.x, half.y),
            Way::Down => (half.y, half.x),
        };
        let middle = thumb.way.vec(
            -along + bar.at + bar.length / 2.0,
            beside - style.inset * scale - thick / 2.0,
        );
        let to = at.affine() * Affine2::from_translation(middle);
        if transform.affine() != to {
            *transform = to.into();
        }

        let color = if thumb.dragged || hovered.0 {
            style.hot
        } else {
            style.rest
        };
        if fill.0 != color {
            fill.0 = color;
        }
    }
}

fn on_thumb_press(
    mut press: On<Pointer<Press>>,
    thumbs: Query<(), With<ScrollThumb>>,
) {
    // Or what is under the bar takes the press too.
    if thumbs.contains(press.entity) {
        press.propagate(false);
    }
}

fn on_thumb_drag(
    mut drag: On<Pointer<Drag>>,
    style: Res<ScrollbarStyle>,
    mut thumbs: Query<(&mut ScrollThumb, &ChildOf)>,
    mut areas: Query<(
        &Node,
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
        Option<&OwnScrollbar>,
    )>,
) {
    let Ok((mut thumb, parent)) = thumbs.get_mut(drag.entity) else {
        return;
    };
    drag.propagate(false);
    thumb.dragged = true;
    let Ok((node, area, mut position, mut goal, own)) =
        areas.get_mut(parent.parent())
    else {
        return;
    };
    let style = style.or(own);
    let Some(bar) = Bar::of(node, area, style, thumb.way) else {
        return;
    };
    let delta =
        thumb.way.vec(thumb.way.of(drag.delta) * bar.gearing, 0.0);
    goal.scroll_by(&mut position, delta, overflow(area), false);
}

fn on_thumb_drag_end(
    end: On<Pointer<DragEnd>>,
    mut thumbs: Query<&mut ScrollThumb>,
) {
    if let Ok(mut thumb) = thumbs.get_mut(end.entity) {
        thumb.dragged = false;
    }
}
