/// User-written interaction handler: reacts to interaction `I` by
/// mutating the world `W`.
///
/// A plain function pointer, so a non-capturing closure coerces into
/// one at the [`on`](crate::ctx::ElementCtx::on) call site. Stored
/// per instance as an element-table component, one per `(element,
/// I)`.
pub type HandlerFn<I, W> = fn(I, &mut W);

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
                .on::<Click>(|_, world| world.clicks += 1)
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
                .on::<Click>(|_, world| world.clicks += 1)
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
                .on::<Click>(|_, world| world.clicks += 1)
                .id()
        };

        // Handlers ride the element table, so removing the element
        // drops them with it.
        assert!(fynix.remove_element(&id));
        assert!(!fynix.dispatch(&id, Click, &mut world));
    }
}
