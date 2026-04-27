use field_path::field_accessor::FieldAccessor;

use crate::Fynix;
use crate::element::{Element, ElementId};
use crate::style::{StyleId, StyleValue};

// TODO: Docs probably needs to be updated with the new `parent_element_id`.

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
    fynix: &'f mut Fynix,
    pub world: &'w mut W,

    // TODO: We need a way to actually create & represent these.
    prev_style: Option<StyleId>,
    /// The first style created within the current context.
    primary_style: Option<StyleId>,
}

impl<W> FynixCtx<'_, '_, W> {
    pub(crate) fn new<'f, 'w>(
        fynix: &'f mut Fynix,
        world: &'w mut W,
        prev_style: Option<StyleId>,
    ) -> FynixCtx<'f, 'w, W> {
        FynixCtx {
            fynix,
            world,
            prev_style,
            primary_style: None,
        }
    }

    /// Creates element `E`, applies the current style chain to it,
    /// stores it, and returns its [`ElementId`].
    ///
    /// Elements created with `add` dont own any styles, so their
    /// `primary_style` is `None`
    #[must_use]
    pub fn add<E: Element>(&mut self) -> ElementId {
        let element = self.create_element::<E>();
        self.fynix.elements.add(element, None)
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
        scope: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId {
        let mut element = self.create_element::<E>();

        let prev_style_id = self.prev_style;
        let primary_style = self.primary_style.take();

        scope(&mut element, self);

        let id = self
            .fynix
            .elements
            .add(element, self.primary_style.take());

        // Restore pre-closure state.
        self.prev_style = prev_style_id;
        self.primary_style = primary_style;

        // Clears all style leftovers to prevent them from leaking
        // outside the scope.
        self.fynix.styles.clear_builder();

        id
    }

    /// Queues a style default: field `T` on element type `E` will be
    /// set to `value` for all elements added after this call (within
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
    fn create_element<E: Element>(&mut self) -> E {
        if self.fynix.styles.should_commit() {
            let committed_id = self.fynix.styles.current_id();

            let is_nested = self.primary_style.is_none();

            self.fynix
                .styles
                .commit_styles(self.prev_style, is_nested);
            self.prev_style = Some(committed_id);

            if is_nested {
                self.primary_style = Some(committed_id);
            }
        }

        let mut element = E::new();
        if let Some(id) = &self.prev_style {
            self.fynix.styles.apply(&mut element, id);
        }

        element
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use field_path::field_accessor;
    use rectree::{Constraint, NodeContext, Size, Vec2};

    use crate::element::ElementBuild;
    use crate::element::layout::ElementNodes;

    use super::*;

    #[derive(Element, Default, Clone)]
    struct Label {
        pub text: &'static str,
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

    #[derive(Element, Clone)]
    struct Vertical {
        #[elem(children)]
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
        let mut world = ();
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(field_accessor!(<Label>::text), "hello");
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
            ctx.set(field_accessor!(<Label>::text), "outer");
            ctx.add_with::<Vertical>(|v, ctx| {
                // Inner scope overrides the label text.
                let inner_id = ctx.add_with::<Vertical>(|v, ctx| {
                    ctx.set(field_accessor!(<Label>::text), "inner");
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
        let mut world = ();
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(field_accessor!(<Label>::text), "parent");
            ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(field_accessor!(<Label>::text), "child");
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
    fn style_tree_cleanup() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let mut ctx = fynix.root_ctx(&mut world);

        ctx.set(field_accessor!(<Label>::text), "z");
        let elem_a = ctx.add::<Label>();

        let mut elem_c = ElementId::PLACEHOLDER;
        let mut elem_d = ElementId::PLACEHOLDER;
        let mut elem_e = ElementId::PLACEHOLDER;
        let mut elem_f = ElementId::PLACEHOLDER;
        let elem_b = ctx.add_with::<Vertical>(|v, ctx| {
            ctx.set(field_accessor!(<Label>::text), "a");

            v.add({
                elem_c = ctx.add_with::<Vertical>(|v, ctx| {
                    ctx.set(field_accessor!(<Label>::text), "b");

                    v.add({
                        elem_d =
                            ctx.add_with::<Vertical>(|v, ctx| {
                                // Trigger `create_element` without
                                // any prior style.
                                v.add(ctx.add::<Label>());

                                ctx.set(
                                    field_accessor!(<Label>::text),
                                    "c",
                                );
                                v.add({
                                    elem_e = ctx
                                        .add_with::<Label>(|_, _| {});
                                    elem_e
                                });

                                ctx.set(
                                    field_accessor!(<Label>::text),
                                    "d",
                                );
                                v.add({
                                    elem_f = ctx.add::<Label>();
                                    elem_f
                                });
                            });
                        elem_d
                    });
                });
                elem_c
            });
        });

        let mut len = fynix.styles.styles.len();
        // Verify we have 6 styles [z, a, b, c, d].
        assert_eq!(len, 5);

        let has_primary_style = |e: &ElementId| {
            fynix
                .elements
                .metas
                .get(e)
                .and_then(|m| m.primary_style)
                .is_some()
        };

        assert!(
            ![elem_a, elem_e, elem_f].iter().any(has_primary_style)
        );
        assert!(
            [elem_b, elem_c, elem_d].iter().all(has_primary_style)
        );

        // No styles removed.
        fynix.remove_element(&elem_a);
        assert_eq!(fynix.styles.styles.len(), len);

        // No styles removed.
        fynix.remove_element(&elem_f);
        assert_eq!(fynix.styles.styles.len(), len);

        // No styles removed.
        fynix.remove_element(&elem_e);
        assert_eq!(fynix.styles.styles.len(), len);

        // [c, d] will be removed.
        fynix.remove_element(&elem_d);
        len -= 2;
        assert_eq!(fynix.styles.styles.len(), len);

        // [b] will be removed.
        fynix.remove_element(&elem_c);
        len -= 1;
        assert_eq!(fynix.styles.styles.len(), len);

        // [a] will be removed.
        fynix.remove_element(&elem_b);
        len -= 1;
        assert_eq!(fynix.styles.styles.len(), len);

        // Only [z] remains.
        assert_eq!(len, 1);
    }

    #[test]
    fn nested_style_tree_cleanup() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let mut ctx = fynix.root_ctx(&mut world);

        let mut elem_b = ElementId::PLACEHOLDER;
        let elem_a = ctx.add_with::<Vertical>(|v, ctx| {
            v.add(ctx.add_with::<Vertical>(|v, ctx| {
                v.add({
                    elem_b = ctx.add_with::<Vertical>(|v, ctx| {
                        ctx.set(field_accessor!(<Label>::text), "a");
                        v.add(ctx.add::<Label>());
                    });
                    elem_b
                })
            }));
        });

        // Verify we have 1 style [a].
        assert_eq!(fynix.styles.styles.len(), 1);

        let has_primary_style = |e: &ElementId| {
            fynix
                .elements
                .metas
                .get(e)
                .and_then(|m| m.primary_style)
                .is_some()
        };

        assert!(!has_primary_style(&elem_a));
        assert!(has_primary_style(&elem_b));

        // [a] will be removed.
        fynix.remove_element(&elem_a);
        assert_eq!(fynix.styles.styles.len(), 0);
    }
}
