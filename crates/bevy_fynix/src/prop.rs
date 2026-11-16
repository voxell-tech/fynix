//! The core's props, bound to Bevy's world.

use bevy::ecs::change_detection::Tick;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;

/// A prop of a view built into [`Bevy`](crate::Bevy).
pub type Prop<T> = fynix::Prop<World, T>;

/// A value read from the world, with its change check.
pub type Signal<T> = fynix::Signal<World, T>;

/// A view built again when its key changes, built into
/// [`Bevy`](crate::Bevy).
pub type Keyed<T, K> = fynix::Keyed<crate::Bevy, T, K>;

/// One view per item, built into [`Bevy`](crate::Bevy).
pub type Each<T, I, K> = fynix::Each<crate::Bevy, T, I, K>;

/// See [`fynix::keyed`].
pub fn keyed<T, K>(
    key: Signal<K>,
    build: impl Fn(&K) -> fynix::AnyView<crate::Bevy, T>
    + Send
    + Sync
    + 'static,
) -> Keyed<T, K>
where
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fynix::keyed(key, build)
}

/// See [`fynix::each`].
pub fn each<T, I, K>(
    items: Signal<Vec<I>>,
    key: fn(&I) -> K,
    build: impl Fn(&I) -> fynix::AnyView<crate::Bevy, T>
    + Send
    + Sync
    + 'static,
) -> Each<T, I, K>
where
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fynix::each(items, key, build)
}

/// A read of the world still waiting for its change check.
pub type Derived<T> = fynix::Derived<World, T>;

/// A read of whatever `read` returns, to be given its change check
/// with [`when`](fynix::Derived::when). It fixes the world's
/// type, so a closure can use its argument at once.
pub fn derived<T>(
    read: impl Fn(&World) -> T + Send + Sync + 'static,
) -> Derived<T> {
    fynix::derived(read)
}

/// A check that fires at every update, for a read with no source to
/// watch.
pub fn every_frame()
-> impl FnMut(&World) -> bool + Send + Sync + 'static {
    |_| true
}

/// The change tick a check saw last.
#[derive(Default)]
struct Seen {
    tick: Option<Tick>,
    /// Whether a write could still land on `tick`.
    open: bool,
}

impl Seen {
    /// Whether `tick` differs from the one seen before, and takes it
    /// as seen. `None` is an absent source.
    fn fires(&mut self, world: &World, tick: Option<Tick>) -> bool {
        let fires = self.open || self.tick != tick;
        self.tick = tick;
        // A write made without a system running in between gets the
        // same tick as the one seen, and would go unnoticed.
        self.open = tick == Some(world.read_change_tick());
        fires
    }
}

/// A signal reading the resource `R`, re-read when `R` changes.
/// `R` has to be in the world whenever the signal is read.
pub fn resource<R: Resource, T>(
    read: impl Fn(&R) -> T + Send + Sync + 'static,
) -> Signal<T> {
    let mut seen = Seen::default();
    Signal::new(
        move |world: &World| read(world.resource::<R>()),
        move |world: &World| {
            let tick = world
                .get_resource_change_ticks::<R>()
                .map(|ticks| ticks.changed);
            seen.fires(world, tick)
        },
    )
}

/// A signal reading the component `C` of `entity`, re-read when `C`
/// changes, or is added or removed. `read` gets `None` while `C` is
/// absent.
pub fn component<C: Component, T>(
    entity: Entity,
    read: impl Fn(Option<&C>) -> T + Send + Sync + 'static,
) -> Signal<T> {
    let mut seen = Seen::default();
    Signal::new(
        move |world: &World| read(world.get::<C>(entity)),
        move |world: &World| {
            let tick = world
                .get_entity(entity)
                .ok()
                .and_then(|entity| entity.get_change_ticks::<C>())
                .map(|ticks| ticks.changed);
            seen.fires(world, tick)
        },
    )
}

/// What runs with a value the user asked for, to write it into the
/// world.
pub type Write<T> = Box<dyn Fn(&mut World, T) + Send + Sync>;

/// A value a control both shows and writes: what it shows, and what
/// runs when the user asks for another. See [`bind!`](crate::bind).
pub struct Binding<T> {
    pub value: Prop<T>,
    pub write: Write<T>,
}

impl<T: 'static> Binding<T> {
    /// A binding showing `value` and running `write` with what the
    /// user asks for, for a write that does more than set one field.
    pub fn new(
        value: impl Into<Prop<T>>,
        write: impl Fn(&mut World, T) + Send + Sync + 'static,
    ) -> Self {
        Self {
            value: value.into(),
            write: Box::new(write),
        }
    }

    /// This, shown as a `U`: `to` turns what it holds into what is
    /// shown, and `from` turns what the user asks for back.
    pub fn map<U>(
        self,
        to: impl Fn(T) -> U + Send + Sync + 'static,
        from: impl Fn(U) -> T + Send + Sync + 'static,
    ) -> Binding<U> {
        let Self { value, write } = self;
        Binding {
            value: value.map(to),
            write: Box::new(move |world, asked| {
                write(world, from(asked))
            }),
        }
    }
}

/// A [`Binding`] to a field, named once.
///
/// Of a resource: `bind!(Shape, radius)`, `bind!(Page, 0)` or
/// `bind!(Shape, at.x)`. It is re-read when the resource changes,
/// which has to be in the world.
///
/// Of a component of an entity: `bind!(entity => Health, current)`.
/// It is re-read when the component changes, or is added or removed.
/// While the component is absent it reads as the default and drops
/// writes, as an inspector may for a moment while what it shows goes
/// away.
#[macro_export]
macro_rules! bind {
    ($resource:ty, $($field:tt).+) => {
        $crate::Binding::new(
            $crate::resource::<$resource, _>(|source| {
                ::core::clone::Clone::clone(&source.$($field).+)
            }),
            |world, value| {
                world.resource_mut::<$resource>().$($field).+ = value;
            },
        )
    };
    ($entity:expr => $component:ty, $($field:tt).+) => {{
        let entity = $entity;
        $crate::Binding::new(
            $crate::component::<$component, _>(entity, |source| {
                source
                    .map(|source| {
                        ::core::clone::Clone::clone(&source.$($field).+)
                    })
                    .unwrap_or_default()
            }),
            move |world, value| {
                if let Some(mut source) =
                    world.get_mut::<$component>(entity)
                {
                    source.$($field).+ = value;
                }
            },
        )
    }};
}

#[cfg(test)]
mod tests {
    use bevy::ecs::component::Component;
    use bevy::ecs::world::World;

    use crate::bind;

    #[derive(Component)]
    struct Health {
        current: f32,
    }

    #[test]
    fn a_component_binding_reads_and_writes_its_field() {
        let mut world = World::new();
        let entity = world.spawn(Health { current: 3.0 }).id();
        let binding = bind!(entity => Health, current);
        assert_eq!(binding.value.get(&world), Some(3.0));

        (binding.write)(&mut world, 5.0);
        assert_eq!(world.get::<Health>(entity).unwrap().current, 5.0);

        // Gone, it reads as the default and a write goes nowhere.
        world.entity_mut(entity).remove::<Health>();
        assert_eq!(binding.value.get(&world), Some(0.0));
        (binding.write)(&mut world, 7.0);
        assert!(world.get::<Health>(entity).is_none());
    }
}
