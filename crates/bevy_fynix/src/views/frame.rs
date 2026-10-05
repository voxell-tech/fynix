//! A box of bevy_ui layout.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display,
    FlexDirection, JustifyContent, Overflow, OverflowClipMargin,
    PositionType, UiRect, Val,
};
use fynix::element;

use crate::patch::*;
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::SpacingTokens;
use crate::transition::{blend_color, blend_f32};
use crate::{Bevy, Styled};

/// A bevy_ui [`Node`](bevy::ui::Node) with a fill and a border,
/// holding no views of its own.
///
/// Each prop is written on its own, and only when it changes, so
/// whatever else edits the node keeps the fields a frame's props did
/// not just write.
#[element(backend = Bevy, theme = SpacingTokens, prepare = prepare)]
pub struct Frame {
    #[elem(patch = PatchDirection)]
    pub direction: Prop<FlexDirection>,
    /// Between children, along both axes. The theme's gap when
    /// unset.
    #[elem(default = theme.gap(), patch = PatchGap)]
    pub gap: Prop<f32>,
    #[elem(patch = PatchPadding)]
    pub padding: Prop<UiRect>,
    #[elem(patch = PatchMargin)]
    pub margin: Prop<UiRect>,
    #[elem(patch = PatchWidth)]
    pub width: Prop<Val>,
    #[elem(patch = PatchHeight)]
    pub height: Prop<Val>,
    /// The floor `width` can shrink to. `px(0.0)` lets a flex item
    /// shrink below its content.
    #[elem(patch = PatchMinWidth)]
    pub min_width: Prop<Val>,
    #[elem(patch = PatchMinHeight)]
    pub min_height: Prop<Val>,
    #[elem(patch = PatchMaxWidth)]
    pub max_width: Prop<Val>,
    #[elem(patch = PatchMaxHeight)]
    pub max_height: Prop<Val>,
    #[elem(patch = PatchGrow)]
    pub grow: Prop<f32>,
    #[elem(default = 1.0, patch = PatchShrink)]
    pub shrink: Prop<f32>,
    #[elem(patch = PatchJustify)]
    pub justify: Prop<JustifyContent>,
    #[elem(patch = PatchAlign)]
    pub align: Prop<AlignItems>,
    /// Absolute for a frame that places itself, with `inset` saying
    /// where.
    #[elem(patch = PatchPosition)]
    pub position: Prop<PositionType>,
    /// How far each edge sits from the parent's, for an absolute
    /// frame. `Auto` on an edge leaves that one to the layout.
    #[elem(default = UiRect::all(Val::Auto), patch = PatchInset)]
    pub inset: Prop<UiRect>,
    #[elem(patch = PatchOverflow)]
    pub overflow: Prop<Overflow>,
    /// The box what overflows is clipped to. The padding box when
    /// unset.
    #[elem(patch = PatchClipMargin)]
    pub clip_margin: Prop<OverflowClipMargin>,
    /// `Display::None` hides it and takes it out of the layout.
    #[elem(patch = PatchDisplay)]
    pub display: Prop<Display>,
    /// Transparent when unset, for a frame that only wants the
    /// layout.
    #[elem(default = Color::NONE, patch = PatchFill, blend = blend_color)]
    pub fill: Prop<Color>,
    #[elem(patch = PatchRadius, blend = blend_f32)]
    pub radius: Prop<f32>,
    /// Independent corners, which win over `radius` while set.
    #[elem(default = NO_CORNERS, patch = PatchCorners)]
    pub corners: Prop<BorderRadius>,
    /// The width of a border on every edge. It needs `border_color`
    /// to show.
    #[elem(patch = PatchBorder, blend = blend_f32)]
    pub border: Prop<f32>,
    #[elem(
        default = Color::NONE,
        patch = PatchBorderColor,
        blend = blend_color
    )]
    pub border_color: Prop<Color>,
    /// Where it sits in the window's stack, for a frame that has to
    /// be above what it does not sit inside. `None`, as when unset,
    /// leaves it in its parent's.
    #[elem(patch = PatchZ)]
    pub z: Prop<Option<i32>>,
    /// How opaque it is, 1.0 when unset.
    #[elem(default = 1.0, patch = PatchOpacity, blend = blend_f32)]
    pub opacity: Prop<f32>,
    /// The factor it is scaled by around its centre after layout,
    /// 1.0 when unset. Its children are scaled with it.
    #[elem(default = 1.0, patch = PatchScale, blend = blend_f32)]
    pub scale: Prop<f32>,
}

pub fn frame() -> Frame {
    Frame::unset()
}

fn prepare(world: &mut World, node: Entity) {
    world.entity_mut(node).insert((
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
        Paint::default(),
        Rounding::default(),
    ));
}

own_when!(Frame);

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Alpha;
    use bevy::ecs::resource::Resource;
    use bevy::ui::{BorderRadius, GlobalZIndex, Node, percent, px};

    use super::*;
    use crate::tests::{Plain, app};
    use crate::tokens::Motion;
    use crate::{ScopedExt, mount};

    fn ui(app: &App, node: Entity) -> &Node {
        app.world().get::<Node>(node).expect("a node")
    }

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
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
        assert_eq!(fill(&app, node), Color::WHITE);
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
                .z(Some(7)),
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
    fn corners_win_over_the_radius_whichever_is_written_last() {
        let mut app = app();
        let left = BorderRadius::left(px(8.0));
        let node = mount::<Plain>(
            app.world_mut(),
            frame().corners(left).radius(2.0),
        );
        let plain =
            mount::<Plain>(app.world_mut(), frame().radius(2.0));

        assert_eq!(ui(&app, node).border_radius, left);
        assert_eq!(
            ui(&app, plain).border_radius,
            BorderRadius::all(px(2.0)),
            "unset corners use the radius"
        );
    }

    #[test]
    fn an_unset_z_leaves_the_node_in_its_parents_stack() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), frame());

        assert!(app.world().get::<GlobalZIndex>(node).is_none());
    }

    #[test]
    fn opacity_fades_the_fill_and_the_border() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            frame()
                .fill(Color::WHITE)
                .border_color(Color::WHITE)
                .opacity(0.5),
        );

        let half = Color::WHITE.with_alpha(0.5);
        assert_eq!(fill(&app, node), half);
        assert_eq!(
            app.world().get::<BorderColor>(node),
            Some(&BorderColor::all(half))
        );
    }

    #[derive(Resource)]
    struct Wide(f32);

    #[test]
    fn a_bound_prop_is_rewritten_and_leaves_other_fields() {
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

    #[derive(Resource)]
    struct Lit(bool);

    #[test]
    fn under_a_transition_the_fill_eases_while_the_width_snaps() {
        let mut app = app();
        app.insert_resource(Lit(false));
        let lit =
            |on: Color, off: Color| {
                crate::resource::<Lit, _>(move |lit| {
                    if lit.0 { on } else { off }
                })
            };
        let node = mount::<Plain>(
            app.world_mut(),
            frame()
                .fill(lit(Color::WHITE, Color::BLACK))
                .width(crate::resource::<Lit, _>(|lit| {
                    px(if lit.0 { 20.0 } else { 10.0 })
                }))
                .transition(Motion::Interact),
        );

        app.world_mut().resource_mut::<Lit>().0 = true;
        app.update();

        assert_eq!(ui(&app, node).width, Val::Px(20.0), "at once");
        assert_eq!(
            fill(&app, node),
            blend_color(&Color::BLACK, &Color::WHITE, 0.5),
            "50ms of 100ms"
        );
    }
}
