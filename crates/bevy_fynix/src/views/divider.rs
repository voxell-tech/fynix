//! The line between two panes.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ui::{Node, PositionType, Val, ZIndex, percent, px};
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens,
};
use crate::views::frame::{Frame, FrameProps};
use crate::{Bevy, Cx, Dragging, Styled, View, style};

/// Which way a [`Divider`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Left to right, between a pane above and one below.
    Horizontal,
    /// Top to bottom, between a pane on each side.
    Vertical,
}

/// A line the length of its parent, in the theme's hairline colour,
/// with the resize cursor of its axis, grabbed by a clear strip the
/// theme's divider grip thick centred on it. It only draws: what
/// dragging it does is the app's to wire.
pub struct Divider {
    pub frame: Frame,
    pub axis: Axis,
    pub thickness: Option<f32>,
}

/// A [`Divider`] running along `axis`.
pub fn divider(axis: Axis) -> Divider {
    Divider {
        frame: Frame::unset(),
        axis,
        thickness: None,
    }
}

impl Divider {
    /// How thick the line is, the theme's divider when unset.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = Some(thickness);
        self
    }
}

impl FrameProps for Divider {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T> View<Bevy, T> for Divider
where
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let thickness =
            self.thickness.unwrap_or_else(|| cx.theme().divider());
        let grip = cx.theme().divider_grip().max(thickness);
        let reach = px((thickness - grip) / 2.0);
        let (width, height, cursor, strip) = match self.axis {
            Axis::Horizontal => (
                percent(100.0),
                px(thickness),
                SystemCursorIcon::NsResize,
                Node {
                    top: reach,
                    width: percent(100.0),
                    height: px(grip),
                    ..Node::default()
                },
            ),
            Axis::Vertical => (
                px(thickness),
                percent(100.0),
                SystemCursorIcon::EwResize,
                Node {
                    left: reach,
                    width: px(grip),
                    height: percent(100.0),
                    ..Node::default()
                },
            ),
        };
        cx.scope(|cx| {
            cx.defaults(|cx| {
                cx.root(|cx| line::<T>(cx, width, height));
            });
            let node = cx.build(self.frame);
            // Above its siblings, so the strip's overhang is not
            // under the pane beside it.
            cx.world
                .entity_mut(node)
                .insert((EntityCursor(cursor), ZIndex(1)));
            cx.world.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    ..strip
                },
                ChildOf(node),
            ));
            node
        })
    }
}

/// A divider that is not drawn until the pointer is on it or it is
/// [`Dragging`], then in the theme's accent, still the same size to
/// grab. Apply with `.rules(revealed)`.
pub fn revealed<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + MotionTokens + 'static,
{
    style::<T>()
        .fill(|_| Color::NONE)
        .hovered(|s| s.fill(|theme| theme.accent()))
        .when::<Dragging>(|s| s.fill(|theme| theme.accent()))
        .transition(Motion::Interact)
        .apply(cx);
}

/// A divider's defaults, for its frame alone.
fn line<T>(cx: &mut Cx<'_, Bevy, T>, width: Val, height: Val)
where
    T: SurfaceTokens + 'static,
{
    cx.set::<Frame>(move |frame, theme: &T| {
        frame
            .width(width)
            .height(height)
            .shrink(0.0)
            .fill(theme.hairline())
    });
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ui::{BackgroundColor, Node};

    use super::*;
    use crate::tests::{self, Plain};
    use crate::{ScopedExt, mount};

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    fn settle(app: &mut App) {
        for _ in 0..6 {
            app.update();
        }
    }

    #[test]
    fn a_revealed_divider_is_clear_until_hovered_or_dragged() {
        let accent = Color::srgb(0.9, 0.5, 0.1);
        let mut app = tests::app_with(Plain {
            hairline: Some(Color::WHITE),
            accent: Some(accent),
            ..Plain::default()
        });
        let node = mount::<Plain>(
            app.world_mut(),
            divider(Axis::Vertical).thickness(6.0).rules(revealed),
        );
        settle(&mut app);
        assert_eq!(fill(&app, node), Color::NONE);
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!((ui.width, ui.height), (px(6.0), percent(100.0)));
        assert_eq!(
            app.world().get::<EntityCursor>(node),
            Some(&EntityCursor(SystemCursorIcon::EwResize)),
            "still there to grab"
        );

        tests::hover(&mut app, node, true);
        settle(&mut app);
        assert_eq!(fill(&app, node), accent);
        tests::hover(&mut app, node, false);
        settle(&mut app);
        assert_eq!(fill(&app, node), Color::NONE);

        app.world_mut().entity_mut(node).insert(Dragging);
        settle(&mut app);
        assert_eq!(fill(&app, node), accent, "kept while dragged");
        app.world_mut().entity_mut(node).remove::<Dragging>();
        settle(&mut app);
        assert_eq!(fill(&app, node), Color::NONE);
    }

    #[test]
    fn a_wider_strip_centred_on_the_line_grabs_it() {
        let mut app = tests::app();
        let node =
            mount::<Plain>(app.world_mut(), divider(Axis::Vertical));
        let theme = Plain::default();
        let (line, grip) = (theme.divider(), theme.divider_grip());
        assert!(grip >= line * 2.0);

        let strip = tests::kids(&app, node)[0];
        let ui = app.world().get::<Node>(strip).unwrap();
        assert_eq!(ui.width, px(grip));
        assert_eq!(ui.left, px((line - grip) / 2.0));
        assert_eq!(fill(&app, strip), Color::NONE, "never drawn");
    }
}
