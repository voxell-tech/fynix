use field_path::field_accessor::FieldAccessor;

use crate::Fynix;
use crate::element::{Element, ElementId};
use crate::style::{StyleId, StyleValue};

/// Build-time context for constructing the element tree and declaring
/// style defaults.
///
/// Obtained from [`Fynix::root_ctx`]. Tracks which style node is
/// "current" so that defaults set via [`Self::set`] are inherited by
/// subsequently added elements.
///
/// # Style scoping
///
/// Style changes queued with [`Self::set`] are committed into a new
/// [`Style`] node the next time an element is added. Inside an
/// [`Self::add_with`] closure, the outer `parent_style_id` is saved
/// and restored after the closure returns, so inner style changes do
/// not leak outward.
///
/// [`Style`]: crate::style::Style
pub struct FynixCtx<'f, 'w, W> {
    parent_style_id: Option<StyleId>,
    fynix: &'f mut Fynix,
    pub world: &'w mut W,
}

impl<W> FynixCtx<'_, '_, W> {
    pub(crate) fn new<'f, 'w>(
        parent_style_id: Option<StyleId>,
        fynix: &'f mut Fynix,
        world: &'w mut W,
    ) -> FynixCtx<'f, 'w, W> {
        FynixCtx {
            parent_style_id,
            fynix,
            world,
        }
    }

    /// Creates element `E`, applies the current style chain to it,
    /// stores it, and returns its [`ElementId`].
    #[must_use]
    pub fn add<E>(&mut self) -> ElementId
    where
        E: Element,
    {
        let element = self.create_element::<E>();
        self.fynix.elements.add(element)
    }

    /// Like [`Self::add`], but also runs `f` for inline mutations and
    /// nested element additions.
    ///
    /// The outer `parent_style_id` is restored after `f` returns, so
    /// any [`Self::set`] calls inside `f` do not affect elements
    /// added after this call.
    #[must_use]
    pub fn add_with<E>(
        &mut self,
        f: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId
    where
        E: Element,
    {
        let mut element = self.create_element::<E>();
        let parent_style_id = self.parent_style_id;

        f(&mut element, self);

        // Restore parent style id from before the closure.
        self.parent_style_id = parent_style_id;

        self.fynix.elements.add(element)
    }

    /// Queues a style default: field `field_accessor` on element type `E`
    /// will be set to `value` for all elements added after this call (within
    /// the current scope).
    pub fn set<E, T>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) where
        E: Element,
        T: StyleValue,
    {
        self.fynix.styles.set(field_accessor, value);
    }

    /// Commits any pending style changes, constructs `E::new()`, and
    /// applies the current style chain to it.
    fn create_element<E>(&mut self) -> E
    where
        E: Element,
    {
        if self.fynix.styles.should_commit() {
            let committed_id = self.fynix.styles.current_id();
            self.fynix.styles.commit_styles(self.parent_style_id);
            self.parent_style_id = Some(committed_id);
        }
        let mut element = E::new();
        if let Some(id) = &self.parent_style_id {
            self.fynix.styles.apply(&mut element, id);
        }
        element
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    use field_path::field_accessor;
    use rectree::{Constraint, NodeContext, Size, Vec2};

    use crate::element::ElementNodes;

    use super::*;

    #[derive(Default, Clone)]
    struct Label {
        pub text: String,
    }

    impl Element for Label {
        fn new() -> Self {
            Self::default()
        }

        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> rectree::Size
        where
            Self: Sized,
        {
            constraint.min
        }
    }

    #[derive(Default, Clone)]
    struct Vertical {
        children: Vec<ElementId>,
    }

    impl Vertical {
        pub fn add(&mut self, id: ElementId) {
            self.children.push(id);
        }
    }

    impl Element for Vertical {
        fn new() -> Self {
            Self::default()
        }

        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            nodes: &mut ElementNodes,
        ) -> Size
        where
            Self: Sized,
        {
            let mut size = Size::ZERO;

            for child in self.children.iter() {
                let child_size = nodes.get_size(child);
                nodes.set_translation(
                    child,
                    Vec2::new(0.0, size.height),
                );

                size.width = size.width.max(child_size.width);
                size.height += child_size.height;
            }

            constraint.constrain(size)
        }
    }

    #[test]
    fn style_applied_after_set() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(
                field_accessor!(<Label>::text),
                "hello".to_string(),
            );
            ctx.add_with::<Vertical>(|v, ctx| {
                v.add(ctx.add::<Label>());
            })
        };

        let vertical =
            fynix.elements.get_typed::<Vertical>(&root_id).unwrap();
        let label_id = vertical.children[0];
        let label =
            fynix.elements.get_typed::<Label>(&label_id).unwrap();
        assert_eq!(label.text, "hello");
    }

    #[test]
    fn add_with_restores_parent_style_id() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(
                field_accessor!(<Label>::text),
                "outer".to_string(),
            );
            ctx.add_with::<Vertical>(|v, ctx| {
                // Inner scope overrides the label text.
                let inner_id = ctx.add_with::<Vertical>(|v, ctx| {
                    ctx.set(
                        field_accessor!(<Label>::text),
                        "inner".to_string(),
                    );
                    v.add(ctx.add::<Label>());
                });

                // After the inner closure, "outer" style is restored.
                v.add(inner_id);
                v.add(ctx.add::<Label>());
            })
        };

        let vertical =
            fynix.elements.get_typed::<Vertical>(&root_id).unwrap();
        let outer_label_id = vertical.children[1];
        let label = fynix
            .elements
            .get_typed::<Label>(&outer_label_id)
            .unwrap();
        assert_eq!(label.text, "outer");
    }

    #[test]
    fn child_style_wins_over_parent() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(
                field_accessor!(<Label>::text),
                "parent".to_string(),
            );
            ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(
                    field_accessor!(<Label>::text),
                    "child".to_string(),
                );
                v.add(ctx.add::<Label>());
            })
        };

        let vertical =
            fynix.elements.get_typed::<Vertical>(&root_id).unwrap();
        let label_id = vertical.children[0];
        let label =
            fynix.elements.get_typed::<Label>(&label_id).unwrap();
        assert_eq!(label.text, "child");
    }
}
