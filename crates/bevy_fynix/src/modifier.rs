//! Generic wrappers any [`Bevy`] view takes. Each builds the inner
//! view and then edits the [`Node`] it returned, so on a composite it
//! applies to the root node.
//!
//! An edit is made once at build. A live prop of the inner frame on
//! the same field rewrites it when that prop changes.

use bevy::ecs::entity::Entity;
use bevy::ui::{Node, UiRect, Val};

use crate::{Bevy, Cx, View};

/// A view with its root node's padding set.
pub struct Padded<V> {
    inner: V,
    padding: UiRect,
}

/// A view with its root node's width and height set.
pub struct Sizing<V> {
    inner: V,
    width: Option<Val>,
    height: Option<Val>,
}

/// A view with its root node's flex grow set.
pub struct Grown<V> {
    inner: V,
    grow: f32,
}

/// Builds `inner`, then edits its root node's [`Node`].
fn edit<T, V: View<Bevy, T>>(
    inner: V,
    cx: &mut Cx<'_, Bevy, T>,
    edit: impl FnOnce(&mut Node),
) -> Entity {
    let node = inner.build(cx);
    if let Some(mut ui) = cx.world.get_mut::<Node>(node) {
        edit(&mut ui);
    }
    node
}

impl<T, V: View<Bevy, T>> View<Bevy, T> for Padded<V> {
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        edit(self.inner, cx, |ui| ui.padding = self.padding)
    }
}

impl<T, V: View<Bevy, T>> View<Bevy, T> for Sizing<V> {
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        edit(self.inner, cx, |ui| {
            if let Some(width) = self.width {
                ui.width = width;
            }
            if let Some(height) = self.height {
                ui.height = height;
            }
        })
    }
}

impl<T, V: View<Bevy, T>> View<Bevy, T> for Grown<V> {
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        edit(self.inner, cx, |ui| ui.flex_grow = self.grow)
    }
}

/// The modifiers any view takes, for a view with no such prop of its
/// own. They are named apart from a frame's props (`padding`,
/// `width`, `height`, `grow`), which are the better choice where
/// there is a frame.
pub trait ModifierExt: Sized {
    /// This view, with its root node's padding set.
    fn padded(self, padding: UiRect) -> Padded<Self> {
        Padded {
            inner: self,
            padding,
        }
    }

    /// This view, with its root node's width set.
    fn wide(self, width: Val) -> Sizing<Self> {
        Sizing {
            inner: self,
            width: Some(width),
            height: None,
        }
    }

    /// This view, with its root node's height set.
    fn tall(self, height: Val) -> Sizing<Self> {
        Sizing {
            inner: self,
            width: None,
            height: Some(height),
        }
    }

    /// This view, with its root node's flex grow set.
    fn grown(self, grow: f32) -> Grown<Self> {
        Grown { inner: self, grow }
    }
}

impl<V> ModifierExt for V {}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::time::TimePlugin;
    use bevy::ui::{percent, px};

    use super::*;
    use crate::tokens::{SpacingTokens, TextTokens, Tone};
    use crate::views::{FrameProps, label, row};
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

    #[test]
    fn modifiers_edit_an_elements_node() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            label("x")
                .padded(UiRect::all(px(4.0)))
                .wide(px(40.0))
                .tall(px(20.0))
                .grown(2.0),
        );

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.padding, UiRect::all(Val::Px(4.0)));
        assert_eq!(ui.width, Val::Px(40.0));
        assert_eq!(ui.height, Val::Px(20.0));
        assert_eq!(ui.flex_grow, 2.0);
    }

    #[test]
    fn a_modifier_on_a_composite_edits_its_root_node() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            row((label("a"), label("b")))
                .gap(8.0)
                .padded(UiRect::all(px(5.0)))
                .wide(percent(50.0))
                .grown(1.0),
        );

        let ui = app.world().get::<Node>(root).unwrap();
        assert_eq!(ui.padding, UiRect::all(Val::Px(5.0)));
        assert_eq!(ui.width, Val::Percent(50.0));
        assert_eq!(ui.flex_grow, 1.0);
        assert_eq!(
            ui.column_gap,
            Val::Px(8.0),
            "the frame keeps its own"
        );
        let kids = app.world().get::<Children>(root).unwrap();
        assert_eq!(kids.len(), 2);
        let child = app.world().get::<Node>(kids[0]).unwrap();
        assert_eq!(
            child.padding,
            UiRect::default(),
            "children untouched"
        );
    }
}
