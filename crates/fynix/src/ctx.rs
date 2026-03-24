use field_path::field_accessor::FieldAccessor;

use crate::element::{Element, ElementId, Elements};
use crate::style::{StyleId, StyleValue, Styles};

// TODO: Merge into FynixCtx.
pub struct BuildCtx<'a> {
    parent_style_id: Option<StyleId>,
    pub(crate) elements: &'a mut Elements,
    pub(crate) styles: &'a mut Styles,
}

impl BuildCtx<'_> {
    pub(crate) fn new<'a>(
        parent_style_id: Option<StyleId>,
        elements: &'a mut Elements,
        styles: &'a mut Styles,
    ) -> BuildCtx<'a> {
        BuildCtx {
            parent_style_id,
            elements,
            styles,
        }
    }

    #[must_use]
    pub fn add<E>(&mut self) -> ElementId
    where
        E: Element,
    {
        let element = self.create_element::<E>();
        self.elements.add(element)
    }

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

        self.elements.add(element)
    }

    pub fn set<E, T>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) where
        E: Element,
        T: StyleValue,
    {
        self.styles.set(field_accessor, value);
    }

    /// Commits pending styles if needed, creates a new element (`E`),
    /// and applies styles to it.
    fn create_element<E>(&mut self) -> E
    where
        E: Element,
    {
        if self.styles.should_commit() {
            let committed_id = self.styles.current_id();
            self.styles.commit_styles(self.parent_style_id);
            self.parent_style_id = Some(committed_id);
        }
        let mut element = E::new();
        if let Some(id) = &self.parent_style_id {
            self.styles.apply(&mut element, id);
        }
        element
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    use field_path::field_accessor;

    use crate::Fynix;
    use crate::element::{Element, ElementId};

    #[derive(Default, Clone)]
    struct Label {
        pub text: String,
    }

    impl Element for Label {
        fn new() -> Self {
            Self::default()
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
    }

    #[test]
    fn style_applied_after_set() {
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx();
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
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx();
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
        let mut fynix = Fynix::new();
        let root_id = {
            let mut ctx = fynix.root_ctx();
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
