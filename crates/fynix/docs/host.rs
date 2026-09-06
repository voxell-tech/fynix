//! Shared setup for the crate-level doc examples: a tiny in-memory
//! backend, two elements, and one pointer tag.

#![allow(dead_code)]

pub use fynix::prelude::*;

use std::collections::HashMap;
use std::time::Duration;

use motiongfx_interp::ease;

/// What elements write to. A real backend has components; this has a
/// field per thing anybody writes.
#[derive(Default)]
pub struct Node {
    parent: Option<usize>,
    pub text: String,
    pub size: u32,
    pub padding: u32,
}

/// App state a binding reads from.
#[derive(Default)]
pub struct Doc {
    pub title: String,
    pub dirty: bool,
}

#[derive(Default)]
pub struct World {
    nodes: HashMap<usize, Node>,
    next: usize,
    pub doc: Doc,
    pub delta: Duration,
}

impl World {
    /// A world holding nothing but a root, and that root.
    pub fn with_root() -> (Self, usize) {
        let mut world = Self::default();
        let root = world.insert(Node::default());
        (world, root)
    }

    fn insert(&mut self, node: Node) -> usize {
        let id = self.next;
        self.next += 1;
        self.nodes.insert(id, node);
        id
    }

    pub fn node(&mut self, id: usize) -> &mut Node {
        self.nodes.get_mut(&id).expect("live node")
    }

    pub fn get(&self, id: usize) -> &Node {
        self.nodes.get(&id).expect("live node")
    }
}

/// A watch predicate that fires on the first flush and never again,
/// the way a one-shot bootstrap build does.
pub fn once()
-> impl for<'w> FnMut(WorldNodeRef<'w, FynixHost>) -> bool + Send + Sync + 'static
{
    let mut pending = true;
    move |_| std::mem::take(&mut pending)
}

pub struct FynixHost;

impl Host for FynixHost {
    type Node = usize;
    type World = World;
    type Theme = ();

    fn delta(world: &World) -> Duration {
        world.delta
    }

    fn spawn(world: &mut World, parent: usize) -> usize {
        world.insert(Node {
            parent: Some(parent),
            ..Node::default()
        })
    }

    fn exists(world: &World, node: usize) -> bool {
        world.nodes.contains_key(&node)
    }

    fn children(world: &World, node: usize) -> Vec<usize> {
        let mut kids: Vec<usize> = world
            .nodes
            .iter()
            .filter(|(_, n)| n.parent == Some(node))
            .map(|(id, _)| *id)
            .collect();
        kids.sort_unstable();
        kids
    }

    fn despawn(world: &mut World, node: usize) {
        for child in Self::children(world, node) {
            Self::despawn(world, child);
        }
        world.nodes.remove(&node);
    }
}

/// The pointer is over this node. Tags are plain types; nothing here
/// implements a fynix trait.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Hovered;

/// A text label.
#[element(host = FynixHost)]
pub struct Label {
    #[elem(default = String::from("Label"), patch = WriteText)]
    pub text: String,
    #[elem(default = 13, patch = WriteSize)]
    pub size: u32,
}

/// A label plus padding, whose `size` eases toward `hover_size` while
/// the node carries [`Hovered`].
#[element(host = FynixHost)]
pub struct Button {
    #[elem(child)]
    pub label: Label,

    #[elem(default = 4, patch = WritePadding)]
    pub padding: u32,

    #[elem(default = 13, patch = WriteSize, anim(
        ms = 120,
        ease = ease::cubic::ease_in_out,
        on(Hovered, read = hover_size),
    ))]
    pub size: u32,

    /// Element state the line reads through; nothing draws it.
    pub hover_size: u32,
}

pub struct WriteText;

impl FieldPatch<FynixHost> for WriteText {
    type Target = String;

    fn patch(patch: &mut Patch<FynixHost>, text: &String) {
        let id = patch.id();
        patch.world.node(id).text = text.clone();
    }
}

pub struct WriteSize;

impl FieldPatch<FynixHost> for WriteSize {
    type Target = u32;

    fn patch(patch: &mut Patch<FynixHost>, size: &u32) {
        let id = patch.id();
        patch.world.node(id).size = *size;
    }
}

pub struct WritePadding;

impl FieldPatch<FynixHost> for WritePadding {
    type Target = u32;

    fn patch(patch: &mut Patch<FynixHost>, padding: &u32) {
        let id = patch.id();
        patch.world.node(id).padding = *padding;
    }
}
