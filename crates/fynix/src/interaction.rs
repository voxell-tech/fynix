use core::ops::{Deref, DerefMut};

/// Whether an interaction keeps bubbling after a handler runs.
///
/// Handlers consume by default ([`Self::Stop`]); a handler opts into
/// bubbling with [`Response::propagate`], which sets
/// [`Self::Continue`] so the interaction reaches the next ancestor
/// handler.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagation {
    /// Consumed; bubbling stops here.
    #[default]
    Stop,
    /// Keep bubbling to the next ancestor handler.
    Continue,
}

/// What a handler is given to react to an interaction: mutable access
/// to the world, and control over whether the interaction keeps
/// bubbling.
///
/// [`Deref`]/[`DerefMut`] target the world, so a handler reaches
/// world fields directly (`res.score += 1`). The interaction is
/// consumed by default; call [`Self::propagate`] to let it continue
/// to the next ancestor handler.
pub struct Response<'w, W> {
    /// The backend world, free for the handler to mutate.
    pub world: &'w mut W,
    propagation: Propagation,
}

impl<'w, W> Response<'w, W> {
    /// Creates a response over `world`, consuming by default.
    pub fn new(world: &'w mut W) -> Self {
        Self {
            world,
            propagation: Propagation::default(),
        }
    }

    /// Lets the interaction keep bubbling to ancestor handlers
    /// instead of consuming it here.
    pub fn propagate(&mut self) {
        self.propagation = Propagation::Continue;
    }

    /// Returns `true` if the interaction was consumed, i.e. bubbling
    /// stops here.
    pub fn consumed(&self) -> bool {
        matches!(self.propagation, Propagation::Stop)
    }
}

impl<W> Deref for Response<'_, W> {
    type Target = W;

    fn deref(&self) -> &W {
        &*self.world
    }
}

impl<W> DerefMut for Response<'_, W> {
    fn deref_mut(&mut self) -> &mut W {
        &mut *self.world
    }
}

/// User-written interaction handler: a non-capturing
/// `fn(I, &mut Response<W>)` that reacts to interaction `I` by
/// mutating the world through the [`Response`].
///
/// A plain function pointer, so a non-capturing closure coerces into
/// one at the [`on`](crate::ctx::ElementCtx::on) call site. Stored
/// per instance as an element-table component, one per `(element,
/// I)`. The handler consumes the interaction by default; call
/// [`Response::propagate`] to let it bubble instead.
pub type HandlerFn<I, W> = fn(I, &mut Response<'_, W>);

#[cfg(test)]
mod tests {
    use rectree::{Constraint, Size};

    use crate::Fynix;
    use crate::element::layout::ElementNodes;
    use crate::element::{Element, ElementBuild, ElementId};
    use crate::init::Init;

    #[derive(Init, Element)]
    struct Button;

    impl ElementBuild for Button {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> Size {
            constraint.min
        }
    }

    #[derive(Init, Element)]
    struct Container {
        #[elem(children)]
        child: Option<ElementId>,
    }

    impl ElementBuild for Container {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> Size {
            constraint.min
        }
    }

    #[derive(Clone, Copy)]
    struct Click;

    #[derive(Default)]
    struct World {
        clicks: u32,
    }

    #[test]
    fn dispatch_runs_attached_handler() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>()
                .on::<Click>(|_, res| res.clicks += 1)
                .id()
        };

        let ran = fynix.dispatch(&id, Click, &mut world);

        assert!(ran);
        assert_eq!(world.clicks, 1);
    }

    #[test]
    fn dispatch_without_handler_returns_false() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>().id()
        };

        struct Unhandled;
        let ran = fynix.dispatch(&id, Unhandled, &mut world);

        assert!(!ran);
    }

    #[test]
    fn handlers_are_per_instance() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let (handled, plain) = {
            let mut ctx = fynix.root_ctx(&mut world);
            let handled = ctx
                .add::<Button>()
                .on::<Click>(|_, res| res.clicks += 1)
                .id();
            // A second instance of the same type with no handler.
            let plain = ctx.add::<Button>().id();
            (handled, plain)
        };

        assert!(fynix.dispatch(&handled, Click, &mut world));
        assert!(!fynix.dispatch(&plain, Click, &mut world));
        assert_eq!(world.clicks, 1);
    }

    #[test]
    fn removing_element_drops_handlers() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>()
                .on::<Click>(|_, res| res.clicks += 1)
                .id()
        };

        // Handlers ride the element table, so removing the element
        // drops them with it.
        assert!(fynix.remove_element(&id));
        assert!(!fynix.dispatch(&id, Click, &mut world));
    }

    #[test]
    fn bubbling_reaches_ancestor_handler() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let (parent, child) = {
            let mut ctx = fynix.root_ctx(&mut world);
            // The child carries no handler; adding the container sets
            // the child's `parent_id` to the container.
            let child = ctx.add::<Button>().id();
            let parent = ctx
                .add_with::<Container>(|c, _| c.child = Some(child))
                .on::<Click>(|_, res| res.clicks += 1)
                .id();
            (parent, child)
        };

        // Bubbling from the child skips it and reaches the parent.
        assert!(fynix.dispatch_bubbling(
            &child,
            Click,
            &mut world,
            |_| true,
        ));
        assert_eq!(world.clicks, 1);
        // Direct dispatch to the child still finds no handler.
        assert!(!fynix.dispatch(&child, Click, &mut world));
        let _ = parent;
    }

    #[test]
    fn bubbling_stops_when_gate_returns_false() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let child = {
            let mut ctx = fynix.root_ctx(&mut world);
            let child = ctx.add::<Button>().id();
            ctx.add_with::<Container>(|c, _| c.child = Some(child))
                .on::<Click>(|_, res| res.clicks += 1)
                .id();
            child
        };

        // The gate rejects the start element, so the walk never
        // reaches the parent's handler.
        assert!(!fynix.dispatch_bubbling(
            &child,
            Click,
            &mut world,
            |_| false,
        ));
        assert_eq!(world.clicks, 0);
    }

    #[test]
    fn bubbling_continues_past_declined_handler() {
        let mut world = World::default();
        let mut fynix = Fynix::new();
        let child = {
            let mut ctx = fynix.root_ctx(&mut world);
            // Child handles `Click` but lets it propagate, so
            // bubbling should continue to the parent.
            let child = ctx
                .add::<Button>()
                .on::<Click>(|_, res| {
                    res.clicks += 1;
                    res.propagate();
                })
                .id();
            ctx.add_with::<Container>(|c, _| c.child = Some(child))
                .on::<Click>(|_, res| res.clicks += 1)
                .id();
            child
        };

        // Both the child (propagates) and the parent (consumes) run.
        assert!(fynix.dispatch_bubbling(
            &child,
            Click,
            &mut world,
            |_| true,
        ));
        assert_eq!(world.clicks, 2);
    }
}
