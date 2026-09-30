use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;

use lenz::{Cursor, FieldId, FieldPath};

use crate::backend::Backend;
use crate::layer::{Layer, Live};
use crate::mounted::{Group, Mounted};
use crate::prop::Prop;
use crate::rules::{Condition, RuleKind, ScopeEntry, When};
use crate::transition::{Curve, Motion, MotionTokens};
use crate::view::{Element, Styled, View};

/// One rule restyling a `V`, with the theme in hand.
type Rule<V, T> = Box<dyn Fn(V, &T) -> V + Send + Sync>;

/// Which set rules in force could decide a field, for telling where a
/// value came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    /// The scope depth of each path rule naming the field, outermost
    /// first. Unless the call site set it, the last one decided it.
    pub named: Vec<usize>,
    /// Closure rules in force for the same kind of view. Any of them
    /// could have set it too, and nothing can tell which.
    pub opaque: usize,
}

/// What rules set now are marked with, by the blocks they are set in.
struct Marks<B: Backend> {
    root_only: bool,
    default: bool,
    when: Option<When<B>>,
}

impl<B: Backend> Clone for Marks<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Backend> Copy for Marks<B> {}

impl<B: Backend> Default for Marks<B> {
    fn default() -> Self {
        Self {
            root_only: false,
            default: false,
            when: None,
        }
    }
}

pub struct Cx<'a, B: Backend, T> {
    pub world: &'a mut B::World,
    theme: &'a T,
    mounted: &'a mut Mounted<B, T>,
    parent: Option<B::Node>,
    /// The rules in force, outermost first.
    entries: Vec<ScopeEntry<B>>,
    /// The innermost scope's depth. The root's is 0.
    depth: usize,
    /// The group structural views built now belong to.
    owner: Option<Group>,
    marks: Marks<B>,
}

impl<B: Backend, T> Drop for Cx<'_, B, T> {
    fn drop(&mut self) {
        for entry in self.entries.drain(..) {
            self.mounted.rules.release(entry.key);
        }
    }
}

impl<'a, B: Backend, T: 'static> Cx<'a, B, T> {
    pub fn new(
        world: &'a mut B::World,
        theme: &'a T,
        mounted: &'a mut Mounted<B, T>,
    ) -> Self {
        Self {
            world,
            theme,
            mounted,
            parent: None,
            entries: Vec::new(),
            depth: 0,
            owner: None,
            marks: Marks::default(),
        }
    }

    /// A context with `capture` in force, for building again after the
    /// original is gone.
    pub(crate) fn seeded(
        world: &'a mut B::World,
        theme: &'a T,
        mounted: &'a mut Mounted<B, T>,
        capture: &[ScopeEntry<B>],
        owner: Group,
    ) -> Self {
        for entry in capture {
            mounted.rules.retain(entry.key);
        }
        Self {
            world,
            theme,
            mounted,
            parent: None,
            entries: capture.to_vec(),
            depth: capture.last().map_or(0, |entry| entry.depth),
            owner: Some(owner),
            marks: Marks::default(),
        }
    }

    /// The rules in force now, each referred to once more.
    pub(crate) fn capture(&mut self) -> Vec<ScopeEntry<B>> {
        for entry in &self.entries {
            self.mounted.rules.retain(entry.key);
        }
        self.entries.clone()
    }

    pub(crate) fn owner(&self) -> Option<Group> {
        self.owner
    }

    pub(crate) fn set_owner(&mut self, owner: Option<Group>) {
        self.owner = owner;
    }

    pub(crate) fn mounted(&mut self) -> &mut Mounted<B, T> {
        self.mounted
    }

    pub fn theme(&self) -> &'a T {
        self.theme
    }

    /// Keeps `element` in step with the world, as the one on `node`.
    pub(crate) fn mount<E: Element<B, T>>(
        &mut self,
        node: B::Node,
        element: E,
        snapshot: E::Snapshot,
        live: Live<B, E, E::Snapshot>,
    ) {
        self.mounted
            .mount(self.world, node, element, snapshot, live);
    }

    /// Where a view built now hangs. `None` at the root.
    pub fn parent(&self) -> Option<B::Node> {
        self.parent
    }

    /// A new, empty node where a view built now hangs. It is the root
    /// of every rule set since the last node was spawned.
    pub fn spawn(&mut self) -> B::Node {
        let node = B::spawn(self.world, self.parent);
        for entry in &mut self.entries {
            entry.root.get_or_insert(node);
        }
        node
    }

    /// Builds `view` where a view built now hangs.
    pub fn build(&mut self, view: impl View<B, T>) -> B::Node {
        view.build(self)
    }

    /// Runs `build` with views hanging under `parent`.
    pub fn under<R>(
        &mut self,
        parent: B::Node,
        build: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.parent.replace(parent);
        let built = build(self);
        self.parent = outer;
        built
    }

    /// Runs `build` in a scope of its own: rules it sets end with it.
    pub fn scope<R>(
        &mut self,
        build: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.depth += 1;
        let built = build(self);
        while let Some(entry) =
            self.entries.pop_if(|entry| entry.depth == self.depth)
        {
            self.mounted.rules.release(entry.key);
        }
        self.depth -= 1;
        built
    }

    /// Runs `set` with the rules it sets marked by `mark`.
    fn marked(
        &mut self,
        mark: impl FnOnce(&mut Marks<B>),
        set: impl FnOnce(&mut Self),
    ) {
        let outer = self.marks;
        mark(&mut self.marks);
        set(self);
        self.marks = outer;
    }

    /// Runs `set` with the rules it sets holding only while the root of
    /// the view they are set for is in the state `C`.
    ///
    /// On that root they beat its call site, as a state rule does. On a
    /// view under it they only fill what that view's call site left
    /// unset, as a set rule does. Either way they beat rules that do
    /// not wait on a state.
    pub fn when<C: Condition<B>>(
        &mut self,
        set: impl FnOnce(&mut Self),
    ) {
        self.marked(|marks| marks.when = Some(When::of::<C>()), set);
    }

    /// Runs `set` with the rules it sets reaching only the root of the
    /// view they are set for: the first node spawned after them.
    pub fn root(&mut self, set: impl FnOnce(&mut Self)) {
        self.marked(|marks| marks.root_only = true, set);
    }

    /// Runs `set` with the rules it sets weaker than any that are not,
    /// wherever they are: a composite's defaults, which an app's rules
    /// should restyle.
    pub fn defaults(&mut self, set: impl FnOnce(&mut Self)) {
        self.marked(|marks| marks.default = true, set);
    }

    /// Restyles every `V` built from here to the end of the scope.
    /// A call-site value still beats it.
    pub fn set<V: Styled>(
        &mut self,
        rule: impl Fn(V, &T) -> V + Send + Sync + 'static,
    ) {
        self.push::<V>(RuleKind::Set, None, Box::new(rule));
    }

    fn push<V: Styled>(
        &mut self,
        kind: RuleKind,
        field: Option<FieldId>,
        rule: Rule<V, T>,
    ) {
        let key = self.mounted.rules.insert(rule);
        self.entry(TypeId::of::<V>(), kind, key, field);
    }

    fn entry(
        &mut self,
        view: TypeId,
        kind: RuleKind,
        key: typarena::type_pool::PoolKey,
        field: Option<FieldId>,
    ) {
        self.entries.push(ScopeEntry {
            view,
            kind,
            key,
            field,
            depth: self.depth,
            root: None,
            root_only: self.marks.root_only,
            default: self.marks.default,
            when: self.marks.when,
        });
    }

    /// The rules for a `V` that hold whatever the state, of `kind`,
    /// defaults first, then outermost first.
    fn rules<V: Styled>(
        &self,
        kind: RuleKind,
    ) -> impl Iterator<Item = &Rule<V, T>> {
        let matching = move |default: bool| {
            self.entries.iter().filter(move |entry| {
                entry.kind == kind
                    && entry.view == TypeId::of::<V>()
                    && entry.when.is_none()
                    && entry.default == default
                    && entry.reaches()
            })
        };
        matching(true).chain(matching(false)).filter_map(|entry| {
            self.mounted.rules.get::<Rule<V, T>>(&entry.key)
        })
    }

    /// What the rules waiting on a state would set on an `E` resolved
    /// now, weakest first: rules from ancestors before rules on the
    /// element itself, and defaults before the rest.
    ///
    /// Among rules from ancestors the inner wins, as with set rules.
    /// Among rules on the element itself the outer wins: it was
    /// written later in a chain (`.when(a).when(b)`), or by the call
    /// site around a composite's own.
    pub(crate) fn layers<E: Styled>(&self) -> Vec<Layer<B, E>> {
        let mut layers = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.kind != RuleKind::Motion
                    && entry.view == TypeId::of::<E>()
                    && entry.reaches()
            })
            .filter_map(|(index, entry)| {
                let when = entry.when?;
                let rule = self
                    .mounted
                    .rules
                    .get::<Rule<E, T>>(&entry.key)?;
                let own = entry.root.is_none();
                let order = if own {
                    -(index as isize)
                } else {
                    index as isize
                };
                Some((
                    (own, !entry.default, order),
                    Layer {
                        view: rule(E::unset(), self.theme),
                        when,
                        on: entry.root,
                        own,
                    },
                ))
            })
            .collect::<Vec<_>>();
        layers.sort_by_key(|(rank, _)| *rank);
        layers.into_iter().map(|(_, layer)| layer).collect()
    }

    /// The curve a transition rule in force gives an element resolved
    /// now: the innermost that is not a default, else the innermost
    /// default.
    pub(crate) fn curve(&self) -> Option<Curve> {
        let innermost = |default: bool| {
            self.entries
                .iter()
                .rev()
                .filter(|entry| {
                    entry.kind == RuleKind::Motion
                        && entry.default == default
                        && entry.reaches()
                })
                .find_map(|entry| {
                    self.mounted
                        .rules
                        .get::<Curve>(&entry.key)
                        .copied()
                })
        };
        innermost(false).or_else(|| innermost(true))
    }

    /// Makes every element built from here to the end of the scope
    /// travel to new values over the theme's curve for `motion`, if it
    /// says how its values blend.
    pub fn transition(&mut self, motion: Motion)
    where
        T: MotionTokens,
    {
        self.transition_over(self.theme.motion(motion));
    }

    /// As [`transition`](Self::transition), over `curve`.
    pub fn transition_over(&mut self, curve: Curve) {
        let key = self.mounted.rules.insert(curve);
        self.entry(
            TypeId::of::<Curve>(),
            RuleKind::Motion,
            key,
            None,
        );
    }

    /// Sets the field `path` names to `value`, on every view `path`
    /// starts from, from here to the end of the scope. A set rule
    /// like [`set`](Self::set), but one that says which field it sets.
    ///
    /// The path may reach into a composite's own parts:
    /// `Card::cursor().title().size()` sets the size of a card's
    /// title, and of nothing else.
    pub fn set_field<P, X>(&mut self, path: Cursor<P>, value: X)
    where
        P: FieldPath<Target = Prop<B::World, X>>,
        P::Source: Styled,
        X: Clone + Send + Sync + 'static,
    {
        self.set_field_with(path, move |_| value.clone());
    }

    /// As [`set_field`](Self::set_field), with the value read from the
    /// theme.
    pub fn set_field_with<P, X>(
        &mut self,
        path: Cursor<P>,
        read: impl Fn(&T) -> X + Send + Sync + 'static,
    ) where
        P: FieldPath<Target = Prop<B::World, X>>,
        P::Source: Styled,
        X: Send + Sync + 'static,
    {
        let field = path.key();
        let accessor = path.accessor();
        self.push::<P::Source>(
            RuleKind::Set,
            Some(field),
            Box::new(move |mut view, theme| {
                if let Some(prop) = accessor.get_mut(&mut view) {
                    *prop = Prop::Value(read(theme));
                }
                view
            }),
        );
    }

    /// Which set rules in force for a `V` could decide `field`.
    pub fn trace<V: Styled>(&self, field: FieldId) -> Trace {
        let mut trace = Trace::default();
        let sets = self.entries.iter().filter(|entry| {
            entry.kind == RuleKind::Set
                && entry.view == TypeId::of::<V>()
                && entry.when.is_none()
        });
        for entry in sets {
            match entry.field {
                None => trace.opaque += 1,
                Some(named) if named == field => {
                    if trace.named.last() != Some(&entry.depth) {
                        trace.named.push(entry.depth);
                    }
                }
                Some(_) => {}
            }
        }
        trace
    }

    /// Transforms every `V` built from here to the end of the scope,
    /// after its call site, so unlike a set rule it wins over it.
    pub fn show<V: Styled>(
        &mut self,
        rule: impl Fn(V, &T) -> V + Send + Sync + 'static,
    ) {
        self.push::<V>(RuleKind::Show, None, Box::new(rule));
    }

    /// `view` with the rules in force applied: set rules fill what its
    /// call site left unset, defaults first, then outer scopes before
    /// inner, then show rules transform the result. Rules waiting on a
    /// state are left out: an element puts them on top while mounted.
    pub fn resolve<V: Styled>(&self, view: V) -> V {
        let theme = self.theme;
        let below = self
            .rules::<V>(RuleKind::Set)
            .fold(V::unset(), |below, rule| rule(below, theme));
        self.rules::<V>(RuleKind::Show)
            .fold(view.over(below), |view, rule| rule(view, theme))
    }
}
