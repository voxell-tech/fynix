use crate::ctx::FynixCtx;
use crate::element::Element;
use crate::element::storage::ElementHandle;
use crate::style::Stylable;

/// Builds a subtree from caller-supplied inputs and a pre-styled data
/// struct.
///
/// Unlike [`Element`], a composer is not
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
    /// The pre-styled input struct, built and styled by
    /// [`FynixCtx::compose`] before [`Self::compose`] runs.
    type Style: Stylable;
    /// The root element type the composer produces.
    type Element: Element;

    /// Builds the subtree and returns a handle to its root element.
    fn compose(
        self,
        style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, W>,
    ) -> ElementHandle<Self::Element>
    where
        Self: Sized;
}
