//! Composites laying their children out in a line.
//!
//! A [`Stack`] holds a [`Frame`] and forwards the frame's builder
//! methods, so `row((a, b)).gap(8.0)` styles the frame directly and a
//! call site can still set every prop. Generic modifiers reach the
//! root node too, but a prop of the frame is better said on the frame.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ui::{
    AlignItems, FlexDirection, JustifyContent, UiRect, Val,
};

use crate::prop::Prop;
use crate::tokens::SpacingTokens;
use crate::views::frame::{Frame, forward_all_frame_props};
use crate::{Bevy, Cx, Styled, View, ViewSeq};

/// A [`Frame`] with `children` built under it, in order.
pub struct Stack<C> {
    pub frame: Frame,
    pub children: C,
}

/// A [`Stack`] laying `children` out left to right.
pub fn row<C>(children: C) -> Stack<C> {
    Stack {
        frame: Frame::unset().direction(FlexDirection::Row),
        children,
    }
}

/// A [`Stack`] laying `children` out top to bottom.
pub fn column<C>(children: C) -> Stack<C> {
    Stack {
        frame: Frame::unset().direction(FlexDirection::Column),
        children,
    }
}

impl<C> Stack<C> {
    pub fn direction(
        mut self,
        direction: impl Into<Prop<FlexDirection>>,
    ) -> Self {
        self.frame = self.frame.direction(direction);
        self
    }

    forward_all_frame_props!();
}

impl<T, C> View<Bevy, T> for Stack<C>
where
    T: SpacingTokens + Send + Sync + 'static,
    C: ViewSeq<Bevy, T>,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let node = cx.build(self.frame);
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
    use bevy::time::TimePlugin;
    use bevy::ui::widget::Text;
    use bevy::ui::{BackgroundColor, Node};

    use super::*;
    use crate::tokens::{TextTokens, Tone};
    use crate::views::{Label, label};
    use crate::{AnyView, FynixPlugin, Theme, mount};

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

    impl TextTokens for Plain {
        fn tone(&self, _: Tone) -> Color {
            Color::WHITE
        }

        fn body_size(&self) -> f32 {
            14.0
        }

        fn small_size(&self) -> f32 {
            11.0
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
    fn frame_props_are_styled_through_the_stack() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            row((label("a"),)).gap(8.0).fill(Color::WHITE),
        );

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.column_gap, Val::Px(8.0));
        assert_eq!(
            app.world().get::<BackgroundColor>(node).unwrap().0,
            Color::WHITE
        );
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
}
