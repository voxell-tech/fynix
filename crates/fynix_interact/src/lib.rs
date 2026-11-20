#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;

use fynix::{Backend, Condition, Cx, View};

/// A backend whose nodes can be hovered, pressed, focused and
/// activated, and can say what they are to assistive technology.
pub trait Interactive: Backend + Sized {
    /// Holds while the pointer is over a node or one under it.
    type Hovered: Condition<Self>;
    /// Holds while a pointer button is down on a node.
    type Pressed: Condition<Self>;
    /// Holds while a node or one under it has input focus.
    type Focused: Condition<Self>;

    /// Runs `action` whenever `node` is activated: clicked, or
    /// pressed with a key while it has focus. Every action given to
    /// a node runs, in the order given.
    fn on_activate(
        world: &mut Self::World,
        node: Self::Node,
        action: Action<Self>,
    );

    /// Tells assistive technology what `node` is.
    fn describe(
        world: &mut Self::World,
        node: Self::Node,
        semantics: Semantics,
    );
}

/// What runs with the world when a node is activated.
pub type Action<B> =
    Box<dyn Fn(&mut <B as Backend>::World) + Send + Sync>;

/// What a node is to assistive technology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Semantics {
    pub role: Role,
    /// What it is called, when its role and content do not say.
    pub label: Option<String>,
}

/// The kind of thing a node is to assistive technology.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    CheckBox,
    RadioButton,
    Switch,
    Slider,
    TextInput,
    ComboBox,
    Label,
    Image,
    List,
    ListItem,
    Menu,
    MenuItem,
    TabList,
    Tab,
    TabPanel,
    Dialog,
    Tooltip,
    Group,
}

/// The state of a node the pointer is over, on any [`Interactive`]
/// backend: `.when_in::<Hovered, _>(..)`.
pub struct Hovered;

/// The state of a node a pointer button is down on, on any
/// [`Interactive`] backend.
pub struct Pressed;

/// The state of a node that has input focus, or holds the node that
/// does, on any [`Interactive`] backend.
pub struct Focused;

impl<B: Interactive> Condition<B> for Hovered {
    fn holds(world: &B::World, node: B::Node) -> bool {
        B::Hovered::holds(world, node)
    }

    fn watch(world: &mut B::World, node: B::Node) {
        B::Hovered::watch(world, node);
    }
}

impl<B: Interactive> Condition<B> for Pressed {
    fn holds(world: &B::World, node: B::Node) -> bool {
        B::Pressed::holds(world, node)
    }

    fn watch(world: &mut B::World, node: B::Node) {
        B::Pressed::watch(world, node);
    }
}

impl<B: Interactive> Condition<B> for Focused {
    fn holds(world: &B::World, node: B::Node) -> bool {
        B::Focused::holds(world, node)
    }

    fn watch(world: &mut B::World, node: B::Node) {
        B::Focused::watch(world, node);
    }
}

/// A view whose root node runs an action when activated.
pub struct OnActivate<V, F> {
    view: V,
    action: F,
}

/// A view whose root node says what it is to assistive technology.
pub struct Described<V> {
    view: V,
    semantics: Semantics,
}

impl<B, T, V, F> View<B, T> for OnActivate<V, F>
where
    B: Interactive,
    V: View<B, T>,
    F: Fn(&mut B::World) + Send + Sync + 'static,
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        let node = self.view.build(cx);
        B::on_activate(cx.world, node, Box::new(self.action));
        node
    }
}

impl<B, T, V> View<B, T> for Described<V>
where
    B: Interactive,
    V: View<B, T>,
{
    fn build(self, cx: &mut Cx<'_, B, T>) -> B::Node {
        let node = self.view.build(cx);
        B::describe(cx.world, node, self.semantics);
        node
    }
}

/// What any view can be made to answer and say with.
pub trait InteractExt: Sized {
    /// This view, running `action` with the world when its root node
    /// is activated. A closure names the world's type, as in
    /// `|world: &mut World| ..`.
    fn on_activate<F>(self, action: F) -> OnActivate<Self, F> {
        OnActivate { view: self, action }
    }

    /// This view, its root node said to assistive technology to be a
    /// `role`.
    fn accessible(self, role: Role) -> Described<Self> {
        Described {
            view: self,
            semantics: Semantics { role, label: None },
        }
    }

    /// This view, its root node said to assistive technology to be a
    /// `role` called `label`.
    fn described(
        self,
        role: Role,
        label: impl Into<String>,
    ) -> Described<Self> {
        Described {
            view: self,
            semantics: Semantics {
                role,
                label: Some(label.into()),
            },
        }
    }
}

impl<V> InteractExt for V {}
