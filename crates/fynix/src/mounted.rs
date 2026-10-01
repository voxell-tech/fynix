//! Elements whose props can change after they are built, kept in step
//! with the world.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::any::TypeId;
use core::marker::PhantomData;
use core::time::Duration;

use hashbrown::{HashMap, HashSet};
use typarena::type_table::TypeTable;

use crate::backend::Backend;
use crate::layer::Live;
use crate::rules::RuleArena;
use crate::transition::Curve;
use crate::view::Element;

/// A set of structural views built together, dropped together.
pub(crate) type Group = u32;

/// A view that builds and drops parts of the tree as the world
/// changes.
pub(crate) trait Structure<B: Backend, T>:
    Send + Sync
{
    /// Rebuilds what the world's changes call for.
    fn update(
        &mut self,
        id: Group,
        world: &mut B::World,
        theme: &T,
        mounted: &mut Mounted<B, T>,
    );

    /// Lets go of the rules it captured.
    fn release(&mut self, rules: &mut RuleArena);

    /// The groups it built views in besides its own: an `each` has
    /// one per row. They are dropped with it.
    fn groups(&self, _groups: &mut Vec<Group>) {}
}

/// One registered structural view.
struct Slot<B: Backend, T> {
    /// The group the slot was built in.
    parent: Option<Group>,
    container: B::Node,
    structure: Box<dyn Structure<B, T>>,
}

/// What one update runs with.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tick {
    /// Time since the last update, what transitions advance by.
    pub delta: Duration,
    /// Whether every transition finishes at once, and every step of
    /// a view coming in or leaving, its fade and the growing or
    /// shrinking of its space, is skipped.
    pub reduced_motion: bool,
}

/// Brings every mounted element of one kind up to date.
type UpdateFn<B, T> = fn(
    &mut TypeTable<<B as Backend>::Node>,
    &mut <B as Backend>::World,
    &T,
    Tick,
);

/// How many elements of one kind are mounted.
type CountFn<B> = fn(&TypeTable<<B as Backend>::Node>) -> usize;

/// Acts on the mounted element of one kind on a node, without naming
/// the kind.
type NodeFn<B> =
    fn(&mut TypeTable<<B as Backend>::Node>, <B as Backend>::Node);

/// What [`Mounted`] does to a node's element, for the element's kind.
struct Hooks<B: Backend> {
    mark: NodeFn<B>,
    remove: NodeFn<B>,
    curve: fn(&TypeTable<B::Node>, B::Node) -> Option<Curve>,
}

/// A dropped view's root, kept while it animates out: first its
/// elements travel to their leaving state, then the space it takes
/// collapses, then it is despawned.
struct Leaving {
    curve: Curve,
    elapsed: Duration,
    collapsing: bool,
    /// How much of the space was already gone when the collapse
    /// began: 0 unless the view left before it was whole.
    from: f32,
}

/// A built view's root, kept while it comes in: it waits for the
/// views that left in its place, then its space expands, then it is
/// released and its elements travel from their entering state.
struct Arriving<N> {
    curve: Curve,
    elapsed: Duration,
    /// The roots whose leaving it waits for.
    after: Vec<N>,
    /// Whether a layout has had the chance to run since it was held.
    settled: bool,
    expanding: bool,
}

/// How far `elapsed` is through `duration`, which it is under.
fn fraction(elapsed: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.0;
    }
    elapsed.as_secs_f32() / duration.as_secs_f32()
}

impl<B: Backend> Clone for Hooks<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Backend> Copy for Hooks<B> {}

/// An effect's check and run, called with its node at every update.
pub(crate) type EffectFn<B> = Box<
    dyn FnMut(&mut <B as Backend>::World, <B as Backend>::Node)
        + Send
        + Sync,
>;

/// Every mounted element built with the theme `T`: one column per
/// kind of element, keyed by its node, and one update per kind to
/// walk it.
pub struct Mounted<B: Backend, T> {
    table: TypeTable<B::Node>,
    updates: Vec<UpdateFn<B, T>>,
    counts: Vec<CountFn<B>>,
    kinds: HashSet<TypeId>,
    hooks: HashMap<B::Node, Hooks<B>>,
    /// The elements whose state rules are read on each node, besides
    /// the node's own element.
    readers: HashMap<B::Node, Vec<B::Node>>,
    /// The roots of dropped views still animating out.
    leaving: HashMap<B::Node, Leaving>,
    /// The roots of built views still coming in.
    arriving: HashMap<B::Node, Arriving<B::Node>>,
    /// What each node does whenever a bound value changes, in the
    /// order it was asked to.
    effects: HashMap<B::Node, Vec<EffectFn<B>>>,
    /// Structural views by id, parents before the views they build.
    slots: BTreeMap<Group, Slot<B, T>>,
    /// The slot of each container node.
    containers: HashMap<B::Node, Group>,
    next_group: Group,
    pub(crate) rules: RuleArena,
}

impl<B: Backend, T> Default for Mounted<B, T> {
    fn default() -> Self {
        Self {
            table: TypeTable::new(),
            updates: Vec::new(),
            counts: Vec::new(),
            kinds: HashSet::new(),
            hooks: HashMap::new(),
            readers: HashMap::new(),
            leaving: HashMap::new(),
            arriving: HashMap::new(),
            effects: HashMap::new(),
            slots: BTreeMap::new(),
            containers: HashMap::new(),
            next_group: 0,
            rules: RuleArena::default(),
        }
    }
}

impl<B: Backend, T: 'static> Mounted<B, T> {
    /// Keeps `element` in step with the world as the one on `node`.
    pub(crate) fn mount<E: Element<B, T>>(
        &mut self,
        world: &mut B::World,
        node: B::Node,
        mut element: E,
        shown: E::Shown,
        mut live: Live<B, E>,
    ) {
        if self.kinds.insert(TypeId::of::<E>()) {
            self.updates.push(update_kind::<B, T, E>);
            self.counts.push(|table| table.len::<Mount<B, T, E>>());
        }
        self.hooks.insert(
            node,
            Hooks {
                mark: mark::<B, T, E>,
                remove: remove::<B, T, E>,
                curve: curve::<B, T, E>,
            },
        );
        // Checks often fire on their first call, which the first
        // write already covers.
        element.changed(world);
        live.changed::<T>(world);
        for on in live.read_on(node) {
            let readers = self.readers.entry(on).or_default();
            if !readers.contains(&node) {
                readers.push(node);
            }
        }
        B::on_mount(world, node);
        element.on_mounted(world, node);
        self.table.insert(
            node,
            Mount::<B, T, E> {
                element,
                live,
                shown,
                moving: false,
                dirty: false,
                marker: PhantomData,
            },
        );
    }

    /// Runs `run` at every update until `node` is unmounted. See
    /// [`Cx::effect`](crate::Cx::effect).
    pub(crate) fn add_effect(
        &mut self,
        node: B::Node,
        run: EffectFn<B>,
    ) {
        self.effects.entry(node).or_default().push(run);
    }

    /// Runs every effect whose bound value changed. A backend calls
    /// it before the rest of an update, so what an effect writes is
    /// read by that same update.
    pub fn run_effects(&mut self, world: &mut B::World) {
        for (&node, effects) in &mut self.effects {
            for run in effects {
                run(world, node);
            }
        }
    }

    /// How many effects are kept.
    pub fn effects_len(&self) -> usize {
        self.effects.values().map(Vec::len).sum()
    }

    /// Makes the element on `node`, and every element with a state
    /// rule read on `node`, re-read at the next update, whatever
    /// their checks say.
    pub fn mark_dirty(&mut self, node: B::Node) {
        if let Some(hooks) = self.hooks.get(&node) {
            (hooks.mark)(&mut self.table, node);
        }
        let Some(readers) = self.readers.get_mut(&node) else {
            return;
        };
        // Readers rebuilt away since are dropped here.
        readers.retain(|reader| self.hooks.contains_key(reader));
        for &reader in readers.iter() {
            (self.hooks[&reader].mark)(&mut self.table, reader);
        }
    }

    /// Drops the element on `node`, and the structural view whose
    /// container it is.
    pub fn unmount(&mut self, node: B::Node) {
        if let Some(hooks) = self.hooks.remove(&node) {
            (hooks.remove)(&mut self.table, node);
        }
        self.readers.remove(&node);
        self.leaving.remove(&node);
        self.arriving.remove(&node);
        self.effects.remove(&node);
        if let Some(id) = self.containers.remove(&node) {
            let mut groups = vec![id];
            if let Some(slot) = self.slots.remove(&id) {
                self.forget(slot, &mut groups);
            }
            for group in groups {
                self.drop_group(group);
            }
        }
    }

    /// The curve the element on `node` travels over, if it does.
    fn curve_of(&self, node: B::Node) -> Option<Curve> {
        self.hooks
            .get(&node)
            .and_then(|hooks| (hooks.curve)(&self.table, node))
    }

    /// Takes out the view whose root is `node`: animated out when its
    /// root element travels over a curve, despawned at once
    /// otherwise. Whether it is animating out.
    pub(crate) fn leave(
        &mut self,
        world: &mut B::World,
        node: B::Node,
    ) -> bool {
        let arriving = self.arriving.remove(&node);
        // A view still held was never seen.
        let seen = arriving.as_ref().is_none_or(|a| a.expanding);
        let Some(curve) = self.curve_of(node).filter(|_| seen) else {
            B::despawn(world, node);
            return false;
        };
        B::leave(world, node);
        // A view that left while coming in is already invisible, and
        // gives back the space it had got.
        let from = arriving.map(|arriving| {
            1.0 - (arriving.curve.ease)(fraction(
                arriving.elapsed,
                arriving.curve.duration,
            ))
        });
        self.leaving.insert(
            node,
            Leaving {
                curve,
                elapsed: Duration::ZERO,
                collapsing: from.is_some(),
                from: from.unwrap_or(0.0),
            },
        );
        true
    }

    /// Whether `node` is the root of a view still animating out.
    pub fn is_leaving(&self, node: B::Node) -> bool {
        self.leaving.contains_key(&node)
    }

    /// Brings in the view whose root is `node`, just built: held
    /// out of the layout until the views in `after`, which left in
    /// its place, are gone, then its space expands. A view whose root
    /// element does not travel over a curve is left whole.
    pub(crate) fn enter(
        &mut self,
        world: &mut B::World,
        node: B::Node,
        after: &[B::Node],
    ) {
        let Some(curve) = self.curve_of(node) else {
            return;
        };
        B::hold(world, node);
        self.arriving.insert(
            node,
            Arriving {
                curve,
                elapsed: Duration::ZERO,
                after: after.to_vec(),
                settled: false,
                expanding: false,
            },
        );
    }

    /// Whether `node` is the root of a view still coming in.
    pub fn is_entering(&self, node: B::Node) -> bool {
        self.arriving.contains_key(&node)
    }

    /// Moves every leaving view on by `tick`.
    fn update_leaving(&mut self, world: &mut B::World, tick: Tick) {
        self.leaving.retain(|&node, leaving| {
            let duration = leaving.curve.duration;
            leaving.elapsed += tick.delta;
            if tick.reduced_motion {
                B::despawn(world, node);
                return false;
            }
            if !leaving.collapsing {
                if leaving.elapsed < duration {
                    return true;
                }
                // What is left of the tick goes to the collapse.
                leaving.collapsing = true;
                leaving.elapsed -= duration;
                B::collapse(world, node, 0.0);
            }
            if leaving.elapsed >= duration {
                B::collapse(world, node, 1.0);
                B::despawn(world, node);
                return false;
            }
            let eased = (leaving.curve.ease)(fraction(
                leaving.elapsed,
                duration,
            ));
            let progress =
                leaving.from + (1.0 - leaving.from) * eased;
            B::collapse(world, node, progress);
            true
        });
    }

    /// Moves every view coming in on by `tick`.
    fn update_arriving(&mut self, world: &mut B::World, tick: Tick) {
        let leaving = &self.leaving;
        self.arriving.retain(|&node, arriving| {
            if tick.reduced_motion {
                B::release(world, node);
                return false;
            }
            if !arriving.expanding {
                // The first tick is the one that built it, before any
                // layout could run.
                if !core::mem::replace(&mut arriving.settled, true)
                    || arriving
                        .after
                        .iter()
                        .any(|gone| leaving.contains_key(gone))
                {
                    return true;
                }
                arriving.expanding = true;
                B::collapse(world, node, 1.0);
                return true;
            }
            arriving.elapsed += tick.delta;
            let duration = arriving.curve.duration;
            if arriving.elapsed >= duration {
                B::collapse(world, node, 0.0);
                B::release(world, node);
                return false;
            }
            let eased = (arriving.curve.ease)(fraction(
                arriving.elapsed,
                duration,
            ));
            B::collapse(world, node, 1.0 - eased);
            true
        });
    }

    /// A group for structural views built together.
    pub(crate) fn new_group(&mut self) -> Group {
        self.next_group += 1;
        self.next_group
    }

    /// Registers `structure` as the one with `container`, under the
    /// group `id` that `new_group` gave.
    pub(crate) fn register(
        &mut self,
        id: Group,
        parent: Option<Group>,
        container: B::Node,
        structure: Box<dyn Structure<B, T>>,
    ) {
        self.containers.insert(container, id);
        self.slots.insert(
            id,
            Slot {
                parent,
                container,
                structure,
            },
        );
    }

    /// Drops every structural view built in `group`, and in the
    /// groups those made.
    pub(crate) fn drop_group(&mut self, group: Group) {
        let mut dead = vec![group];
        let mut next = 0;
        while let Some(&group) = dead.get(next) {
            next += 1;
            let built = self
                .slots
                .iter()
                .filter(|(_, slot)| slot.parent == Some(group))
                .map(|(&id, _)| id)
                .collect::<Vec<_>>();
            for id in built {
                if let Some(slot) = self.slots.remove(&id) {
                    self.forget(slot, &mut dead);
                }
                dead.push(id);
            }
        }
    }

    /// Lets go of what `slot` holds, and adds the groups it built
    /// views in to `dead`, for those to be dropped too.
    fn forget(
        &mut self,
        mut slot: Slot<B, T>,
        dead: &mut Vec<Group>,
    ) {
        self.containers.remove(&slot.container);
        slot.structure.release(&mut self.rules);
        slot.structure.groups(dead);
    }

    /// Rebuilds what the world's changes call for, in every `keyed`
    /// and `each` view. Views they build are checked from the next
    /// update on.
    pub fn update_structure(
        &mut self,
        world: &mut B::World,
        theme: &T,
    ) {
        let ids = self.slots.keys().copied().collect::<Vec<_>>();
        for id in ids {
            // Gone when the rebuild of a view before it dropped it.
            let Some(mut slot) = self.slots.remove(&id) else {
                continue;
            };
            slot.structure.update(id, world, theme, self);
            self.slots.insert(id, slot);
        }
    }

    /// Writes what changed to every mounted element that reports a
    /// change or is marked dirty, and keeps every transition under
    /// way advancing.
    pub fn update_elements(
        &mut self,
        world: &mut B::World,
        theme: &T,
        tick: Tick,
    ) {
        for update in &self.updates {
            update(&mut self.table, world, theme, tick);
        }
        self.update_leaving(world, tick);
        self.update_arriving(world, tick);
    }

    /// The rules stored for the scopes and captures alive.
    pub fn rules(&self) -> &RuleArena {
        &self.rules
    }

    /// How many `keyed` and `each` views are registered.
    pub fn structure_len(&self) -> usize {
        self.slots.len()
    }

    pub fn len(&self) -> usize {
        self.counts.iter().map(|count| count(&self.table)).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

struct Mount<B: Backend, T, E: Element<B, T>> {
    element: E,
    live: Live<B, E>,
    /// Each prop's value as written, and where it is heading.
    shown: E::Shown,
    /// Whether any prop is still travelling.
    moving: bool,
    /// Whether to re-read at the next update whatever the checks
    /// say.
    dirty: bool,
    marker: PhantomData<fn() -> (B, T)>,
}

fn update_kind<B: Backend, T: 'static, E: Element<B, T>>(
    table: &mut TypeTable<B::Node>,
    world: &mut B::World,
    theme: &T,
    tick: Tick,
) {
    for (&node, mount) in table.iter_mut::<Mount<B, T, E>>() {
        mount.update(world, node, theme, tick);
    }
}

fn mark<B: Backend, T: 'static, E: Element<B, T>>(
    table: &mut TypeTable<B::Node>,
    node: B::Node,
) {
    if let Some(mount) = table.get_mut::<Mount<B, T, E>>(&node) {
        mount.dirty = true;
    }
}

fn curve<B: Backend, T: 'static, E: Element<B, T>>(
    table: &TypeTable<B::Node>,
    node: B::Node,
) -> Option<Curve> {
    table.get::<Mount<B, T, E>>(&node)?.live.curve
}

fn remove<B: Backend, T: 'static, E: Element<B, T>>(
    table: &mut TypeTable<B::Node>,
    node: B::Node,
) {
    table.remove::<Mount<B, T, E>>(&node);
}

impl<B: Backend, T, E: Element<B, T>> Mount<B, T, E> {
    fn update(
        &mut self,
        world: &mut B::World,
        node: B::Node,
        theme: &T,
        tick: Tick,
    ) {
        let mut dirty = self.element.changed(world)
            | self.live.changed::<T>(world);
        if core::mem::take(&mut self.dirty) {
            // A state changed: only props a state rule sets can
            // differ. Marked for any other reason, every prop is
            // re-read.
            dirty |= match self.live.layered::<T>() {
                0 => u64::MAX,
                layered => layered,
            };
        }
        if dirty == 0 && !self.moving {
            return;
        }
        self.moving = self.live.update(
            &mut self.element,
            &mut self.shown,
            dirty,
            world,
            node,
            theme,
            tick,
        );
    }
}
