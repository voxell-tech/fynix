//! Modifiers adding behaviour, a record, or a scoped tone to any
//! [`Bevy`] view. Each acts on the root node of the view it wraps.

use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::ui_widgets::Activate;

use crate::backend::Seed;
use crate::tokens::Tone;
use crate::views::{Icon, Label};
use crate::{Bevy, Cx, View};

/// A handler run with the whole world and the node it fired on.
type Handler = Box<dyn Fn(&mut World, Entity) + Send + Sync>;

/// The handlers of a node's [`OnActivate`] wrappers, in build order.
#[derive(Component)]
struct ActivateHandlers(Vec<Handler>);

/// A view whose root node runs a handler on [`Activate`].
pub struct OnActivate<V> {
    inner: V,
    handler: Handler,
}

/// A view whose root node carries a component.
pub struct Tagged<V, C> {
    inner: V,
    component: C,
}

/// A view whose root node has a bundle from the moment it is
/// spawned, before anything is written to it.
pub struct Seeded<V, N> {
    inner: V,
    bundle: N,
}

/// A view built under set rules giving every [`Label`] and [`Icon`]
/// in it a default tone.
pub struct Toned<V> {
    inner: V,
    tone: Tone,
}

fn activated(activate: On<Activate>, mut commands: Commands) {
    let node = activate.event_target();
    commands.queue(move |world: &mut World| {
        let Some(ActivateHandlers(handlers)) = world
            .get_entity_mut(node)
            .ok()
            .and_then(|mut entity| entity.take::<ActivateHandlers>())
        else {
            return;
        };
        for handler in &handlers {
            handler(world, node);
        }
        if let Ok(mut entity) = world.get_entity_mut(node) {
            entity.insert(ActivateHandlers(handlers));
        }
    });
}

impl<T, V: View<Bevy, T>> View<Bevy, T> for OnActivate<V> {
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let node = self.inner.build(cx);
        let mut entity = cx.world.entity_mut(node);
        match entity.get_mut::<ActivateHandlers>() {
            Some(mut existing) => existing.0.push(self.handler),
            None => {
                entity
                    .insert(ActivateHandlers(vec![self.handler]))
                    .observe(activated);
            }
        }
        node
    }
}

impl<T, V: View<Bevy, T>, C: Component> View<Bevy, T>
    for Tagged<V, C>
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let node = self.inner.build(cx);
        cx.world.entity_mut(node).insert(self.component);
        node
    }
}

impl<T, V, N> View<Bevy, T> for Seeded<V, N>
where
    V: View<Bevy, T>,
    N: Bundle,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let bundle = self.bundle;
        cx.world.init_resource::<Seed>();
        cx.world.resource_mut::<Seed>().push(move |entity| {
            entity.insert(bundle);
        });
        let node = self.inner.build(cx);
        // If the view spawned nothing, no other node takes it.
        cx.world.resource_mut::<Seed>().clear();
        node
    }
}

impl<T: 'static, V: View<Bevy, T>> View<Bevy, T> for Toned<V> {
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let tone = self.tone;
        cx.scope(|cx| {
            cx.set::<Label>(move |label, _| label.tone(tone));
            cx.set::<Icon>(move |icon, _| icon.tone(tone));
            self.inner.build(cx)
        })
    }
}

/// The behaviour modifiers any view takes.
pub trait BehaviorExt: Sized {
    /// This view, running `handler` when its root node is activated.
    fn on_activate(
        self,
        handler: impl Fn(&mut World) + Send + Sync + 'static,
    ) -> OnActivate<Self> {
        self.on_activate_with(move |world, _| handler(world))
    }

    /// As [`on_activate`](Self::on_activate), with the root node
    /// handed to `handler` too.
    fn on_activate_with(
        self,
        handler: impl Fn(&mut World, Entity) + Send + Sync + 'static,
    ) -> OnActivate<Self> {
        OnActivate {
            inner: self,
            handler: Box::new(handler),
        }
    }

    /// This view, with `component` on its root node.
    fn tagged<C: Component>(self, component: C) -> Tagged<Self, C> {
        Tagged {
            inner: self,
            component,
        }
    }

    /// This view, with `bundle` on its root node from the start, so
    /// a state rule on a component in it holds for the first write
    /// rather than the next update. It takes the first node the
    /// view spawns, which is its root. Use
    /// [`tagged`](Self::tagged) for a component that need not be
    /// there for the first write.
    fn seeded<N: Bundle>(self, bundle: N) -> Seeded<Self, N> {
        Seeded {
            inner: self,
            bundle,
        }
    }

    /// This view, with `tone` the default for every label and icon
    /// in it. It is not called `tone`, which a label or icon takes
    /// for its own prop.
    fn toned(self, tone: Tone) -> Toned<Self> {
        Toned { inner: self, tone }
    }
}

impl<V> BehaviorExt for V {}
