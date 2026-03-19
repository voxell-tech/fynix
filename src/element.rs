use rectree::Rectree;
use rectree::kurbo::Size;
use rectree::layout::{Constraint, LayoutSolver, Positioner};
use rectree::node::RectNode;
use vello::peniko::Color;

/// A widget that forces a specific size that ignore parent constraints.
#[derive(Default, Debug, Clone)]
pub struct FixedSize {
    pub size: Size,
    pub color: Option<Color>,
}

impl LayoutSolver for FixedSize {
    fn constraint(&self, _parent: Constraint) -> Constraint {
        // Fixed size yield fixed contraint.
        Constraint::fixed(self.size.width, self.size.height)
    }

    fn build(
        &self,
        _node: &RectNode,
        _tree: &Rectree,
        _positioner: &mut Positioner,
    ) -> Size {
        self.size
    }
}

impl FixedSize {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

pub struct Label<'a> {
    pub text: &'a str,
    pub fill: Color,
    pub stroke: Color,
}

/// A widget that respects its parent's constraints
/// Will get the minimum between the widget's and parent's sizes.
#[derive(Default, Debug, Clone)]
pub struct DynamicContainer {
    pub size: Size,
}
impl DynamicContainer {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }
}

impl LayoutSolver for DynamicContainer {
    fn build(
        &self,
        _node: &RectNode,
        _tree: &Rectree,
        _positioner: &mut Positioner,
    ) -> Size {
        self.size
    }

    fn constraint(
        &self,
        parent_constraint: Constraint,
    ) -> Constraint {
        Constraint::fixed(
            self.size.width.min(
                parent_constraint.width.unwrap_or(core::f64::MAX),
            ),
            self.size.height.min(
                parent_constraint.height.unwrap_or(core::f64::MAX),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::element::{DynamicContainer, FixedSize};
    use rectree::kurbo::Size;
    use rectree::layout::{Constraint, LayoutSolver};

    #[test]
    fn fixed_size_ignores_parent_constraints() {
        let fs = FixedSize::new(Size::new(30.0, 30.0));
        let tight_parent = Constraint::fixed(500.0, 500.0);
        let result = fs.constraint(tight_parent);

        let expected = Constraint::fixed(30.0, 30.0);
        assert_eq!(result, expected);
    }

    #[test]
    fn dynamic_container_respects_tight_parent() {
        let fs = DynamicContainer::new(Size::new(500.0, 500.0));
        let tight_parent = Constraint::fixed(20.0, 20.0);
        let result = fs.constraint(tight_parent);

        let expected = Constraint::fixed(20.0, 20.0);
        assert_eq!(result, expected);
    }

    #[test]
    fn dynamic_container_uses_own_size_when_smaller() {
        let fs = DynamicContainer::new(Size::new(100.0, 100.0));
        let loose_parent = Constraint::fixed(200.0, 200.0);
        let result = fs.constraint(loose_parent);

        let expected = Constraint::fixed(100.0, 100.0);
        assert_eq!(result, expected);
    }
}
