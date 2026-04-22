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
    ///
    /// Elements created with `add` dont own any styles, so their
    /// `primary_style` is `None`
    #[must_use]
    pub fn add<E: Element>(&mut self) -> ElementId {
        let element = self.create_element::<E>(false);
        self.fynix.elements.add(element)
    }

    /// Like [`Self::add`], but also runs `f` for inline mutations and
    /// nested element additions.
    ///
    /// The outer `parent_style_id` is restored after `f` returns, so
    /// any [`Self::set`] calls inside `f` do not affect elements
    /// added after this call.
    ///
    /// The element's `primary_style` is set to the first style
    /// committed inside the closure.
    ///
    /// When the element is removed, that style and all its descendants
    /// are also removed.
    #[must_use]
    pub fn add_with<E: Element>(
        &mut self,
        f: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId {
        let parent_style_id_before = self.parent_style_id;

        let mut element = self.create_element::<E>(true);

        // styleId from create_element
        // else, current uncomited style if is None
        let first_inner_style_id = self
            .parent_style_id
            .filter(|id| Some(*id) != parent_style_id_before)
            .unwrap_or_else(|| self.fynix.styles.current_id());

        f(&mut element, self);

        // If self.parent_style_id has changed, that means a
        // style has been committed inside the closure.
        // It's ID would be that of `first_inner_style_id`,
        // so our primary style is set to that.
        let primary_style =
            if self.parent_style_id != parent_style_id_before {
                Some(first_inner_style_id)
            } else {
                None
            };

        // restore to old value
        self.parent_style_id = parent_style_id_before;

        let id = self.fynix.elements.add(element);

        if let Some(style_id) = primary_style
            && let Some(meta) = self.fynix.elements.metas.get_mut(&id)
        {
            meta.primary_style = Some(style_id);
        }

        id
    }

    /// Queues a style default: field `field_accessor` on element type `E`
    /// will be set to `value` for all elements added after this call (within
    /// the current scope).
    pub fn set<E: Element, T: StyleValue>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) {
        self.fynix.styles.set(field_accessor, value);
    }

    /// Commits any pending style changes, constructs `E::new()`, and
    /// applies the current style chain to it.
    ///
    /// `is_deeper` indicates whether the committed style should go
    /// into children[0] (deeper scope) or children[1]
    /// (sibling node at same scope).
    fn create_element<E: Element>(&mut self, is_deeper: bool) -> E {
        if self.fynix.styles.should_commit() {
            let committed_id = self.fynix.styles.current_id();

            self.fynix
                .styles
                .commit_styles(self.parent_style_id, is_deeper);
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

    use crate::element::{ElementBuild, ElementNodes};

    use super::*;

    #[derive(Element, Default, Clone)]
    struct Label {
        pub text: String,
    }

    impl ElementBuild for Label {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> Size {
            constraint.min
        }
    }

    #[derive(Element, Default, Clone)]
    struct Vertical {
        #[children]
        children: Vec<ElementId>,
    }

    impl Vertical {
        pub fn add(&mut self, id: ElementId) {
            self.children.push(id);
        }
    }

    impl ElementBuild for Vertical {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            nodes: &mut ElementNodes,
        ) -> Size {
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
        crate::init();
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
        crate::init();
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

                // After the inner closure, "outer" style is
                // restored.
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
        crate::init();
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

    #[test]
    fn style_tree_cleanup_on_element_removal() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let element_a = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(field_accessor!(<Label>::text), "a".to_string());
            ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(
                    field_accessor!(<Label>::text),
                    "b".to_string(),
                );
                v.add(ctx.add::<Label>());
                ctx.set(
                    field_accessor!(<Label>::text),
                    "c".to_string(),
                );
                v.add(ctx.add::<Label>());
            })
        };

        // Verify we have 3 styles (a, b, c).
        assert_eq!(
            fynix.styles.styles.len(),
            3,
            "Should have 3 styles before removal"
        );

        // Get the primary style for element_a.
        let primary_style = fynix
            .elements
            .metas
            .get(&element_a)
            .and_then(|m| m.primary_style);
        assert!(
            primary_style.is_some(),
            "Element should have primary style"
        );

        // Remove element_a, which should also remove its style tree.
        fynix.remove_element(&element_a);

        // After removal, all 3 styles (a, b, c) should be gone.
        assert_eq!(
            fynix.styles.styles.len(),
            0,
            "All styles should be removed with primary style"
        );
    }

    #[test]
    fn element_without_primary_style_no_cleanup() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let (element_a, element_b) = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(field_accessor!(<Label>::text), "a".to_string());
            let a = ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(
                    field_accessor!(<Label>::text),
                    "b".to_string(),
                );
                v.add(ctx.add::<Label>());
            });
            ctx.set(field_accessor!(<Label>::text), "c".to_string());
            let b = ctx.add::<Label>();
            (a, b)
        };

        // Should have 3 styles (a, b, c).
        assert_eq!(fynix.styles.styles.len(), 3);

        // element_b has no primary_style (created with add(), not add_with()).
        let b_primary = fynix
            .elements
            .metas
            .get(&element_b)
            .and_then(|m| m.primary_style);
        assert!(b_primary.is_none());

        // Remove element_b - should not affect styles.
        fynix.remove_element(&element_b);

        // All 3 styles should still exist.
        assert_eq!(fynix.styles.styles.len(), 3);

        // Remove element_a - should remove its style tree (a, b).
        fynix.remove_element(&element_a);
        // Style c should remain (not part of element_a's tree).
        assert_eq!(fynix.styles.styles.len(), 1);
    }
}
