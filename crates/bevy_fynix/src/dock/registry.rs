//! The window kinds that can be docked.

use bevy::asset::Handle;
use bevy::ecs::resource::Resource;
use bevy::image::Image;

use crate::{AnyView, Bevy};

/// What a window kind builds its content with.
type Build<T> = Box<dyn Fn() -> AnyView<Bevy, T> + Send + Sync>;

/// A kind of window: its name, an optional icon, and the view of
/// its content.
pub struct DockWindowKind<T> {
    pub name: String,
    pub icon: Option<Handle<Image>>,
    build: Build<T>,
}

impl<T> DockWindowKind<T> {
    pub fn new(
        name: impl Into<String>,
        build: impl Fn() -> AnyView<Bevy, T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            icon: None,
            build: Box::new(build),
        }
    }

    pub fn icon(mut self, icon: Handle<Image>) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A new view of the window's content.
    pub fn build(&self) -> AnyView<Bevy, T> {
        (self.build)()
    }
}

/// The window kinds of an app, by id, in the order they were
/// registered.
#[derive(Resource)]
pub struct DockRegistry<T> {
    kinds: Vec<(String, DockWindowKind<T>)>,
}

impl<T> Default for DockRegistry<T> {
    fn default() -> Self {
        Self { kinds: Vec::new() }
    }
}

impl<T> DockRegistry<T> {
    /// Registers `kind` as `id`, replacing the kind already there.
    pub fn register(
        &mut self,
        id: impl Into<String>,
        kind: DockWindowKind<T>,
    ) -> &mut Self {
        let id = id.into();
        match self.kinds.iter_mut().find(|(known, _)| *known == id) {
            Some(slot) => slot.1 = kind,
            None => self.kinds.push((id, kind)),
        }
        self
    }

    /// Removes the kind `id`, returning whether it was there.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.kinds.len();
        self.kinds.retain(|(known, _)| known != id);
        self.kinds.len() != before
    }

    pub fn get(&self, id: &str) -> Option<&DockWindowKind<T>> {
        self.kinds
            .iter()
            .find(|(known, _)| known == id)
            .map(|(_, kind)| kind)
    }

    /// The `(id, kind)` pairs in registration order.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&str, &DockWindowKind<T>)> {
        self.kinds.iter().map(|(id, kind)| (id.as_str(), kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViewExt;
    use crate::testing::Plain;
    use crate::views::label;

    fn kind(name: &str) -> DockWindowKind<Plain> {
        DockWindowKind::new(name, || label("x").boxed())
    }

    #[test]
    fn register_duplicate_id_replaces_in_place() {
        let mut reg = DockRegistry::default();
        reg.register("x", kind("first"))
            .register("y", kind("other"))
            .register("x", kind("second"));

        assert_eq!(reg.iter().count(), 2);
        assert_eq!(
            reg.get("x").map(|kind| kind.name.as_str()),
            Some("second")
        );
        assert_eq!(
            reg.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            ["x", "y"]
        );
    }

    #[test]
    fn unregister_after_duplicate_leaves_nothing() {
        let mut reg = DockRegistry::default();
        reg.register("x", kind("first"))
            .register("x", kind("second"));

        assert!(reg.unregister("x"));
        assert!(!reg.unregister("x"));
        assert_eq!(reg.iter().count(), 0);
        assert!(reg.get("x").is_none());
    }
}
