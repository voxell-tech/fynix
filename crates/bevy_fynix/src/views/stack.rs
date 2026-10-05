//! Composites laying their children out in a line.
//!
//! A [`Stack`] holds a [`Frame`] and takes its props through
//! [`FrameProps`], so `row((a, b)).gap(8.0)` styles the frame
//! directly and a call site can still set every prop.

use bevy::ecs::bundle::Bundle;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::EntityWorldMut;
use bevy::picking::Pickable;
use bevy::ui::{
    FlexDirection, Overflow, OverflowClipMargin, PositionType,
    UiRect, percent, px,
};

use crate::scroll::ScrollGoal;
use crate::tokens::SpacingTokens;
use crate::views::frame::{Frame, FrameProps};
use crate::{Bevy, Cx, Styled, View, ViewSeq};

/// Components a view puts on its root node, in the order they were
/// given: nothing, or more of them and then a bundle.
pub trait Extra {
    fn insert(self, entity: &mut EntityWorldMut);
}

impl Extra for () {
    fn insert(self, _: &mut EntityWorldMut) {}
}

impl<X: Extra, N: Bundle> Extra for (X, N) {
    fn insert(self, entity: &mut EntityWorldMut) {
        self.0.insert(entity);
        entity.insert(self.1);
    }
}

/// A [`Frame`] with `children` built under it, in order, and the
/// components `X` on its node.
pub struct Stack<C, X = ()> {
    pub frame: Frame,
    pub children: C,
    pub extra: X,
}

/// A [`Stack`] laying `children` out left to right.
pub fn row<C>(children: C) -> Stack<C> {
    Stack {
        frame: Frame::unset().direction(FlexDirection::Row),
        children,
        extra: (),
    }
}

/// A [`Stack`] laying `children` out top to bottom.
pub fn column<C>(children: C) -> Stack<C> {
    Stack {
        frame: Frame::unset().direction(FlexDirection::Column),
        children,
        extra: (),
    }
}

/// A column that scrolls what does not fit, by wheel or trackpad. It
/// can shrink below its content, which is what leaves something to
/// scroll. What scrolls is clipped inside its padding, and a wheel
/// notch eases to where it leads.
pub fn scroll<C>(children: C) -> Stack<C, ((), ScrollGoal)> {
    column(children)
        .overflow(Overflow::scroll())
        .clip_margin(OverflowClipMargin::content_box())
        .min_width(px(0.0))
        .min_height(px(0.0))
        .with(ScrollGoal::default())
}

/// A stack the size of its parent and out of its layout, for what
/// positions itself against the window or a panel instead of among
/// its siblings. The pointer passes through it unless
/// `.with(Pickable::default())` says otherwise.
pub fn overlay<C>(children: C) -> Stack<C, ((), Pickable)> {
    row(children)
        .position(PositionType::Absolute)
        .inset(UiRect::all(px(0.0)))
        .width(percent(100.0))
        .height(percent(100.0))
        .with(Pickable::IGNORE)
}

impl<C, X> Stack<C, X> {
    /// This, with `bundle` on its node too. A component given again
    /// replaces the one given before.
    pub fn with<N: Bundle>(self, bundle: N) -> Stack<C, (X, N)> {
        Stack {
            frame: self.frame,
            children: self.children,
            extra: (self.extra, bundle),
        }
    }
}

impl<C, X> FrameProps for Stack<C, X> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T, C, X> View<Bevy, T> for Stack<C, X>
where
    T: SpacingTokens + Send + Sync + 'static,
    C: ViewSeq<Bevy, T>,
    X: Extra,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let node = cx.build(self.frame);
        self.extra.insert(&mut cx.world.entity_mut(node));
        cx.under(node, |cx| self.children.build_each(cx));
        node
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::text::{FontSize, TextFont};
    use bevy::ui::widget::Text;
    use bevy::ui::{Node, Val};

    use super::*;
    use crate::tests::{Plain, app};
    use crate::views::{Label, label};
    use crate::{AnyView, mount};

    fn kids(app: &App, node: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(node)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    fn text(app: &App, node: Entity) -> String {
        app.world().get::<Text>(node).expect("a label").0.clone()
    }

    #[test]
    fn a_row_builds_its_children_in_order() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            row((label("a"), label("b"), label("c"))),
        );

        let texts = kids(&app, node)
            .into_iter()
            .map(|kid| text(&app, kid))
            .collect::<Vec<_>>();
        assert_eq!(texts, ["a", "b", "c"]);
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.flex_direction, FlexDirection::Row);
    }

    #[test]
    fn a_column_takes_a_vec_and_defaults_the_gap_from_the_theme() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            column(vec![label("a"), label("b")]),
        );

        assert_eq!(kids(&app, node).len(), 2);
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.flex_direction, FlexDirection::Column);
        assert_eq!(ui.row_gap, Val::Px(6.0));
    }

    #[test]
    fn a_set_rule_on_frame_reaches_a_stack_but_not_its_call_site() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Frame>(|f, _| f.gap(12.0));
                    cx.set::<Label>(|l, _| l.size(20.0));
                    cx.build(row((label("a"),)));
                    cx.build(row((label("b"),)).gap(1.0));
                });
                root
            }),
        );
        let [ruled, explicit] = kids(&app, root)[..] else {
            panic!("two rows");
        };

        let gap =
            |node| app.world().get::<Node>(node).unwrap().row_gap;
        assert_eq!(gap(ruled), Val::Px(12.0));
        assert_eq!(gap(explicit), Val::Px(1.0), "call site wins");
        let inner = kids(&app, ruled)[0];
        assert_eq!(
            app.world().get::<TextFont>(inner).unwrap().font_size,
            FontSize::Px(20.0),
            "rules reach children of a stack"
        );
    }

    #[test]
    fn a_stack_carries_what_it_is_given_and_the_later_replaces() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            row((label("a"),))
                .with(Pickable::IGNORE)
                .with(Pickable::default())
                .gap(3.0),
        );

        assert_eq!(
            app.world().get::<Pickable>(node),
            Some(&Pickable::default())
        );
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.column_gap, Val::Px(3.0), "still a stack");
    }

    #[test]
    fn a_scroll_area_scrolls_and_can_shrink_below_its_content() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), scroll((label("a"),)));

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.overflow, Overflow::scroll());
        assert_eq!((ui.min_width, ui.min_height), (px(0.0), px(0.0)));
        assert_eq!(ui.flex_direction, FlexDirection::Column);
        assert!(app.world().get::<ScrollGoal>(node).is_some());
        assert_eq!(kids(&app, node).len(), 1);
    }

    #[test]
    fn an_overlay_fills_its_parent_out_of_the_layout_and_is_not_picked()
     {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), overlay((label("a"),)));

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.position_type, PositionType::Absolute);
        assert_eq!((ui.left, ui.top), (px(0.0), px(0.0)));
        assert_eq!(
            (ui.width, ui.height),
            (percent(100.0), percent(100.0))
        );
        assert_eq!(
            app.world().get::<Pickable>(node),
            Some(&Pickable::IGNORE)
        );
    }
}
