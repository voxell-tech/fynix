//! The bars of a scroll area.
//!
//! A bar has no `Node`: it is placed after the layout, against the
//! area and not the content scrolling in it.

use core::time::Duration;

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
use bevy::picking::pointer::PointerLocation;
use bevy::time::Time;
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
    /// While the area scrolls or the pointer is by the edge the bar
    /// runs along, fading away a while after.
    #[default]
    WhenActive,
    /// While there is something to scroll that way.
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
    /// How near its edge the pointer brings a bar back, for
    /// [`ScrollbarVisibility::WhenActive`].
    pub reach: f32,
    /// How long such a bar stays once nothing keeps it.
    pub linger: Duration,
    /// How long a bar takes to fade in or out.
    pub fade: Duration,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            visibility: ScrollbarVisibility::WhenActive,
            floating: true,
            width: 6.0,
            inset: 2.0,
            min_length: 24.0,
            rest: Color::WHITE.with_alpha(0.16),
            hot: Color::WHITE.with_alpha(0.4),
            reach: 16.0,
            linger: Duration::from_millis(800),
            fade: Duration::from_millis(150),
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
    /// How far the area was scrolled when the bar was last placed.
    scrolled: f32,
    /// How long nothing has kept the bar showing.
    idle: Duration,
    /// How far it has faded in, from 0 to 1.
    shown: f32,
}

fn add_thumbs(added: On<Add, ScrollGoal>, mut commands: Commands) {
    for way in [Way::Across, Way::Down] {
        commands.spawn((
            ScrollThumb {
                way,
                dragged: false,
                scrolled: 0.0,
                // Nothing has kept it yet.
                idle: Duration::MAX,
                shown: 0.0,
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
            ScrollbarVisibility::WhenActive
            | ScrollbarVisibility::WhenNeeded => range >= 1.0,
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
    &'static mut ScrollThumb,
    &'static ChildOf,
    &'static Hovered,
    &'static ComputedUiRenderTargetInfo,
    &'static mut ComputedNode,
    &'static mut UiGlobalTransform,
    &'static mut BackgroundColor,
    &'static mut Visibility,
);

fn place_thumbs(
    time: Res<Time>,
    style: Res<ScrollbarStyle>,
    pointers: Query<&PointerLocation>,
    areas: Query<(
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&OwnScrollbar>,
    )>,
    mut thumbs: Query<Thumb, Without<Node>>,
) {
    for (
        mut thumb,
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
        let scale = area.inverse_scale_factor().recip();
        let half = area.size() / 2.0;
        let (along, beside) = match thumb.way {
            Way::Across => (half.x, half.y),
            Way::Down => (half.y, half.x),
        };

        // Kept by a scroll, a drag, or a pointer by its edge.
        let scrolled = thumb.way.of(area.scroll_position);
        let by_edge = || {
            let inverse = at.affine().inverse();
            pointers
                .iter()
                .filter_map(|pointer| pointer.location.as_ref())
                .map(|location| {
                    inverse.transform_point2(
                        location.position * target.scale_factor(),
                    )
                })
                .any(|point| {
                    let (a, b) = match thumb.way {
                        Way::Across => (point.x, point.y),
                        Way::Down => (point.y, point.x),
                    };
                    a.abs() <= along
                        && b <= beside
                        && b >= beside - style.reach * scale
                })
        };
        let kept = scrolled != thumb.scrolled
            || thumb.dragged
            || hovered.0
            || by_edge();
        thumb.scrolled = scrolled;
        thumb.idle = if kept {
            Duration::ZERO
        } else {
            thumb.idle.saturating_add(time.delta())
        };
        let showing = style.visibility
            != ScrollbarVisibility::WhenActive
            || thumb.idle < style.linger;
        let step = if style.fade.is_zero() {
            1.0
        } else {
            time.delta_secs() / style.fade.as_secs_f32()
        };
        thumb.shown = if showing {
            (thumb.shown + step).min(1.0)
        } else {
            (thumb.shown - step).max(0.0)
        };
        // Out of the way of what is under it once it has gone.
        if thumb.shown <= 0.0 {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        visibility.set_if_neq(Visibility::Inherited);

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
        let color = color.with_alpha(color.alpha() * thumb.shown);
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
