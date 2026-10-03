# Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix.svg)](https://crates.io/crates/fynix)
[![Downloads](https://img.shields.io/crates/d/fynix.svg)](https://crates.io/crates/fynix)
[![Docs](https://docs.rs/fynix/badge.svg)](https://docs.rs/fynix/latest/fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix** is a backend agnostic reactive view tree, styled the way
[Typst](https://typst.app) styles a document with set and show rules.

A backend says what a world and a node are, and writes the elements.
Everything above the elements, from composites to rules, themes and
transitions, is the same on every backend.

## Features

- Views, rules and transitions never name an engine.
- A view asks for the token traits it reads, so it works under any
  theme that implements them.
- Set and show rules restyle every view of a kind within a scope.
- State rules hold while a node is hovered, pressed, entering,
  leaving, or in any state of your own.
- Props bound to the world are read again only when their source
  changes.
- Views can animate their changes, and animate in and out of the
  tree.
- `#![no_std]`, with `alloc` only.

## Concepts

The examples below run against a toy backend from
[`docs/toy.rs`](docs/toy.rs): a flat list of nodes that hold text.

### Elements

An element is a struct of props that ends up on one node. Each prop
names the `Patch` that writes it to the node, and `#[element]` writes
the rest: the unset state, a builder method per prop, and the updates
when a bound prop changes.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
trait TextTokens {
    fn body_size(&self) -> f32;
}

#[element(backend = Toy, theme = TextTokens)]
pub struct Text {
    #[elem(patch = WriteText)]
    text: Prop<World, String>,
    // Unset, the size comes from the theme.
    #[elem(default = theme.body_size(), patch = WriteSize)]
    size: Prop<World, f32>,
}

fn text(text: impl Into<Prop<World, String>>) -> Text {
    Text {
        text: text.into(),
        ..Text::unset()
    }
}

pub struct WriteSize;

impl Patch<Toy, f32> for WriteSize {
    fn patch(world: &mut World, node: usize, size: &f32) {
        world.nodes[node].size = *size;
    }
}
# pub struct WriteText;
# impl Patch<Toy, String> for WriteText {
#     fn patch(world: &mut World, node: usize, text: &String) {
#         world.nodes[node].text.clone_from(text);
#     }
# }
# fn main() {}
```

### Tokens and themes

A token trait, like `TextTokens` above, is all a view knows about a
theme. A theme is any type that implements the token traits its views
read, so swapping the theme restyles every view that reads it.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
struct Large;

impl TextTokens for Large {
    fn body_size(&self) -> f32 {
        18.0
    }
}
# impl MotionTokens for Large {
#     fn motion(&self, m: Motion) -> Curve {
#         Warm.motion(m)
#     }
# }

# fn main() {
let mut ui = Ui::new(Large);
let node = ui.build(|cx| cx.build(text("Hello")));
assert_eq!(ui.size(node), 18.0);
# }
```

### Set rules

A set rule restyles every view of one kind built after it, like
Typst's `#set text(size: 12pt)`. It only fills what the call site
left unset.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
# fn main() {
let mut ui = Ui::new(Warm);
let (body, title) = ui.build(|cx| {
    cx.set::<Text>(|t, _| t.size(12.0));
    (
        cx.build(text("Body")),
        cx.build(text("Title").size(20.0)),
    )
});
assert_eq!(ui.size(body), 12.0);
assert_eq!(ui.size(title), 20.0);
# }
```

### Show rules

A show rule, like Typst's `#show`, transforms every view of one kind
after its call site, so it wins over a value set there.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
# fn main() {
let mut ui = Ui::new(Warm);
let title = ui.build(|cx| {
    cx.show::<Text>(|t, _| t.size(30.0));
    cx.build(text("Title").size(20.0))
});
assert_eq!(ui.size(title), 30.0);
# }
```

### Scopes

Rules end with the scope they are set in.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
# fn main() {
let mut ui = Ui::new(Warm);
let (inside, after) = ui.build(|cx| {
    let inside = cx.scope(|cx| {
        cx.set::<Text>(|t, _| t.size(12.0));
        cx.build(text("Inside"))
    });
    (inside, cx.build(text("After")))
});
assert_eq!(ui.size(inside), 12.0);
assert_eq!(ui.size(after), 14.0);
# }
```

### State rules

A state rule holds only while its view is in a state, such as
hovered. Rules set inside it reach every view under that view.

```rust
# #[path = "docs/toy.rs"] mod _doc; use _doc::*;
# fn main() {
let mut ui = Ui::new(Warm);
let node = ui.build(|cx| {
    text("Save")
        .when_in::<Hovered, _>(|cx: &mut Cx<Toy, Warm>| {
            cx.set::<Text>(|t, _| t.size(20.0));
        })
        .build(cx)
});
assert_eq!(ui.size(node), 14.0);

ui.world.nodes[node].hovered = true;
ui.mounted.mark_dirty(node);
ui.frame();
assert_eq!(ui.size(node), 20.0);
# }
```

Add `.transition(..)` to a view and its changes animate over a curve
from the theme's `MotionTokens`. `keyed` and `each` rebuild parts of
the tree as the world changes: the old view fades out and its space
collapses, then the new view's space expands and it fades in.
`Tick::reduced_motion` skips every step.

## Building a UI framework

Fynix is the part of a UI framework that does not depend on an
engine. A framework on top of it fills in five things, and
[`bevy_fynix`](../bevy_fynix) is a full example of each:

1. **A backend**, which says what a world and a node are. `spawn`,
   `despawn` and `reorder` are required. `on_mount`, `leave`, `hold`,
   `collapse` and `release` have empty defaults. Cleanup, input
   blocking and the animation of a view's space go there: a dropped
   view is `leave`d, then `collapse`d from 0 to 1, and a new one is
   `hold`ed out of the layout, `collapse`d from 1 to 0 once its size
   can be measured, then `release`d.
2. **Elements**, as above. For each prop you say how its value is
   written to a node (`Patch`), and optionally its default from the
   theme.
3. **Token traits**. Elements and composites bound themselves on the
   tokens they read, and your framework's users bring their own
   themes.
4. **States**, which implement `Condition`: whether the state holds on
   a node, and a request to the backend to report the node to
   `Mounted::mark_dirty` whenever that changes.
5. **A mount**, which builds a view at the root with a `Cx`, and a
   frame loop that calls `update_structure` and `update_elements`.

Here are all five in one file:

```rust
use core::time::Duration;

// `#[element]` finds these at the root of the crate it is used in,
// so a framework re-exports them from its own root.
use fynix::{
    Backend, Condition, Curve, Cx, Element, Motion, MotionTokens,
    Mounted, Patch, Prop, ScopedExt, Slot, Styled, Tick, View,
    ViewExt, element, styled,
};

// 1. A backend.
#[derive(Default)]
pub struct World {
    nodes: Vec<Node>,
}

#[derive(Default)]
struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    text: String,
    size: f32,
    hovered: bool,
}

struct Toy;

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
        // The node stays in the list, only unlinked from its parent.
        if let Some(parent) = world.nodes[node].parent {
            world.nodes[parent].children.retain(|&c| c != node);
        }
    }

    fn reorder(world: &mut World, parent: usize, children: &[usize]) {
        world.nodes[parent].children = children.to_vec();
    }
}

// 3. Token traits.
trait TextTokens {
    fn body_size(&self) -> f32;
}

// 2. An element, reading `TextTokens` from any theme.
#[element(backend = Toy, theme = TextTokens)]
pub struct Text {
    #[elem(patch = WriteText)]
    text: Prop<World, String>,
    /// The theme's body size when unset.
    #[elem(default = theme.body_size(), patch = WriteSize)]
    size: Prop<World, f32>,
}

fn text(text: impl Into<Prop<World, String>>) -> Text {
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

// 4. A state.
struct Hovered;

impl Condition<Toy> for Hovered {
    fn holds(world: &World, node: usize) -> bool {
        world.nodes[node].hovered
    }

    fn watch(_: &mut World, _: usize) {
        // A real backend hooks its hover events up to `mark_dirty`.
    }
}

// A theme, which any app can write its own of.
struct Warm;

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

// 5. Mount a view, then update it from the frame loop.
fn main() {
    let mut world = World::default();
    let mut mounted = Mounted::<Toy, Warm>::default();
    let node = {
        let mut cx = Cx::new(&mut world, &Warm, &mut mounted);
        text("Save")
            // Every text under this one grows while it is hovered.
            .when_in::<Hovered, _>(|cx: &mut Cx<Toy, Warm>| {
                cx.set::<Text>(|t, _| t.size(20.0));
            })
            .build(&mut cx)
    };

    let tick = Tick {
        delta: Duration::from_millis(16),
        reduced_motion: false,
    };
    mounted.update_structure(&mut world, &Warm);
    mounted.update_elements(&mut world, &Warm, tick);
    assert_eq!(world.nodes[node].text, "Save");
    assert_eq!(world.nodes[node].size, 14.0);

    world.nodes[node].hovered = true;
    mounted.mark_dirty(node);
    mounted.update_elements(&mut world, &Warm, tick);
    assert_eq!(world.nodes[node].size, 20.0);
}
```

## Views

A view is a struct that owns its props. There are three kinds:

- **Elements** are one node each, written by the backend: `Label`,
  `Frame`, `Icon`.
- **Composites** are built out of other views: `row`, `column`,
  `button`, `foldable`. Their parts are ordinary views, so ordinary
  rules reach them.
- **Wrappers** add something to any view: `.when(..)`,
  `.transition(..)`, `.rules(..)`, `.on_activate(..)`.

`keyed` and `each` build and drop parts of the tree as the world
changes, and keep every view whose key stays.

## Styling

From weakest to strongest:

| Layer | Example |
|---|---|
| View default | a theme token |
| Composite defaults | `cx.defaults(..)` |
| Set rules, outer then inner | `cx.set::<Label>(\|l, _\| l.size(12.0))` |
| State rules from ancestors | `row(..).when::<Hovered, _>(..)` |
| Call site | `label("x").size(20.0)` |
| State rules on the node itself | `label("x").when::<Hovered, _>(..)` |

Show rules then transform the result. A theme says what the tokens
are (colours, sizes, curves), a rule bundle (`.rules(..)`) says which
look a view has, and a state rule says what that look does while a
state holds. Of several state rules on one node, the one in the outer
scope wins, and within one scope the one written later. The Bevy
backend's README shows buttons styled this way, with ready-made looks
and a `Style` builder.

## Officially supported backends

- [Bevy Fynix](https://crates.io/crates/bevy_fynix)

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
