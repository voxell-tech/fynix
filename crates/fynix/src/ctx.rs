use field_path::field_accessor::FieldAccessor;

use crate::Fynix;
use crate::composer::Composer;
use crate::element::{Element, ElementId};
use crate::style::{Stylable, StyleId, StyleValue};

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
/// [`Self::add_with`] closure, the outer `prev_style` is saved
/// and restored after the closure returns, so inner style changes do
/// not leak outward.
///
/// [`Style`]: crate::style::Style
pub struct FynixCtx<'f, 'w, W> {
    fynix: &'f mut Fynix,
    pub world: &'w mut W,

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
        let element = self.create_styled::<E>();
        self.fynix.elements.add(element, None)
    }

    /// Like [`Self::add`], but also runs `scope` for inline mutations
    /// and nested element additions.
    #[must_use]
    pub fn add_with<E: Element>(
        &mut self,
        scope: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId {
        let mut element = self.create_styled::<E>();
        self.style_scoped(|ctx| {
            scope(&mut element, ctx);
            ctx.fynix.elements.add(element, ctx.primary_style.take())
        })
    }

    /// Queues a style default: field `T` on type `S` will be set to
    /// `value` for all elements added after this call (within the
    /// current scope).
    pub fn set<S: Stylable, T: StyleValue>(
        &mut self,
        field_accessor: FieldAccessor<S, T>,
        value: T,
    ) {
        self.fynix.styles.set(field_accessor, value);
    }

    /// Runs `composer`, passing it a style instance built from the
    /// current style chain.
    ///
    /// The composer's scope is isolated: any [`Self::set`] calls made
    /// inside [`Composer::compose`] do not leak into the outer scope.
    #[must_use]
    pub fn compose<C: Composer<W>>(
        &mut self,
        composer: C,
    ) -> ElementId {
        let style = self.create_styled::<C::Style>();
        self.style_scoped(|ctx| composer.compose(style, ctx))
    }

    /// Like [`Self::compose`], but runs `inline` after the style chain
    /// is applied, allowing per-call field overrides.
    #[must_use]
    pub fn compose_with<C: Composer<W>>(
        &mut self,
        composer: C,
        inline: impl FnOnce(&mut C::Style),
    ) -> ElementId {
        let mut style = self.create_styled::<C::Style>();
        inline(&mut style);
        self.style_scoped(|ctx| composer.compose(style, ctx))
    }

    /// Saves the current style scope, runs `scope`, then
    /// restores it.
    ///
    /// Prevents style changes made inside `scope` from leaking
    /// into the outer scope. Any [`Self::set`] calls inside
    /// `scope` do not affect elements added after this call
    /// returns. The first style committed inside `scope` becomes
    /// the element's `primary_style` - when that element is
    /// removed, its style subtree is also removed.
    fn style_scoped<T>(
        &mut self,
        scope: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let prev_style_id = self.prev_style;
        let primary_style = self.primary_style.take();

        let result = scope(self);

        // Restore pre-closure style state.
        self.prev_style = prev_style_id;
        self.primary_style = primary_style;
        // Clear any uncommitted style changes to prevent leaking.
        self.fynix.styles.clear_builder();

        result
    }

    /// Commits any pending style changes, constructs `S::init()`, and
    /// applies the current style chain to it.
    fn create_styled<S: Stylable>(&mut self) -> S {
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

        let mut instance = S::init();
        if let Some(id) = &self.prev_style {
            self.fynix.styles.apply(&mut instance, id);
        }

        instance
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use field_path::field_accessor;
    use rectree::{Constraint, NodeContext, Size, Vec2};

    use crate::element::ElementBuild;
    use crate::element::layout::ElementNodes;
    use crate::init::Init;

    use super::*;

    #[derive(Init, Element, Clone)]
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

    #[derive(Init, Element, Clone)]
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

    #[derive(Init)]
    struct LabelStyle {
        pub text: &'static str,
    }

    struct LabelComposer;

    impl Composer<()> for LabelComposer {
        type Style = LabelStyle;

        fn compose(
            self,
            style: LabelStyle,
            ctx: &mut FynixCtx<'_, '_, ()>,
        ) -> ElementId {
            ctx.add_with::<Label>(|l, _| {
                l.text = style.text;
            })
        }
    }

    #[test]
    fn compose_applies_style_chain() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(
                field_accessor!(<LabelStyle>::text),
                "from_chain",
            );
            ctx.compose(LabelComposer)
        };

        let label = fynix.elements.get_typed::<Label>(&id).unwrap();
        assert_eq!(label.text, "from_chain");
    }

    #[test]
    fn compose_with_inline_overrides_chain() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.set(
                field_accessor!(<LabelStyle>::text),
                "from_chain",
            );
            ctx.compose_with(LabelComposer, |s| s.text = "inline")
        };

        let label = fynix.elements.get_typed::<Label>(&id).unwrap();
        assert_eq!(label.text, "inline");
    }

    #[test]
    fn compose_scope_does_not_leak() {
        let mut world = ();
        let mut fynix = Fynix::new();
        let (inner_id, outer_id) = {
            let mut ctx = fynix.root_ctx(&mut world);
            let inner = ctx.compose_with(LabelComposer, |s| {
                s.text = "inner";
            });
            // Styles set inside compose must not affect elements added
            // after it returns.
            let outer = ctx.add_with::<Label>(|l, _| {
                l.text = "outer";
            });
            (inner, outer)
        };

        let inner =
            fynix.elements.get_typed::<Label>(&inner_id).unwrap();
        let outer =
            fynix.elements.get_typed::<Label>(&outer_id).unwrap();
        assert_eq!(inner.text, "inner");
        assert_eq!(outer.text, "outer");
    }
}
