//! Modifiers adding behaviour, a record, or a scoped tone to any
//! [`Bevy`] view. Each acts on the root node of the view it wraps.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::ui_widgets::Activate;

use crate::tokens::Tone;
use crate::views::{Icon, Label};
use crate::{Bevy, Cx, View};

/// A handler run with the whole world.
type Handler = Box<dyn Fn(&mut World) + Send + Sync>;

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
            handler(world);
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

    /// This view, with `tone` the default for every label and icon
    /// in it. It is not called `tone`, which a label or icon takes
    /// for its own prop.
    fn toned(self, tone: Tone) -> Toned<Self> {
        Toned { inner: self, tone }
    }
}

impl<V> BehaviorExt for V {}
