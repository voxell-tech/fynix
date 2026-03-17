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
