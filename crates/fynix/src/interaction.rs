use typarena::type_table::TypeTable;

use crate::element::ElementId;
use crate::events::Events;

/// User-written interaction handler: reacts to interaction `I`,
/// emitting messages into [`Events`].
///
/// A plain function pointer, so a non-capturing closure coerces into
/// one at the [`on`](crate::ctx::ElementHandle::on) call site.
pub type HandlerFn<I> = fn(I, &mut Events);

/// Per-instance interaction store.
///
/// Each element instance carries its own handlers, attached at build
/// time via [`FynixCtx::add`] and [`ElementHandle::on`]. The
/// [`TypeTable`] keeps one column of [`HandlerFn<I>`] per interaction
/// type `I`, addressed by [`ElementId`], so dispatching `I` to an id
/// is a single typed column lookup.
///
/// [`FynixCtx::add`]: crate::ctx::FynixCtx::add
/// [`ElementHandle::on`]: crate::ctx::ElementHandle::on
pub struct Interactions {
    table: TypeTable<ElementId>,
}

impl Interactions {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self {
            table: TypeTable::new(),
        }
    }

    /// Attaches `handler` to `id` for interaction type `I`.
    ///
    /// Replaces any handler previously attached to `id` for `I`.
    pub fn register<I: 'static>(
        &mut self,
        id: ElementId,
        handler: HandlerFn<I>,
    ) {
        self.table.insert(id, handler);
    }

    /// Drops every handler attached to `id`, across all interaction
    /// types. Returns `true` if any were present.
    pub fn remove(&mut self, id: &ElementId) -> bool {
        self.table.remove_row(id)
    }

    /// Runs the handler attached to `id` for interaction `I`, if one
    /// exists. Returns `true` if a handler ran.
    pub fn dispatch<I: 'static>(
        &self,
        id: &ElementId,
        interaction: I,
        events: &mut Events,
    ) -> bool {
        if let Some(handler) = self.table.get::<HandlerFn<I>>(id) {
            handler(interaction, events);
            return true;
        }

        false
    }
}

impl Default for Interactions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use rectree::{Constraint, Size};

    use super::*;
    use crate::Fynix;
    use crate::element::layout::ElementNodes;
    use crate::element::{Element, ElementBuild};
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

    #[derive(Debug, PartialEq)]
    struct Clicked(u32);

    #[test]
    fn dispatch_runs_attached_handler() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>()
                .on::<Click>(|_, events| {
                    events.push(Clicked(1));
                })
                .id()
        };

        let ran = fynix.dispatch(&id, Click);

        assert!(ran);
        assert_eq!(
            fynix.events.iter::<Clicked>().next(),
            Some(&Clicked(1))
        );
    }

    #[test]
    fn dispatch_without_handler_returns_false() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>().id()
        };

        struct Unhandled;
        let ran = fynix.dispatch(&id, Unhandled);

        assert!(!ran);
    }

    #[test]
    fn handlers_are_per_instance() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let (handled, plain) = {
            let mut ctx = fynix.root_ctx(&mut world);
            let handled = ctx
                .add::<Button>()
                .on::<Click>(|_, events| events.push(Clicked(1)))
                .id();
            // A second instance of the same type with no handler.
            let plain = ctx.add::<Button>().id();
            (handled, plain)
        };

        assert!(fynix.dispatch(&handled, Click));
        assert!(!fynix.dispatch(&plain, Click));
        assert_eq!(fynix.events.iter::<Clicked>().count(), 1);
    }

    #[test]
    fn remove_drops_handlers() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Button>()
                .on::<Click>(|_, events| events.push(Clicked(1)))
                .id()
        };

        assert!(fynix.interactions.remove(&id));
        assert!(!fynix.dispatch(&id, Click));
    }
}
