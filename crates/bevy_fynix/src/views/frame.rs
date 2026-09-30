//! A box of bevy_ui layout, and the macro composites use to forward
//! its builder methods.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection,
    JustifyContent, Node, UiRect, Val, px,
};

use motiongfx_interp::interpolation::{InterpFn, Interpolation};

use crate::leave::Collapsing;
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::SpacingTokens;
use crate::transition::BevyMarker;
use crate::visual::{faded, scaled, visual_access, visual_props};
use crate::{Bevy, Element, Styled};

/// A bevy_ui [`Node`] with a fill, holding no views of its own.
///
/// A prop left unset with no theme default leaves that field of the
/// node alone, so a modifier's edit to it survives a live rewrite.
pub struct Frame {
    pub direction: Prop<FlexDirection>,
    pub gap: Prop<f32>,
    pub padding: Prop<UiRect>,
    pub width: Prop<Val>,
    pub height: Prop<Val>,
    pub grow: Prop<f32>,
    pub justify: Prop<JustifyContent>,
    pub align: Prop<AlignItems>,
    pub fill: Prop<Color>,
    pub radius: Prop<f32>,
    pub opacity: Prop<f32>,
    pub scale: Prop<f32>,
}

pub fn frame() -> Frame {
    Frame::unset()
}

/// Forwarding builder methods for a struct holding a `frame: Frame`.
macro_rules! forward_frame_props {
    ($($prop:ident: $ty:ty),* $(,)?) => {
        $(
            pub fn $prop(mut self, $prop: impl Into<Prop<$ty>>) -> Self {
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
            padding: UiRect,
            width: Val,
            height: Val,
            grow: f32,
            justify: JustifyContent,
            align: AlignItems,
            fill: Color,
            radius: f32,
            opacity: f32,
            scale: f32,
        );
    };
}

pub(crate) use {forward_all_frame_props, forward_frame_props};

/// One builder method per prop, setting the field of the same name.
macro_rules! set_props {
    ($($prop:ident: $ty:ty),* $(,)?) => {
        $(
            pub fn $prop(mut self, $prop: impl Into<Prop<$ty>>) -> Self {
                self.$prop = $prop.into();
                self
            }
        )*
    };
}

impl Frame {
    set_props!(
        direction: FlexDirection,
        gap: f32,
        padding: UiRect,
        width: Val,
        height: Val,
        grow: f32,
        justify: JustifyContent,
        align: AlignItems,
        fill: Color,
        radius: f32,
    );

    visual_props!();
}

fynix::styled!(Frame {
    direction,
    gap,
    padding,
    width,
    height,
    grow,
    justify,
    align,
    fill,
    radius,
    opacity,
    scale,
});

own_when!(Frame);

/// A [`Frame`]'s props at one moment. A `None` field is left as the
/// node has it.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSnapshot {
    pub direction: Option<FlexDirection>,
    pub gap: f32,
    pub padding: Option<UiRect>,
    pub width: Option<Val>,
    pub height: Option<Val>,
    pub grow: Option<f32>,
    pub justify: Option<JustifyContent>,
    pub align: Option<AlignItems>,
    pub fill: Color,
    pub radius: f32,
    pub opacity: f32,
    pub scale: f32,
}

/// The fill, opacity and scale blend, and everything else takes the
/// target.
impl Interpolation<BevyMarker> for FrameSnapshot {
    fn interp(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            fill: <Color as Interpolation<BevyMarker>>::interp(
                &from.fill, &to.fill, t,
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
            width: self.width.get(world),
            height: self.height.get(world),
            grow: self.grow.get(world),
            justify: self.justify.get(world),
            align: self.align.get(world),
            fill: self.fill.get(world).unwrap_or(Color::NONE),
            radius: self.radius.get(world).unwrap_or(0.0),
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
            if let Some(padding) = snapshot.padding.filter(|_| sized)
            {
                ui.padding = padding;
            }
            if let Some(width) = snapshot.width.filter(|_| sized) {
                ui.width = width;
            }
            if let Some(height) = snapshot.height.filter(|_| sized) {
                ui.height = height;
            }
            if let Some(grow) = snapshot.grow {
                ui.flex_grow = grow;
            }
            if let Some(justify) = snapshot.justify {
                ui.justify_content = justify;
            }
            if let Some(align) = snapshot.align {
                ui.align_items = align;
            }
        }
        entity.insert((
            BackgroundColor(faded(snapshot.fill, snapshot.opacity)),
            scaled(snapshot.scale),
        ));
    }

    fn is_live(&self) -> bool {
        self.direction.is_bound()
            || self.gap.is_bound()
            || self.padding.is_bound()
            || self.width.is_bound()
            || self.height.is_bound()
            || self.grow.is_bound()
            || self.justify.is_bound()
            || self.align.is_bound()
            || self.fill.is_bound()
            || self.radius.is_bound()
            || self.opacity.is_bound()
            || self.scale.is_bound()
    }

    fn changed(&mut self, world: &World) -> bool {
        self.direction.changed(world)
            | self.gap.changed(world)
            | self.padding.changed(world)
            | self.width.changed(world)
            | self.height.changed(world)
            | self.grow.changed(world)
            | self.justify.changed(world)
            | self.align.changed(world)
            | self.fill.changed(world)
            | self.radius.changed(world)
            | self.opacity.changed(world)
            | self.scale.changed(world)
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
