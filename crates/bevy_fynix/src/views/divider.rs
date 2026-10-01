//! The line between two panes.

use bevy::ecs::entity::Entity;
use bevy::ui::{Val, percent, px};
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::tokens::{SpacingTokens, SurfaceTokens};
use crate::views::frame::{Frame, FrameProps};
use crate::{Bevy, Cx, Styled, View};

/// The thickness of a divider that is there to be grabbed.
const GRIP: f32 = 6.0;

/// Which way a [`Divider`] runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Left to right, between a pane above and one below.
    Horizontal,
    /// Top to bottom, between a pane on each side.
    Vertical,
}

/// A line the length of its parent, in the theme's hairline colour,
/// with the resize cursor of its axis. It only draws: what dragging
/// it does is the app's to wire.
pub struct Divider {
    pub frame: Frame,
    pub axis: Axis,
    pub thickness: f32,
}

/// A [`Divider`] running along `axis`.
pub fn divider(axis: Axis) -> Divider {
    Divider {
        frame: Frame::unset(),
        axis,
        thickness: GRIP,
    }
}

impl Divider {
    /// How thick the line is, 6.0 when unset.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
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
        let (width, height, cursor) = match self.axis {
            Axis::Horizontal => (
                percent(100.0),
                px(self.thickness),
                SystemCursorIcon::NsResize,
            ),
            Axis::Vertical => (
                px(self.thickness),
                percent(100.0),
                SystemCursorIcon::EwResize,
            ),
        };
        cx.scope(|cx| {
            cx.defaults(|cx| {
                cx.root(|cx| line::<T>(cx, width, height));
            });
            let node = cx.build(self.frame);
            cx.world.entity_mut(node).insert(EntityCursor(cursor));
            node
        })
    }
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
    use crate::mount;
    use crate::testing::{self, Plain};

    fn app() -> App {
        testing::app_with(Plain {
            hairline: Some(Color::WHITE),
            ..Plain::default()
        })
    }

    #[test]
    fn a_horizontal_divider_spans_the_width_with_a_vertical_resize_cursor()
     {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            divider(Axis::Horizontal),
        );

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!((ui.width, ui.height), (percent(100.0), px(6.0)));
        assert_eq!(ui.flex_shrink, 0.0);
        assert_eq!(
            app.world().get::<BackgroundColor>(node).unwrap().0,
            Color::WHITE
        );
        assert_eq!(
            app.world().get::<EntityCursor>(node),
            Some(&EntityCursor(SystemCursorIcon::NsResize))
        );
    }

    #[test]
    fn a_vertical_divider_spans_the_height_and_takes_a_thickness() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            divider(Axis::Vertical).thickness(1.0),
        );

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!((ui.width, ui.height), (px(1.0), percent(100.0)));
        assert_eq!(
            app.world().get::<EntityCursor>(node),
            Some(&EntityCursor(SystemCursorIcon::EwResize))
        );
    }

    #[test]
    fn the_call_site_beats_the_dividers_defaults() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            divider(Axis::Horizontal).fill(Color::BLACK),
        );

        assert_eq!(
            app.world().get::<BackgroundColor>(node).unwrap().0,
            Color::BLACK
        );
    }
}
