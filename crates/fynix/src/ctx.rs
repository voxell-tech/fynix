use field_path::accessor::func_pointers::MutFn;
use field_path::field_accessor::FieldAccessor;

use crate::Fynix;
use crate::composer::Composer;
use crate::element::storage::ElementHandle;
use crate::element::{Element, ElementId};
use crate::init::Init;
use crate::interaction::HandlerFn;
use crate::reactive::ChangedFn;
use crate::reactive::binding::{Binding, GetFn};
use crate::reactive::watcher::{BuildFn, Watcher, WatcherElement};
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
/// [`Self::add_with`] closure, the outer `prev_style` is saved and
/// restored after the closure returns, so inner style changes do not
/// leak outward.
///
/// [`Style`]: crate::style::Style
pub struct FynixCtx<'f, 'w, W: 'static> {
    fynix: &'f mut Fynix<W>,
    pub world: &'w mut W,

    prev_style: Option<StyleId>,
    /// The first style created within the current context.
    primary_style: Option<StyleId>,
}

impl<W> FynixCtx<'_, '_, W> {
    pub(crate) fn new<'f, 'w>(
        fynix: &'f mut Fynix<W>,
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
    pub fn add<E: Element>(&mut self) -> ElementCtx<'_, W, E> {
        let element = self.create_styled::<E>();
        let id = self.fynix.elements.add(element, None);
        ElementCtx::new(self.fynix, id)
    }

    /// Like [`Self::add`], but also runs `scope` for inline mutations
    /// and nested element additions.
    #[must_use]
    pub fn add_with<E: Element>(
        &mut self,
        scope: impl FnOnce(&mut E, &mut Self),
    ) -> ElementCtx<'_, W, E> {
        let mut element = self.create_styled::<E>();
        let id = self.style_scoped(|ctx| {
            scope(&mut element, ctx);
            ctx.fynix.elements.add(element, ctx.primary_style.take())
        });
        ElementCtx::new(self.fynix, id)
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
    ) -> ElementCtx<'_, W, C::Element> {
        let style = self.create_styled::<C::Style>();
        let id =
            self.style_scoped(|ctx| composer.compose(style, ctx));
        ElementCtx::new(self.fynix, id)
    }

    /// Like [`Self::compose`], but runs `inline` after the style
    /// chain is applied, allowing per-call field overrides.
    #[must_use]
    pub fn compose_with<C: Composer<W>>(
        &mut self,
        composer: C,
        inline: impl FnOnce(&mut C::Style),
    ) -> ElementCtx<'_, W, C::Element> {
        let mut style = self.create_styled::<C::Style>();
        inline(&mut style);
        let id =
            self.style_scoped(|ctx| composer.compose(style, ctx));
        ElementCtx::new(self.fynix, id)
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

    /// Attaches a watcher to the element tree.
    ///
    /// `build` constructs a subtree and `changed` reports whether the
    /// state it reads has changed since the last build. When
    /// `changed` fires, the backend rebuilds just that subtree in
    /// place.
    ///
    /// The initial subtree is built immediately, under the current
    /// style scope. That scope is captured on the [`Watcher`] and
    /// restored on every rebuild, so a rebuilt subtree is styled like
    /// the first.
    #[must_use]
    pub fn watch(
        &mut self,
        changed: ChangedFn<W>,
        build: BuildFn<W>,
    ) -> ElementCtx<'_, W, WatcherElement> {
        self.commit_pending_styles();

        // Build the holder element, then attach the watcher as a
        // component keyed by the holder's id.
        let style_id = self.prev_style;
        let element_handle =
            self.fynix.elements.add(WatcherElement::init(), None);
        let element_id = element_handle.as_id();
        self.fynix.elements.table.insert_watcher(
            element_id,
            Watcher::new(changed, build, style_id),
        );

        // Build the initial subtree under the current style scope.
        let child = build(self);
        // Clear any uncommitted style changes to prevent leaking.
        self.fynix.styles.clear_builder();

        if let Some(child_id) = child
            && let Some(node) =
                self.fynix.elements.table.node_mut(&child_id)
        {
            node.parent_id = Some(element_id);
        }
        if let Some(elem) = self
            .fynix
            .elements
            .get_typed_mut::<WatcherElement>(&element_id)
        {
            elem.child = child;
        }

        ElementCtx::new(self.fynix, element_handle)
    }

    /// Saves the current style scope, runs `scope`, then restores it.
    ///
    /// Prevents style changes made inside `scope` from leaking into
    /// the outer scope. Any [`Self::set`] calls inside `scope` do not
    /// affect elements added after this call returns.
    ///
    /// The first style committed inside `scope` becomes the element's
    /// [`primary_style`].
    ///
    /// [`primary_style`]: crate::element::table::ElementTable::primary_style
    #[must_use]
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
    #[must_use]
    fn create_styled<S: Stylable>(&mut self) -> S {
        self.commit_pending_styles();
        let mut instance = S::init();
        if let Some(id) = &self.prev_style {
            self.fynix.styles.apply(&mut instance, id);
        }

        instance
    }

    /// Commits any pending style changes into a new committed node,
    /// advancing `prev_style` to it and recording it as
    /// `primary_style` when it is the first commit in this scope.
    fn commit_pending_styles(&mut self) {
        if self.fynix.styles.should_commit() {
            let committed_id = self.fynix.styles.current_id();

            self.fynix.styles.commit_styles(self.prev_style);
            self.prev_style = Some(committed_id);

            if self.primary_style.is_none() {
                self.primary_style = Some(committed_id);
            }
        }
    }
}

/// A freshly added element of type `E`, returned by
/// [`FynixCtx::add`] and friends and borrowed for one build
/// statement to configure it.
///
/// Carries `E` so configuration can be type-checked against the
/// element. Converts into its [`ElementId`] or [`ElementHandle`].
pub struct ElementCtx<'f, W: 'static, E: Element> {
    fynix: &'f mut Fynix<W>,
    handle: ElementHandle<E>,
}

impl<'f, W, E: Element> ElementCtx<'f, W, E> {
    fn new(
        fynix: &'f mut Fynix<W>,
        handle: ElementHandle<E>,
    ) -> Self {
        Self { fynix, handle }
    }

    /// Binds one of this element's fields to the world.
    ///
    /// When `changed_fn` reports a change, `get_fn` reads the new
    /// value from the world and it is written through `mut_fn` into
    /// this element, which is then marked dirty. Unlike a watcher,
    /// nothing is rebuilt: only the field is updated in place.
    ///
    /// Chainable, and applied each frame by
    /// [`Fynix::sync`](crate::Fynix::sync).
    pub fn bind<T>(
        self,
        changed_fn: ChangedFn<W>,
        get_fn: GetFn<W, T>,
        mut_fn: MutFn<E, T>,
    ) -> Self {
        let id = self.id();
        self.fynix.elements.table.insert_binding(
            id,
            Binding::new(changed_fn, get_fn, mut_fn),
        );
        self
    }

    /// Attaches a handler for interaction type `I` to this element.
    ///
    /// `I` is inferred from the handler's parameter. Chainable, so
    /// successive calls attach handlers for different interactions; a
    /// later call for the same `I` replaces the earlier one.
    ///
    /// The handler is a [`HandlerFn`], so a non-capturing closure
    /// coerces into one; a capturing closure does not.
    pub fn on<I: 'static>(self, handler: HandlerFn<I, W>) -> Self {
        self.fynix
            .elements
            .table
            .insert_component::<HandlerFn<I, W>>(self.id(), handler);
        self
    }

    /// Returns the element's handle.
    pub fn handle(&self) -> ElementHandle<E> {
        self.handle
    }

    /// Returns the element's id.
    pub fn id(&self) -> ElementId {
        self.handle.as_id()
    }
}

impl<W, E: Element> From<ElementCtx<'_, W, E>> for ElementId {
    fn from(ctx: ElementCtx<'_, W, E>) -> Self {
        ctx.id()
    }
}

impl<W, E: Element> From<ElementCtx<'_, W, E>> for ElementHandle<E> {
    fn from(ctx: ElementCtx<'_, W, E>) -> Self {
        ctx.handle()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use field_path::field_accessor;
    use rectree::{Constraint, NodeContext, Size, Vec2};

    use super::*;
    use crate::element::ElementBuild;
    use crate::element::layout::ElementNodes;
    use crate::init::Init;

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
        pub fn add(&mut self, id: impl Into<ElementId>) {
            self.children.push(id.into());
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
            .id()
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

                // After the inner closure, "outer" style is restored.
                v.add(inner_id);
                v.add(ctx.add::<Label>());
            })
            .id()
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
            .id()
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
        let elem_a = ctx.add::<Label>().id();

        let mut elem_c = ElementId::PLACEHOLDER;
        let mut elem_d = ElementId::PLACEHOLDER;
        let mut elem_e = ElementId::PLACEHOLDER;
        let mut elem_f = ElementId::PLACEHOLDER;
        let elem_b = ctx
            .add_with::<Vertical>(|v, ctx| {
                ctx.set(field_accessor!(<Label>::text), "a");

                v.add({
                    elem_c = ctx
                        .add_with::<Vertical>(|v, ctx| {
                            ctx.set(
                                field_accessor!(<Label>::text),
                                "b",
                            );

                            v.add({
                                elem_d =
                                    ctx.add_with::<Vertical>(
                                        |v, ctx| {
                                            // Trigger `create_element` without
                                            // any prior style.
                                            v.add(ctx.add::<Label>());

                                            ctx.set(
                                                field_accessor!(
                                                    <Label>::text
                                                ),
                                                "c",
                                            );
                                            v.add({
                                                elem_e = ctx
                                        .add_with::<Label>(|_, _| {})
                                        .id();
                                                elem_e
                                            });

                                            ctx.set(
                                                field_accessor!(
                                                    <Label>::text
                                                ),
                                                "d",
                                            );
                                            v.add({
                                                elem_f = ctx
                                                    .add::<Label>()
                                                    .id();
                                                elem_f
                                            });
                                        },
                                    )
                                    .id();
                                elem_d
                            });
                        })
                        .id();
                    elem_c
                });
            })
            .id();

        let mut len = fynix.styles.styles.len();
        // Verify we have 6 styles [z, a, b, c, d].
        assert_eq!(len, 5);

        let has_primary_style = |e: &ElementId| {
            fynix.elements.table.primary_style(e).is_some()
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
        let elem_a = ctx
            .add_with::<Vertical>(|v, ctx| {
                v.add(ctx.add_with::<Vertical>(|v, ctx| {
                    v.add({
                        elem_b = ctx
                            .add_with::<Vertical>(|v, ctx| {
                                ctx.set(
                                    field_accessor!(<Label>::text),
                                    "a",
                                );
                                v.add(ctx.add::<Label>());
                            })
                            .id();
                        elem_b
                    })
                }));
            })
            .id();

        // Verify we have 1 style [a].
        assert_eq!(fynix.styles.styles.len(), 1);

        let has_primary_style = |e: &ElementId| {
            fynix.elements.table.primary_style(e).is_some()
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
        type Element = Label;

        fn compose(
            self,
            style: LabelStyle,
            ctx: &mut FynixCtx<'_, '_, ()>,
        ) -> ElementHandle<Self::Element> {
            ctx.add_with::<Label>(|l, _| {
                l.text = style.text;
            })
            .handle()
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
            ctx.compose(LabelComposer).id()
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
                .id()
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
            let inner = ctx
                .compose_with(LabelComposer, |s| {
                    s.text = "inner";
                })
                .id();
            // Styles set inside compose must not affect elements
            // added after it returns.
            let outer = ctx
                .add_with::<Label>(|l, _| {
                    l.text = "outer";
                })
                .id();
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
