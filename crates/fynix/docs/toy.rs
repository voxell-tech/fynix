//! A toy backend for the README's examples: a flat list of nodes
//! holding text.

use core::time::Duration;

// `#[element]` finds these at the root of the crate it is used in.
pub use fynix::{
    Backend, Condition, Curve, Cx, Element, Motion, MotionTokens,
    Mounted, Patch, Prop, ScopedExt, Slot, Styled, Tick, View,
    ViewExt, element, styled,
};

#[derive(Default)]
pub struct World {
    pub nodes: Vec<Node>,
}

#[derive(Default)]
pub struct Node {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub text: String,
    pub size: f32,
    pub hovered: bool,
}

pub struct Toy;

impl Backend for Toy {
    type World = World;
    type Node = usize;

    fn spawn(world: &mut World, parent: Option<usize>) -> usize {
        let node = world.nodes.len();
        world.nodes.push(Node {
            parent,
            ..Node::default()
        });
        if let Some(parent) = parent {
            world.nodes[parent].children.push(node);
        }
        node
    }

    fn despawn(world: &mut World, node: usize) {
        if let Some(parent) = world.nodes[node].parent {
            world.nodes[parent].children.retain(|&c| c != node);
        }
    }

    fn reorder(world: &mut World, parent: usize, children: &[usize]) {
        world.nodes[parent].children = children.to_vec();
    }
}

pub trait TextTokens {
    fn body_size(&self) -> f32;
}

#[element(backend = Toy, theme = TextTokens)]
pub struct Text {
    #[elem(patch = WriteText)]
    text: Prop<World, String>,
    #[elem(default = theme.body_size(), patch = WriteSize)]
    size: Prop<World, f32>,
}

pub fn text(text: impl Into<Prop<World, String>>) -> Text {
    Text {
        text: text.into(),
        ..Text::unset()
    }
}

pub struct WriteText;

impl Patch<Toy, String> for WriteText {
    fn patch(world: &mut World, node: usize, text: &String) {
        world.nodes[node].text.clone_from(text);
    }
}

pub struct WriteSize;

impl Patch<Toy, f32> for WriteSize {
    fn patch(world: &mut World, node: usize, size: &f32) {
        world.nodes[node].size = *size;
    }
}

pub struct Hovered;

impl Condition<Toy> for Hovered {
    fn holds(world: &World, node: usize) -> bool {
        world.nodes[node].hovered
    }

    fn watch(_: &mut World, _: usize) {}
}

/// A theme with a 14.0 body size.
pub struct Warm;

impl TextTokens for Warm {
    fn body_size(&self) -> f32 {
        14.0
    }
}

impl MotionTokens for Warm {
    fn motion(&self, _: Motion) -> Curve {
        Curve {
            duration: Duration::from_millis(100),
            ease: |t| t,
        }
    }
}

/// A world, a theme and what is mounted in it.
pub struct Ui<T> {
    pub world: World,
    pub theme: T,
    pub mounted: Mounted<Toy, T>,
}

impl<T: TextTokens + 'static> Ui<T> {
    pub fn new(theme: T) -> Self {
        Self {
            world: World::default(),
            theme,
            mounted: Mounted::default(),
        }
    }

    /// Builds views with `build`, then runs one frame.
    pub fn build<R>(
        &mut self,
        build: impl FnOnce(&mut Cx<Toy, T>) -> R,
    ) -> R {
        let built = {
            let mut cx = Cx::new(
                &mut self.world,
                &self.theme,
                &mut self.mounted,
            );
            build(&mut cx)
        };
        self.mounted.update_structure(&mut self.world, &self.theme);
        self.frame();
        built
    }

    /// Runs one frame.
    pub fn frame(&mut self) {
        let tick = Tick {
            delta: Duration::from_millis(16),
            reduced_motion: false,
        };
        self.mounted.update_elements(
            &mut self.world,
            &self.theme,
            tick,
        );
    }

    pub fn size(&self, node: usize) -> f32 {
        self.world.nodes[node].size
    }
}
