use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;

use hashbrown::HashMap;
use lenz::{Cursor, FieldId, FieldPath};

use crate::backend::Backend;
use crate::layer::{Layer, Live};
use crate::mounted::{Group, Mounted};
use crate::prop::Prop;
use crate::rules::{Condition, RuleKind, ScopeEntry, When};
use crate::transition::{Curve, Motion, MotionTokens, Travel};
use crate::view::{AnyView, Element, Matched, Styled, View};

/// How many replacements can be built one inside another before a
/// show rule is taken to match its own replacement forever.
const MAX_SHOW_DEPTH: usize = 64;

/// One rule restyling a `V`, with the theme in hand.
type Rule<V, T> = Box<dyn Fn(V, &T) -> V + Send + Sync>;

/// One rule replacing an `E`, or handing back one it does not take,
/// with the theme in hand.
type ShowRule<B, E, T> = Box<
    dyn Fn(Matched<E>, &T) -> Result<AnyView<B, T>, Matched<E>>
        + Send
        + Sync,
>;

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
    /// The kind of element each show and show-set rule in force
    /// matches.
    show_kinds: Vec<TypeId>,
    /// How many replacements are being built, one inside another.
    show_depth: usize,
    /// The show-set rules that held where each node was built, for
    /// the views built under it.
    show_sets: HashMap<B::Node, Vec<ScopeEntry<B>>>,
}

impl<B: Backend, T> Drop for Cx<'_, B, T> {
    fn drop(&mut self) {
        let held = self.show_sets.values().flatten();
        for entry in self.entries.iter().chain(held) {
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
            show_kinds: Vec::new(),
            show_depth: 0,
            show_sets: HashMap::new(),
        }
    }

    /// A context with `capture` in force, for building again after
    /// the original is gone.
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
            show_kinds: capture
                .iter()
                .filter_map(ScopeEntry::shows)
                .collect(),
            show_depth: 0,
            show_sets: HashMap::new(),
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
        shown: E::Shown,
        live: Live<B, E>,
    ) {
        self.mounted.mount(self.world, node, element, shown, live);
    }

    /// Where a view built now hangs. `None` at the root.
    pub fn parent(&self) -> Option<B::Node> {
        self.parent
    }

    /// Runs `run` on `node` with what `value` holds: once now, and
    /// again whenever a bound value changes to something different,
    /// until the node is unmounted. An unset prop never runs it.
    ///
    /// This is how a composite follows a bound value: an element
    /// reads its own props, and a composite hands one to this.
    pub fn effect<P>(
        &mut self,
        node: B::Node,
        value: Prop<B::World, P>,
        mut run: impl FnMut(&mut B::World, B::Node, &P)
        + Send
        + Sync
        + 'static,
    ) where
        P: PartialEq + Clone + Send + Sync + 'static,
    {
        let mut signal = match value {
            Prop::Unset => return,
            Prop::Value(value) => {
                run(self.world, node, &value);
                return;
            }
            Prop::Bound(signal) => signal,
        };
        // The first check fires on any source, and this run covers
        // it.
        signal.changed(self.world);
        let mut last = signal.get(self.world);
        run(self.world, node, &last);
        self.mounted.add_effect(
            node,
            Box::new(move |world, node| {
                if !signal.changed(world) {
                    return;
                }
                let now = signal.get(world);
                if now != last {
                    run(world, node, &now);
                    last = now;
                }
            }),
        );
    }

    /// A new, empty node where a view built now hangs. It is the root
    /// of every rule set since the last node was spawned.
    pub fn spawn(&mut self) -> B::Node {
        self.spawn_for(None)
    }

    /// As [`spawn`](Self::spawn), for the element whose type is
    /// `kind`.
    pub(crate) fn spawn_for(
        &mut self,
        kind: Option<TypeId>,
    ) -> B::Node {
        let node = B::spawn(self.world, self.parent, kind);
        for entry in &mut self.entries {
            entry.root.get_or_insert(node);
        }
        node
    }

    /// Builds `view` where a view built now hangs.
    pub fn build(&mut self, view: impl View<B, T>) -> B::Node {
        view.build(self)
    }

    /// Runs `build` with views hanging under `parent`, within the
    /// show-set rules for the element on it.
    pub fn under<R>(
        &mut self,
        parent: B::Node,
        build: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.parent.replace(parent);
        let built = match self.show_sets.get(&parent) {
            Some(sets) => {
                let sets = sets.clone();
                let start = self.entries.len();
                self.hold(&sets);
                let built = build(self);
                // Not a scope: rules `build` set stay in force.
                let held =
                    self.entries.drain(start..start + sets.len());
                for entry in held {
                    self.mounted.rules.release(entry.key);
                }
                built
            }
            None => build(self),
        };
        self.parent = outer;
        built
    }

    /// Builds `element` within the show-set rules for an `E` in
    /// force, which then hold under its node too.
    pub(crate) fn build_element<E: Element<B, T>>(
        &mut self,
        element: E,
    ) -> B::Node {
        let mut sets = Vec::new();
        if self.show_kinds.contains(&TypeId::of::<E>()) {
            sets.extend(self.entries.iter().filter(|entry| {
                entry.kind == RuleKind::ShowSet(TypeId::of::<E>())
                    && entry.reaches()
            }));
        }
        if sets.is_empty() {
            return Matched::resolve(element, self).build(self);
        }
        self.scope(|cx| {
            cx.hold(&sets);
            let node = Matched::resolve(element, cx).build(cx);
            for entry in &sets {
                cx.mounted.rules.retain(entry.key);
            }
            cx.show_sets.entry(node).or_default().extend(sets);
            node
        })
    }

    /// Puts the set rules of the show-set rules `sets` in force, in
    /// the innermost scope.
    fn hold(&mut self, sets: &[ScopeEntry<B>]) {
        for entry in sets {
            self.mounted.rules.retain(entry.key);
            self.entries.push(ScopeEntry {
                kind: RuleKind::Set,
                depth: self.depth,
                root: None,
                root_only: false,
                ..*entry
            });
        }
    }

    /// Runs `build` with views hanging at the root, whatever they
    /// were built under. The rules in force still apply.
    pub fn at_root<R>(
        &mut self,
        build: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let outer = self.parent.take();
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
            if let Some(kind) = entry.shows()
                && let Some(at) =
                    self.show_kinds.iter().position(|&of| of == kind)
            {
                self.show_kinds.swap_remove(at);
            }
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

    /// Runs `set` with the rules it sets holding only while the root
    /// of the view they are set for is in the state `C`.
    ///
    /// On that root they beat its call site, as a state rule does. On
    /// a view under it they only fill what that view's call site
    /// left unset, as a set rule does. Either way they beat rules
    /// that do not wait on a state. Show rules cannot be set in it.
    pub fn when<C: Condition<B>>(
        &mut self,
        set: impl FnOnce(&mut Self),
    ) {
        self.marked(|marks| marks.when = Some(When::of::<C>()), set);
    }

    /// Runs `set` with the rules it sets reaching only the root of
    /// the view they are set for: the first node spawned after
    /// them.
    pub fn root(&mut self, set: impl FnOnce(&mut Self)) {
        self.marked(|marks| marks.root_only = true, set);
    }

    /// Runs `set` with the rules it sets weaker than any that are
    /// not, wherever they are: a composite's defaults, which an
    /// app's rules should restyle.
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
        let entry = ScopeEntry {
            view,
            kind,
            key,
            field,
            depth: self.depth,
            root: None,
            root_only: self.marks.root_only,
            default: self.marks.default,
            when: self.marks.when,
        };
        self.show_kinds.extend(entry.shows());
        self.entries.push(entry);
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
    /// Among rules on the element itself the outer scope wins: it was
    /// written later in a chain (`.when(a).when(b)`), or by the call
    /// site around a composite's own. Within one scope the later wins
    /// in both cases, as with set rules.
    pub(crate) fn layers<E: Styled>(&self) -> Vec<Layer<B, E>> {
        let mut layers = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.kind == RuleKind::Set
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
                let (depth, order) = if own {
                    (-(entry.depth as isize), index)
                } else {
                    (0, index)
                };
                Some((
                    (own, !entry.default, depth, order),
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

    /// How a transition rule in force has an element resolved now
    /// travel: the innermost that is not a default, else the
    /// innermost default.
    pub(crate) fn travel(&self) -> Option<Travel> {
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
                        .get::<Travel>(&entry.key)
                        .copied()
                })
        };
        innermost(false).or_else(|| innermost(true))
    }

    /// Makes every element built from here to the end of the scope
    /// travel to new values over the theme's curve for `motion`, if
    /// it says how its values blend.
    pub fn transition(&mut self, motion: Motion)
    where
        T: MotionTokens,
    {
        self.travel_over(Travel {
            curve: self.theme.motion(motion),
            motion: Some(motion),
        });
    }

    /// As [`transition`](Self::transition), over `curve`. Fynix moves
    /// the values itself, whether the backend animates or not.
    pub fn transition_over(&mut self, curve: Curve) {
        self.travel_over(Travel {
            curve,
            motion: None,
        });
    }

    fn travel_over(&mut self, travel: Travel) {
        let key = self.mounted.rules.insert(travel);
        self.entry(
            TypeId::of::<Travel>(),
            RuleKind::Motion,
            key,
            None,
        );
    }

    /// Sets the field `path` names to `value`, on every view `path`
    /// starts from, from here to the end of the scope. A set rule
    /// like [`set`](Self::set), but one that says which field it
    /// sets.
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

    /// As [`set_field`](Self::set_field), with the value read from
    /// the theme.
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

    /// Replaces every element `E` built from here to the end of the
    /// scope with the view `rule` makes of it.
    ///
    /// The rule receives the element with the set rules in force
    /// applied, and what its call site set in
    /// [`Matched::call_mask`]. Built in the replacement, that element
    /// keeps its props and skips this rule and any
    /// later one it went through, and older rules replace it in turn.
    /// Every other `E`, a new one in the replacement too, is replaced
    /// again: a rule that always builds a new `E` panics once
    /// replacements nest 64 deep.
    pub fn show<E: Element<B, T>>(
        &mut self,
        rule: impl Fn(Matched<E>, &T) -> AnyView<B, T>
        + Send
        + Sync
        + 'static,
    ) {
        self.show_where(|_| true, rule);
    }

    /// Restyles every `W` built within an element `E` from here to
    /// the end of the scope, as a set rule set there would: the
    /// element itself, its replacement and what is built under
    /// its node.
    pub fn show_set<E: Element<B, T>, W: Styled>(
        &mut self,
        rule: impl Fn(W, &T) -> W + Send + Sync + 'static,
    ) {
        self.refuse_state();
        let key =
            self.mounted.rules.insert(Box::new(rule) as Rule<W, T>);
        self.entry(
            TypeId::of::<W>(),
            RuleKind::ShowSet(TypeId::of::<E>()),
            key,
            None,
        );
    }

    /// As [`show`](Self::show), for the elements `filter` takes, as
    /// the rule would receive them. An `E` it does not take is left
    /// to the rules before it.
    pub fn show_where<E: Element<B, T>>(
        &mut self,
        filter: impl Fn(&E) -> bool + Send + Sync + 'static,
        rule: impl Fn(Matched<E>, &T) -> AnyView<B, T>
        + Send
        + Sync
        + 'static,
    ) {
        self.refuse_state();
        let key = self.mounted.rules.insert(Box::new(
            move |matched: Matched<E>, theme: &T| {
                if filter(&matched) {
                    Ok(rule(matched, theme))
                } else {
                    Err(matched)
                }
            },
        )
            as ShowRule<B, E, T>);
        self.entry(TypeId::of::<E>(), RuleKind::Show, key, None);
    }

    /// Panics in a [`when`](Self::when) block: a show rule cannot
    /// wait on a state.
    fn refuse_state(&self) {
        assert!(
            self.marks.when.is_none(),
            "a show rule cannot wait on a state: set it outside \
             `when`",
        );
    }

    /// `view` with the set rules in force applied: rules fill what
    /// its call site left unset, defaults first, then outer scopes
    /// before inner.
    /// Rules waiting on a state are left out: an element puts
    /// them on top while mounted.
    pub fn resolve<V: Styled>(&self, view: V) -> V {
        let theme = self.theme;
        let below = self
            .rules::<V>(RuleKind::Set)
            .fold(V::unset(), |below, rule| rule(below, theme));
        view.over(below)
    }

    /// Builds the replacement of `matched` by the latest show rule
    /// that takes it, skipping the rules it went through and trying
    /// defaults last. Gives it back when none takes it.
    pub(crate) fn apply_show<E: Element<B, T>>(
        &mut self,
        mut matched: Matched<E>,
    ) -> Result<B::Node, Matched<E>> {
        if !self.show_kinds.contains(&TypeId::of::<E>()) {
            return Err(matched);
        }
        for default in [false, true] {
            for index in (0..self.entries.len()).rev() {
                let entry = self.entries[index];
                if entry.kind != RuleKind::Show
                    || entry.view != TypeId::of::<E>()
                    || entry.default != default
                    || !entry.reaches()
                    || matched.done.contains(&entry.key)
                {
                    continue;
                }
                let rule = self
                    .mounted
                    .rules
                    .get::<ShowRule<B, E, T>>(&entry.key)
                    .expect("a show rule of its stored type");
                matched.done.push(entry.key);
                match rule(matched, self.theme) {
                    Ok(replacement) => {
                        return Ok(
                            self.build_replacement::<E>(replacement)
                        );
                    }
                    Err(passed) => {
                        matched = passed;
                        matched.done.pop();
                    }
                }
            }
        }
        Err(matched)
    }

    /// Builds what a show rule for an `E` made.
    fn build_replacement<E: 'static>(
        &mut self,
        replacement: AnyView<B, T>,
    ) -> B::Node {
        assert!(
            self.show_depth < MAX_SHOW_DEPTH,
            "show rules for `{}` nested {MAX_SHOW_DEPTH} deep: a \
             rule builds a new view of the kind it matches",
            core::any::type_name::<E>(),
        );
        self.show_depth += 1;
        let node = self.scope(|cx| replacement.build(cx));
        self.show_depth -= 1;
        node
    }
}
