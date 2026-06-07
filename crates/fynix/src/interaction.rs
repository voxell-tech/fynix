use core::mem;

use crate::element::{Element, ElementId, Elements};
use crate::events::Events;
use crate::typing::type_table::TypeTable;

/// User-written interaction handler: reacts to interaction `I` on an
/// element of type `E`, emitting messages into [`Events`].
///
/// A plain function pointer, so a non-capturing closure coerces into
/// one at the [`on`](crate::ctx::ElementHandle::on) call site.
pub type HandlerFn<E, I> = fn(&mut E, I, &mut Events);

/// Monomorphized dispatch for interaction type `I`, with the element
/// type `E` erased into the function pointer.
type DispatchFn<I> =
    fn(&mut Elements, &ElementId, I, &mut Events, *const ());

/// Recovers the typed handler and element, then runs the handler.
///
/// One of these is monomorphized per `(E, I)` and stored,
/// type-erased, inside a [`Dispatcher`].
fn dispatch<E: Element, I>(
    elements: &mut Elements,
    id: &ElementId,
    interaction: I,
    events: &mut Events,
    handler: *const (),
) {
    // SAFETY: `Dispatcher::new` pairs `dispatch::<E, I>` with a
    // `HandlerFn<E, I>` erased to `*const ()`, so the pointer is
    // exactly that handler.
    let handler = unsafe {
        mem::transmute::<*const (), HandlerFn<E, I>>(handler)
    };
    if let Some(element) = elements.get_typed_mut::<E>(id) {
        handler(element, interaction, events);
    }
}

/// A handler paired with the [`dispatch`] that knows how to call it.
/// The interaction type `I` is retained; the element type is erased
/// into `dispatch_fn`.
struct Dispatcher<I> {
    handler_fn: *const (),
    dispatch_fn: DispatchFn<I>,
}

impl<I> Dispatcher<I> {
    fn new<E: Element>(handler_fn: HandlerFn<E, I>) -> Self {
        Self {
            handler_fn: handler_fn as *const (),
            dispatch_fn: dispatch::<E, I>,
        }
    }

    fn run(
        self,
        elements: &mut Elements,
        id: &ElementId,
        interaction: I,
        events: &mut Events,
    ) {
        (self.dispatch_fn)(
            elements,
            id,
            interaction,
            events,
            self.handler_fn,
        );
    }
}

impl<I> Copy for Dispatcher<I> {}

impl<I> Clone for Dispatcher<I> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Per-instance interaction store.
///
/// Each element instance carries its own handlers, attached at build
/// time via [`FynixCtx::add`] and [`ElementHandle::on`]. The
/// [`TypeTable`] keeps one column of [`Dispatcher<I>`] per interaction
/// type `I`, addressed by [`ElementId`], so the element type is erased
/// into the dispatcher and recovered by id when `I` is dispatched.
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
    pub fn register<E, I>(
        &mut self,
        id: ElementId,
        handler: HandlerFn<E, I>,
    ) where
        E: Element,
        I: 'static,
    {
        self.table.insert(id, Dispatcher::new(handler));
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
        elements: &mut Elements,
    ) -> bool {
        if let Some(dispatcher) = self.table.get::<Dispatcher<I>>(id) {
            dispatcher.run(elements, id, interaction, events);
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
    struct Counter {
        count: u32,
    }

    impl ElementBuild for Counter {
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
            ctx.add::<Counter>()
                .on(|counter: &mut Counter, _: Click, events| {
                    counter.count += 1;
                    events.push(Clicked(counter.count));
                })
                .id()
        };

        let ran = fynix.dispatch(&id, Click);

        assert!(ran);
        let counter =
            fynix.elements.get_typed::<Counter>(&id).unwrap();
        assert_eq!(counter.count, 1);
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
            ctx.add::<Counter>().id()
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
                .add::<Counter>()
                .on(|counter: &mut Counter, _: Click, _| {
                    counter.count += 1;
                })
                .id();
            // A second instance of the same type with no handler.
            let plain = ctx.add::<Counter>().id();
            (handled, plain)
        };

        assert!(fynix.dispatch(&handled, Click));
        assert!(!fynix.dispatch(&plain, Click));
        assert_eq!(
            fynix
                .elements
                .get_typed::<Counter>(&handled)
                .unwrap()
                .count,
            1
        );
    }

    #[test]
    fn remove_drops_handlers() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Counter>()
                .on(|counter: &mut Counter, _: Click, _| {
                    counter.count += 1;
                })
                .id()
        };

        assert!(fynix.interactions.remove(&id));
        assert!(!fynix.dispatch(&id, Click));
    }
}
