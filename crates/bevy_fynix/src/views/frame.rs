//! A box of bevy_ui layout, and the macro composites use to forward
//! its builder methods.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display,
    FlexDirection, GlobalZIndex, JustifyContent, Node, Overflow,
    PositionType, UiRect, Val, px,
};
use motiongfx_interp::interpolation::{InterpFn, Interpolation};

use crate::leave::Collapsing;
use crate::props::props;
use crate::state::own_when;
use crate::tokens::SpacingTokens;
use crate::transition::BevyMarker;
use crate::visual::{faded, scaled, visual_access};
use crate::{Bevy, Element, Styled};

props! {
    /// A bevy_ui [`Node`] with a fill and a border, holding no views
    /// of its own.
    ///
    /// A prop left unset with no theme default leaves that field of
    /// the node alone, so a modifier's edit to it survives a live
    /// rewrite.
    pub struct Frame {
        direction: FlexDirection,
        /// Between children, along both axes. The theme's gap when
        /// unset.
        gap: f32,
        padding: UiRect,
        margin: UiRect,
        width: Val,
        height: Val,
        /// The floor `width` can shrink to. `px(0.0)` lets a flex
        /// item shrink below its content.
        min_width: Val,
        min_height: Val,
        max_width: Val,
        max_height: Val,
        grow: f32,
        shrink: f32,
        justify: JustifyContent,
        align: AlignItems,
        /// Absolute for a frame that places itself, with `inset`
        /// saying where.
        position: PositionType,
        /// How far each edge sits from the parent's, for an absolute
        /// frame. `Auto` on an edge leaves that one to the layout.
        inset: UiRect,
        overflow: Overflow,
        /// `Display::None` hides it and takes it out of the layout.
        display: Display,
        /// Transparent when unset, for a frame that only wants the
        /// layout.
        fill: Color,
        radius: f32,
        /// The width of a border on every edge. It needs
        /// `border_color` to show.
        border: f32,
        border_color: Color,
        /// Where it sits in the window's stack, for a frame that has
        /// to be above what it does not sit inside. Unset leaves it
        /// in its parent's.
        z: i32,
        /// How opaque it is, 1.0 when unset.
        opacity: f32,
        /// The factor it is scaled by around its centre after layout,
        /// 1.0 when unset. Its children are scaled with it.
        scale: f32,
    }
}

pub fn frame() -> Frame {
    Frame::unset()
}

/// Forwarding builder methods for a struct holding a `frame: Frame`.
macro_rules! forward_frame_props {
    ($($prop:ident: $ty:ty),* $(,)?) => {
        $(
            pub fn $prop(
                mut self,
                $prop: impl Into<$crate::prop::Prop<$ty>>,
            ) -> Self {
                self.frame = self.frame.$prop($prop);
                self
            }
        )*
    };
}

/// Every [`Frame`] prop except direction, forwarded to `self.frame`.
macro_rules! forward_all_frame_props {
    () => {
        $crate::views::frame::forward_frame_props!(
            gap: f32,
            padding: bevy::ui::UiRect,
            margin: bevy::ui::UiRect,
            width: bevy::ui::Val,
            height: bevy::ui::Val,
            min_width: bevy::ui::Val,
            min_height: bevy::ui::Val,
            max_width: bevy::ui::Val,
            max_height: bevy::ui::Val,
            grow: f32,
            shrink: f32,
            justify: bevy::ui::JustifyContent,
            align: bevy::ui::AlignItems,
            position: bevy::ui::PositionType,
            inset: bevy::ui::UiRect,
            overflow: bevy::ui::Overflow,
            display: bevy::ui::Display,
            fill: bevy::color::Color,
            radius: f32,
            border: f32,
            border_color: bevy::color::Color,
            z: i32,
            opacity: f32,
            scale: f32,
        );
    };
}

pub(crate) use forward_all_frame_props;
pub(crate) use forward_frame_props;

own_when!(Frame);

/// A [`Frame`]'s props at one moment. A `None` field is left as the
/// node has it.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSnapshot {
    pub direction: Option<FlexDirection>,
    pub gap: f32,
    pub padding: Option<UiRect>,
    pub margin: Option<UiRect>,
    pub width: Option<Val>,
    pub height: Option<Val>,
    pub min_width: Option<Val>,
    pub min_height: Option<Val>,
    pub max_width: Option<Val>,
    pub max_height: Option<Val>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    pub justify: Option<JustifyContent>,
    pub align: Option<AlignItems>,
    pub position: Option<PositionType>,
    pub inset: Option<UiRect>,
    pub overflow: Option<Overflow>,
    pub display: Option<Display>,
    pub fill: Color,
    pub radius: f32,
    pub border: f32,
    pub border_color: Color,
    pub z: Option<i32>,
    pub opacity: f32,
    pub scale: f32,
}

/// The fill, border colour, opacity and scale blend, and everything
/// else takes the target.
impl Interpolation<BevyMarker> for FrameSnapshot {
    fn interp(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            fill: <Color as Interpolation<BevyMarker>>::interp(
                &from.fill, &to.fill, t,
            ),
            border_color:
                <Color as Interpolation<BevyMarker>>::interp(
                    &from.border_color,
                    &to.border_color,
                    t,
                ),
            opacity: <f32 as Interpolation<()>>::interp(
                &from.opacity,
                &to.opacity,
                t,
            ),
            scale: <f32 as Interpolation<()>>::interp(
                &from.scale,
                &to.scale,
                t,
            ),
            ..to.clone()
        }
    }
}

impl<T: SpacingTokens> Element<Bevy, T> for Frame {
    type Snapshot = FrameSnapshot;

    fn prepare(world: &mut World, node: Entity) {
        world.entity_mut(node).insert(BackgroundColor(Color::NONE));
    }

    fn snapshot(&self, world: &World, theme: &T) -> FrameSnapshot {
        FrameSnapshot {
            direction: self.direction.get(world),
            gap: self.gap.get(world).unwrap_or(theme.gap()),
            padding: self.padding.get(world),
            margin: self.margin.get(world),
            width: self.width.get(world),
            height: self.height.get(world),
            min_width: self.min_width.get(world),
            min_height: self.min_height.get(world),
            max_width: self.max_width.get(world),
            max_height: self.max_height.get(world),
            grow: self.grow.get(world),
            shrink: self.shrink.get(world),
            justify: self.justify.get(world),
            align: self.align.get(world),
            position: self.position.get(world),
            inset: self.inset.get(world),
            overflow: self.overflow.get(world),
            display: self.display.get(world),
            fill: self.fill.get(world).unwrap_or(Color::NONE),
            radius: self.radius.get(world).unwrap_or(0.0),
            border: self.border.get(world).unwrap_or(0.0),
            border_color: self
                .border_color
                .get(world)
                .unwrap_or(Color::NONE),
            z: self.z.get(world),
            opacity: self.opacity.get(world).unwrap_or(1.0),
            scale: self.scale.get(world).unwrap_or(1.0),
        }
    }

    fn write(
        snapshot: &FrameSnapshot,
        world: &mut World,
        node: Entity,
    ) {
        let mut entity = world.entity_mut(node);
        // A collapsing node's size is the collapse's to write.
        let sized = !entity.contains::<Collapsing>();
        if let Some(mut ui) = entity.get_mut::<Node>() {
            if let Some(direction) = snapshot.direction {
                ui.flex_direction = direction;
            }
            ui.border_radius = BorderRadius::all(px(snapshot.radius));
            ui.row_gap = px(snapshot.gap);
            ui.column_gap = px(snapshot.gap);
            if sized && let Some(padding) = snapshot.padding {
                ui.padding = padding;
            }
            if let Some(width) = snapshot.width.filter(|_| sized) {
                ui.width = width;
            }
            if let Some(height) = snapshot.height.filter(|_| sized) {
                ui.height = height;
            }
            if let Some(min) = snapshot.min_width.filter(|_| sized) {
                ui.min_width = min;
            }
            if let Some(min) = snapshot.min_height.filter(|_| sized) {
                ui.min_height = min;
            }
            if let Some(max) = snapshot.max_width {
                ui.max_width = max;
            }
            if let Some(max) = snapshot.max_height {
                ui.max_height = max;
            }
            if let Some(margin) = snapshot.margin.filter(|_| sized) {
                ui.margin = margin;
            }
            if let Some(grow) = snapshot.grow {
                ui.flex_grow = grow;
            }
            if let Some(shrink) = snapshot.shrink.filter(|_| sized) {
                ui.flex_shrink = shrink;
            }
            if let Some(justify) = snapshot.justify {
                ui.justify_content = justify;
            }
            if let Some(align) = snapshot.align {
                ui.align_items = align;
            }
            if let Some(position) = snapshot.position {
                ui.position_type = position;
            }
            if let Some(inset) = snapshot.inset {
                ui.left = inset.left;
                ui.right = inset.right;
                ui.top = inset.top;
                ui.bottom = inset.bottom;
            }
            if let Some(overflow) =
                snapshot.overflow.filter(|_| sized)
            {
                ui.overflow = overflow;
            }
            if let Some(display) = snapshot.display {
                ui.display = display;
            }
            ui.border = UiRect::all(px(snapshot.border));
        }
        entity.insert((
            BackgroundColor(faded(snapshot.fill, snapshot.opacity)),
            BorderColor::all(faded(
                snapshot.border_color,
                snapshot.opacity,
            )),
            scaled(snapshot.scale),
        ));
        match snapshot.z {
            Some(z) => entity.insert(GlobalZIndex(z)),
            None => entity.remove::<GlobalZIndex>(),
        };
    }

    fn is_live(&self) -> bool {
        self.any_bound()
    }

    fn changed(&mut self, world: &World) -> bool {
        self.any_changed(world)
    }

    fn interp() -> Option<InterpFn<FrameSnapshot>> {
        Some(<FrameSnapshot as Interpolation<BevyMarker>>::interp)
    }

    visual_access!();
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::time::TimePlugin;
    use bevy::ui::percent;

    use super::*;
    use crate::{FynixPlugin, Theme, mount};

    struct Plain;

    impl SpacingTokens for Plain {
        fn gap(&self) -> f32 {
            6.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            3.0
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain));
        app
    }

    fn ui(app: &App, node: Entity) -> &Node {
        app.world().get::<Node>(node).expect("a node")
    }

    #[test]
    fn a_snapshot_blends_its_fill_and_takes_the_rest_from_the_target()
    {
        let from = FrameSnapshot {
            direction: Some(FlexDirection::Row),
            gap: 1.0,
            padding: None,
            width: Some(px(10.0)),
            height: None,
            grow: None,
            justify: None,
            align: None,
            fill: Color::BLACK,
            radius: 2.0,
            opacity: 1.0,
            scale: 1.0,
            margin: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            shrink: None,
            position: None,
            inset: None,
            overflow: None,
            display: None,
            border: 0.0,
            border_color: Color::NONE,
            z: None,
        };
        let to = FrameSnapshot {
            direction: Some(FlexDirection::Column),
            gap: 5.0,
            width: Some(px(20.0)),
            fill: Color::WHITE,
            radius: 6.0,
            ..from.clone()
        };

        let mid =
            <FrameSnapshot as Interpolation<BevyMarker>>::interp(
                &from, &to, 0.5,
            );

        assert_eq!(
            mid.fill,
            <Color as Interpolation<BevyMarker>>::interp(
                &from.fill, &to.fill, 0.5
            )
        );
        assert_eq!(
            FrameSnapshot {
                fill: to.fill,
                border_color: to.border_color,
                ..mid
            },
            to
        );
    }

    #[test]
    fn unset_props_fall_back_to_the_theme_or_the_node() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), frame());

        assert_eq!(ui(&app, node).row_gap, Val::Px(6.0));
        assert_eq!(ui(&app, node).column_gap, Val::Px(6.0));
        assert_eq!(ui(&app, node).width, Val::Auto);
        assert_eq!(ui(&app, node).flex_direction, FlexDirection::Row);
        assert_eq!(
            app.world().get::<BackgroundColor>(node).unwrap().0,
            Color::NONE
        );
    }

    #[test]
    fn props_are_written_to_the_node() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            frame()
                .direction(FlexDirection::Column)
                .gap(2.0)
                .padding(UiRect::all(px(4.0)))
                .width(px(50.0))
                .height(percent(100.0))
                .grow(1.0)
                .justify(JustifyContent::Center)
                .align(AlignItems::End)
                .fill(Color::WHITE)
                .radius(5.0),
        );

        let ui = ui(&app, node);
        assert_eq!(ui.flex_direction, FlexDirection::Column);
        assert_eq!(ui.row_gap, Val::Px(2.0));
        assert_eq!(ui.padding, UiRect::all(Val::Px(4.0)));
        assert_eq!(ui.width, Val::Px(50.0));
        assert_eq!(ui.height, Val::Percent(100.0));
        assert_eq!(ui.flex_grow, 1.0);
        assert_eq!(ui.justify_content, JustifyContent::Center);
        assert_eq!(ui.align_items, AlignItems::End);
        assert_eq!(
            app.world().get::<BackgroundColor>(node).unwrap().0,
            Color::WHITE
        );
        assert_eq!(ui.border_radius, BorderRadius::all(Val::Px(5.0)));
    }

    #[test]
    fn placement_border_and_stacking_are_written_to_the_node() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            frame()
                .position(PositionType::Absolute)
                .inset(UiRect::new(
                    px(1.0),
                    px(2.0),
                    px(3.0),
                    px(4.0),
                ))
                .min_width(px(0.0))
                .max_height(px(90.0))
                .margin(UiRect::all(px(5.0)))
                .shrink(0.0)
                .overflow(Overflow::clip())
                .display(Display::None)
                .border(2.0)
                .border_color(Color::WHITE)
                .z(7),
        );

        let ui = ui(&app, node);
        assert_eq!(ui.position_type, PositionType::Absolute);
        assert_eq!(
            (ui.left, ui.right, ui.top, ui.bottom),
            (px(1.0), px(2.0), px(3.0), px(4.0))
        );
        assert_eq!(ui.min_width, px(0.0));
        assert_eq!(ui.max_height, px(90.0));
        assert_eq!(ui.margin, UiRect::all(px(5.0)));
        assert_eq!(ui.flex_shrink, 0.0);
        assert_eq!(ui.overflow, Overflow::clip());
        assert_eq!(ui.display, Display::None);
        assert_eq!(ui.border, UiRect::all(px(2.0)));
        assert_eq!(
            app.world().get::<BorderColor>(node),
            Some(&BorderColor::all(Color::WHITE))
        );
        assert_eq!(
            app.world().get::<GlobalZIndex>(node),
            Some(&GlobalZIndex(7))
        );
    }

    #[test]
    fn an_unset_z_leaves_the_node_in_its_parents_stack() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), frame());

        assert!(app.world().get::<GlobalZIndex>(node).is_none());
    }

    #[test]
    fn a_set_rule_fills_what_the_call_site_left_unset() {
        use crate::AnyView;

        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Frame>(|f, theme: &Plain| {
                        f.gap(9.0).radius(theme.radius())
                    });
                    cx.build(frame());
                    cx.build(frame().gap(1.0));
                });
                root
            }),
        );
        let kids = app
            .world()
            .get::<Children>(root)
            .expect("two frames")
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(ui(&app, kids[0]).row_gap, Val::Px(9.0));
        assert_eq!(
            ui(&app, kids[1]).row_gap,
            Val::Px(1.0),
            "call site wins"
        );
        assert_eq!(
            ui(&app, kids[1]).border_radius,
            BorderRadius::all(Val::Px(3.0))
        );
    }

    #[test]
    fn a_bound_prop_is_rewritten_and_leaves_other_fields() {
        #[derive(Resource)]
        struct Wide(f32);

        let mut app = app();
        app.insert_resource(Wide(10.0));
        let node = mount::<Plain>(
            app.world_mut(),
            frame()
                .width(crate::resource::<Wide, _>(|wide| px(wide.0))),
        );
        app.world_mut().get_mut::<Node>(node).unwrap().padding =
            UiRect::all(px(7.0));

        app.world_mut().resource_mut::<Wide>().0 = 20.0;
        app.update();

        assert_eq!(ui(&app, node).width, Val::Px(20.0));
        assert_eq!(ui(&app, node).padding, UiRect::all(Val::Px(7.0)));
    }
}
