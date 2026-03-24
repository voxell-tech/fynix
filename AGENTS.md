# Agents Guide

This file is the authoritative reference for AI agents working on
this repository. Read it before making any changes.

---

## Project Overview

Fynix is a `no_std` Rust UI framework. The workspace has three
crates:

| Crate | Role |
|-------|------|
| `crates/fynix` | Core: element trait, style system, build context |
| `crates/fynix_elements` | Default element types (Horizontal, Vertical, Label) |
| `crates/fynix_vello` | Vello rendering backend (skeleton) |

For architecture details see [`docs/VISION.md`](docs/VISION.md).

---

## Code Style

- All crates are `#![no_std]` + `extern crate alloc`.
- `rustfmt.toml` sets `max_width = 70`. Run `cargo fmt` before
  every commit. Doc comment lines must also respect 70 chars
  (`/// ` prefix is 4 chars, leaving 66 for text).
- Edition 2024 — `if let` chains with `&&` are idiomatic.
- No `std` imports. Use `alloc::vec::Vec`, `alloc::string::String`,
  `alloc::boxed::Box`, etc.
- Prefer `hashbrown::{HashMap, HashSet}` over `std::collections`.

---

## Verification Commands

Always run these in order before committing:

```sh
cargo fmt
cargo check --workspace
cargo test --workspace
cargo doc --workspace --no-deps   # must produce zero warnings
```

---

## What Not To Do

- Do not add `std` imports or remove `#![no_std]`.
- Do not create public re-exports between crates unless discussed.
  External consumers use direct `use fynix::...` paths.
- Do not use `ctx.set()` for inline mutations inside `add_with`
  closures — use direct field assignment (`e.field = value`).

---

## Pending Work

See [`docs/VISION.md`](docs/VISION.md) for full context on each.

| Area | Status |
|------|--------|
| Unit system (`src/unit.rs`) | Planned, not started |
| `ctx.scope()` | Planned, not started |
| `LayoutSolver` / rectree integration | Pending rectree API change |
| `fynix_elements` layout impls | Blocked on rectree |
| `fynix_vello` rendering | Blocked on layout |
| Reactivity (`Signal<T>`) | Deferred until after first render |

### Rectree API change (next major task)

The current `LayoutWorld::get_solver() -> &dyn LayoutSolver`
signature forces heap allocation due to lifetime constraints.
The plan is to push the two solver methods directly onto
`LayoutWorld`:

```rust
pub trait LayoutWorld {
    fn constraint(&self, id: &NodeId, parent: Constraint) -> Constraint;
    fn build(&self, id: &NodeId, node: &RectNode,
             tree: &Rectree, pos: &mut Positioner) -> Size;
}
```

`LayoutSolver` becomes a fynix-internal trait (removed from rectree).
`Fynix` implements `LayoutWorld` by dispatching to a
`HashMap<TypeId, Box<dyn LayoutSolver>>` keyed on element type.
`BuildCtx` gains a `tree: &'a mut Rectree` reference so `add` also
inserts a `RectNode`.
