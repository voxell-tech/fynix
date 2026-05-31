use crate::ctx::FynixCtx;
use crate::element::ElementId;
use crate::style::Stylable;

/// Builds a subtree from caller-supplied inputs and a pre-styled data
/// struct.
///
/// Unlike [`Element`](crate::element::Element), a composer is not
/// stored in the element tree. It may carry lifetime parameters (e.g.
/// references into the world) because it is consumed immediately
/// inside [`FynixCtx::compose`].
///
/// # Style flow
///
/// [`FynixCtx::compose`] creates `Self::Style` via
/// [`Init`](crate::init::Init), applies the current style chain to
/// it, then passes the result to [`Composer::compose`].
/// [`FynixCtx::compose_with`] additionally runs an inline closure
/// that can override individual fields after the chain is applied.
pub trait Composer<W> {
    type Style: Stylable;

    fn compose(
        self,
        style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, W>,
    ) -> ElementId
    where
        Self: Sized;
}
