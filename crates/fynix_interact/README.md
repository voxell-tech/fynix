# Fynix Interact

Interactions and accessibility for [fynix](../fynix) views, on any
backend.

Fynix's core knows nothing of a pointer or a screen reader. This crate
names what a backend can offer for them in one trait, `Interactive`, so
a view written against it works on every backend that implements it.
A backend that has no pointer, or an app that wants none of it, can
leave this crate out.

- **States.** `Hovered`, `Pressed` and `Focused` hold on whatever the
  backend tracks them with, so rules wait on them without naming it:
  `.when_in::<Hovered, _>(..)`.
- **Activation.** `.on_activate::<B>(action)` runs `action` when the
  view's root is clicked, or pressed with a key while it has focus.
- **Accessibility.** `.accessible(Role::Button)` and
  `.described(Role::Slider, "Volume")` tell assistive technology what
  the view's root is.

```rust,ignore
use fynix::{Backend, ScopedExt, View};
use fynix_interact::{Hovered, InteractExt, Interactive, Role};

/// A view that does something when activated, named for screen
/// readers, restyled by `hover` while the pointer is over it.
fn pressable<B: Interactive, T: 'static>(
    view: impl View<B, T>,
    name: &str,
    hover: impl FnOnce(&mut fynix::Cx<'_, B, T>) + 'static,
    action: impl Fn(&mut B::World) + Send + Sync + 'static,
) -> impl View<B, T> {
    view.when_in::<Hovered, _>(hover)
        .described(Role::Button, name)
        .on_activate::<B>(action)
}
```

`bevy_fynix` implements `Interactive` behind its `interact` feature,
on by default.
