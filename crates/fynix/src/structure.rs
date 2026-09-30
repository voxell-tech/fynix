//! Views that build and drop parts of the tree after the first build:
//! [`keyed`] and [`each`].

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem;

use crate::backend::Backend;
use crate::cx::Cx;
use crate::mounted::{Group, Mounted, Structure};
use crate::prop::Signal;
use crate::rules::{RuleArena, ScopeEntry};
use crate::view::{AnyView, View};

/// How a structural view builds one part of what it holds.
type BuildFn<B, T, K> =
    dyn Fn(&K) -> AnyView<B, T> + Send + Sync + 'static;

/// Builds with `group` as the owner of the structural views made,
/// in a scope of its own, under `container`.
fn build_in<B: Backend, T: 'static>(
    cx: &mut Cx<'_, B, T>,
    container: B::Node,
    group: Group,
    view: AnyView<B, T>,
) -> B::Node {
    let outer = cx.owner();
    cx.set_owner(Some(group));
    let built =
        cx.scope(|cx| cx.under(container, |cx| view.build(cx)));
    cx.set_owner(outer);
    built
}

/// Builds the container a structural view holds its parts in: `within`,
/// or a bare node, whose layout is the backend's default.
fn container<B: Backend, T: 'static>(
    cx: &mut Cx<'_, B, T>,
    within: Option<AnyView<B, T>>,
) -> B::Node {
    match within {
        Some(view) => cx.build(view),
        None => cx.spawn(),
    }
}

/// A view that builds again under the same node whenever its key
/// changes to a different value. See [`keyed`].
pub struct Keyed<B: Backend, T, K> {
    key: Signal<B::World, K>,
    build: Box<BuildFn<B, T, K>>,
    within: Option<AnyView<B, T>>,
}

impl<B: Backend, T, K> Keyed<B, T, K> {
    /// This, with its view built under the root of `container`, a view
    /// with nothing under it, instead of under a bare node: for giving
    /// the container a layout.
    pub fn within(
        mut self,
        container: impl View<B, T> + 'static,
    ) -> Self {
        self.within =
            Some(AnyView::new(move |cx| container.build(cx)));
        self
    }
}

/// A view of `build(&key)`, built again when the key signal reports a
/// change and the key differs from the last one.
///
/// The old view is despawned and the new one built under the same
/// container node, so the view keeps its place among its siblings.
/// Rules in force where the `keyed` sits are in force in every build.
pub fn keyed<B, T, K>(
    key: Signal<B::World, K>,
    build: impl Fn(&K) -> AnyView<B, T> + Send + Sync + 'static,
) -> Keyed<B, T, K>
where
    B: Backend,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    Keyed {
        key,
        build: Box::new(build),
        within: None,
    }
}

impl<B, T, K> View<B, T> for Keyed<B, T, K>
where
    B: Backend,
    T: 'static,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fn build(mut self, cx: &mut Cx<'_, B, T>) -> B::Node {
        let container = container(cx, self.within.take());
        let parent = cx.owner();
        let id = cx.mounted().new_group();
        let capture = cx.capture();
        let last = self.key.get(cx.world);
        // The first check fires on any source, and the build covers it.
        self.key.changed(cx.world);
        let view = (self.build)(&last);
        let child = build_in(cx, container, id, view);
        cx.mounted().register(
            id,
            parent,
            container,
            Box::new(KeyedEntry {
                key: self.key,
                build: self.build,
                last,
                container,
                child,
                capture,
            }),
        );
        container
    }
}

struct KeyedEntry<B: Backend, T, K> {
    key: Signal<B::World, K>,
    build: Box<BuildFn<B, T, K>>,
    last: K,
    container: B::Node,
    child: B::Node,
    capture: Vec<ScopeEntry<B>>,
}

impl<B, T, K> Structure<B, T> for KeyedEntry<B, T, K>
where
    B: Backend,
    T: 'static,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fn update(
        &mut self,
        id: Group,
        world: &mut B::World,
        theme: &T,
        mounted: &mut Mounted<B, T>,
    ) {
        if !self.key.changed(world) {
            return;
        }
        let next = self.key.get(world);
        if next == self.last {
            return;
        }
        mounted.leave(world, self.child);
        mounted.drop_group(id);
        let mut cx =
            Cx::seeded(world, theme, mounted, &self.capture, id);
        let view = (self.build)(&next);
        self.child = build_in(&mut cx, self.container, id, view);
        self.last = next;
    }

    fn release(&mut self, rules: &mut RuleArena) {
        for entry in self.capture.drain(..) {
            rules.release(entry.key);
        }
    }
}

/// One view per item under a container node, kept while its key is in
/// the list. See [`each`].
pub struct Each<B: Backend, T, I, K> {
    items: Signal<B::World, Vec<I>>,
    key: fn(&I) -> K,
    build: Box<BuildFn<B, T, I>>,
    within: Option<AnyView<B, T>>,
}

impl<B: Backend, T, I, K> Each<B, T, I, K> {
    /// This, with its items built under the root of `container`, a
    /// view with nothing under it, instead of under a bare node: for
    /// giving the container a layout.
    pub fn within(
        mut self,
        container: impl View<B, T> + 'static,
    ) -> Self {
        self.within =
            Some(AnyView::new(move |cx| container.build(cx)));
        self
    }
}

/// One view of `build(&item)` per item, matched by `key` when the list
/// changes.
///
/// A key that leaves the list has its node despawned. A new key is
/// built, under the rules in force where the `each` sits. A key that
/// stays keeps its node, so its state, focus and transitions survive,
/// and the container's children are reordered to match the list.
///
/// A kept item is not built again when its data changes. Whatever
/// should follow the data has to be a bound prop.
pub fn each<B, T, I, K>(
    items: Signal<B::World, Vec<I>>,
    key: fn(&I) -> K,
    build: impl Fn(&I) -> AnyView<B, T> + Send + Sync + 'static,
) -> Each<B, T, I, K>
where
    B: Backend,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    Each {
        items,
        key,
        build: Box::new(build),
        within: None,
    }
}

/// One item's node, and the group of structural views it built.
struct Row<B: Backend, K> {
    key: K,
    node: B::Node,
    group: Group,
}

impl<B, T, I, K> View<B, T> for Each<B, T, I, K>
where
    B: Backend,
    T: 'static,
    I: 'static,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fn build(mut self, cx: &mut Cx<'_, B, T>) -> B::Node {
        let container = container(cx, self.within.take());
        let parent = cx.owner();
        let id = cx.mounted().new_group();
        let capture = cx.capture();
        let items = self.items.get(cx.world);
        self.items.changed(cx.world);
        let mut rows = Vec::with_capacity(items.len());
        for item in &items {
            let group = cx.mounted().new_group();
            let view = (self.build)(item);
            let node = build_in(cx, container, group, view);
            rows.push(Row::<B, K> {
                key: (self.key)(item),
                node,
                group,
            });
        }
        cx.mounted().register(
            id,
            parent,
            container,
            Box::new(EachEntry {
                items: self.items,
                key: self.key,
                build: self.build,
                container,
                shown: rows.iter().map(|row| row.node).collect(),
                rows,
                capture,
            }),
        );
        container
    }
}

struct EachEntry<B: Backend, T, I, K> {
    items: Signal<B::World, Vec<I>>,
    key: fn(&I) -> K,
    build: Box<BuildFn<B, T, I>>,
    container: B::Node,
    rows: Vec<Row<B, K>>,
    /// The container's children as last ordered: the rows, and the
    /// rows animating out where they were.
    shown: Vec<B::Node>,
    capture: Vec<ScopeEntry<B>>,
}

/// `live` in its order, with each node of `before` that `leaving`
/// holds for put back after the node it followed there.
fn keep_leaving<N: Copy + PartialEq>(
    before: &[N],
    mut live: Vec<N>,
    leaving: impl Fn(N) -> bool,
) -> Vec<N> {
    let mut anchor = None;
    for &node in before {
        if leaving(node) {
            let at = anchor
                .and_then(|anchor| {
                    live.iter().position(|&shown| shown == anchor)
                })
                .map_or(0, |at| at + 1);
            live.insert(at, node);
            anchor = Some(node);
        } else if live.contains(&node) {
            anchor = Some(node);
        }
    }
    live
}

impl<B, T, I, K> Structure<B, T> for EachEntry<B, T, I, K>
where
    B: Backend,
    T: 'static,
    I: 'static,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    fn update(
        &mut self,
        id: Group,
        world: &mut B::World,
        theme: &T,
        mounted: &mut Mounted<B, T>,
    ) {
        if !self.items.changed(world) {
            return;
        }
        let items = self.items.get(world);
        // Rows still animating out keep their place among the rest.
        self.shown.retain(|&node| {
            mounted.is_leaving(node)
                || self.rows.iter().any(|row| row.node == node)
        });
        let before = self.shown.clone();

        // Keys may repeat, so each old row is matched at most once.
        let mut old = mem::take(&mut self.rows)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let kept = items
            .iter()
            .map(|item| {
                let key = (self.key)(item);
                let at = old.iter().position(|row| {
                    row.as_ref().is_some_and(|row| row.key == key)
                })?;
                old[at].take()
            })
            .collect::<Vec<_>>();
        for row in old.into_iter().flatten() {
            mounted.leave(world, row.node);
            mounted.drop_group(row.group);
        }

        let mut cx = kept.iter().any(Option::is_none).then(|| {
            Cx::seeded(world, theme, mounted, &self.capture, id)
        });
        let mut rows = Vec::with_capacity(items.len());
        for (item, kept) in items.iter().zip(kept) {
            rows.push(match (kept, cx.as_mut()) {
                (Some(row), _) => row,
                (None, Some(cx)) => {
                    let group = cx.mounted().new_group();
                    let view = (self.build)(item);
                    let node =
                        build_in(cx, self.container, group, view);
                    Row {
                        key: (self.key)(item),
                        node,
                        group,
                    }
                }
                (None, None) => continue,
            });
        }
        drop(cx);

        let live =
            rows.iter().map(|row| row.node).collect::<Vec<_>>();
        let after = keep_leaving(&before, live, |node| {
            mounted.is_leaving(node)
        });
        if after != before {
            B::reorder(world, self.container, &after);
        }
        self.shown = after;
        self.rows = rows;
    }

    fn release(&mut self, rules: &mut RuleArena) {
        for entry in self.capture.drain(..) {
            rules.release(entry.key);
        }
    }
}
