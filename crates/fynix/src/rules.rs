//! The rules in force, stored once each and counted by whoever still
//! needs them.

use core::any::TypeId;

use hashbrown::HashMap;
use lenz::FieldId;
use typarena::type_pool::{PoolKey, TypePool};

use crate::backend::Backend;

/// Whether a rule fills what the call site left unset, transforms the
/// result after it, or names the curve values travel over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleKind {
    Set,
    Show,
    Motion,
}

/// A state a node can be in, which rules can be made to hold under.
pub trait Condition<B: Backend> {
    /// Whether `node` is in the state now.
    fn holds(world: &B::World, node: B::Node) -> bool;

    /// Makes the backend report `node` to
    /// [`Mounted::mark_dirty`](crate::Mounted::mark_dirty) whenever the
    /// state starts or stops holding. Called once per element reading
    /// it, so it must not add a second report for a node it already
    /// watches.
    fn watch(world: &mut B::World, node: B::Node);
}

/// A [`Condition`], its type erased.
pub(crate) struct When<B: Backend> {
    pub holds: fn(&B::World, B::Node) -> bool,
    pub watch: fn(&mut B::World, B::Node),
}

impl<B: Backend> When<B> {
    pub fn of<C: Condition<B>>() -> Self {
        Self {
            holds: C::holds,
            watch: C::watch,
        }
    }
}

impl<B: Backend> Clone for When<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Backend> Copy for When<B> {}

/// One rule in a scope stack: the kind of view it restyles, where it is
/// stored, how deep the scope that set it was, and what it waits on.
pub(crate) struct ScopeEntry<B: Backend> {
    pub view: TypeId,
    pub kind: RuleKind,
    pub key: PoolKey,
    /// The field a path rule sets.
    pub field: Option<FieldId>,
    pub depth: usize,
    /// The first node spawned after the rule was set, once there is
    /// one: the root of the view it was set for.
    pub root: Option<B::Node>,
    /// Whether the rule reaches that root alone.
    pub root_only: bool,
    /// Whether the rule is a composite's default, weaker than any rule
    /// that is not.
    pub default: bool,
    /// The state the rule holds under, read on `root`.
    pub when: Option<When<B>>,
}

impl<B: Backend> Clone for ScopeEntry<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Backend> Copy for ScopeEntry<B> {}

impl<B: Backend> ScopeEntry<B> {
    /// Whether the rule reaches a view resolved now.
    pub fn reaches(&self) -> bool {
        !self.root_only || self.root.is_none()
    }
}

/// Every rule any live scope or capture can still reach. A rule is kept
/// while its count says something refers to it.
#[derive(Default)]
pub struct RuleArena {
    pool: TypePool,
    counts: HashMap<PoolKey, u32>,
}

impl RuleArena {
    /// How many rules are stored.
    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Stores `rule`, referred to once.
    pub(crate) fn insert<R: Send + Sync + 'static>(
        &mut self,
        rule: R,
    ) -> PoolKey {
        let key = self.pool.insert(rule);
        self.counts.insert(key, 1);
        key
    }

    pub(crate) fn get<R: 'static>(
        &self,
        key: &PoolKey,
    ) -> Option<&R> {
        self.pool.get::<R>(key)
    }

    /// Adds a reference to the rule at `key`.
    pub(crate) fn retain(&mut self, key: PoolKey) {
        if let Some(count) = self.counts.get_mut(&key) {
            *count += 1;
        }
    }

    /// Drops a reference to the rule at `key`, and the rule with the
    /// last one.
    pub(crate) fn release(&mut self, key: PoolKey) {
        let Some(count) = self.counts.get_mut(&key) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            self.counts.remove(&key);
            self.pool.dyn_remove(&key);
        }
    }
}
