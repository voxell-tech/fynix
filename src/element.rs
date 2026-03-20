use alloc::vec::Vec;
use rectree::kurbo::{Axis, Size, Vec2};
use rectree::layout::{Constraint, LayoutSolver, Positioner};
use rectree::node::RectNode;
use rectree::{NodeId, Rectree};
use vello::peniko::Color;

use crate::element;

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

/// Example implementation of a flexbox container
/// Positions children according to the main/cross axis
///
///   (Row)              (Column)
///
///     |                   |
///  -------> main       ---|---> cross
///     |                   |
///     V                   V
///   cross               main
///
#[derive(Default, Debug, Clone)]
pub struct FlexContainer {
    pub size: Size,
    pub flex_direction: FlexDirection,
    pub main_alignment: Alignment,
    pub cross_alignment: Alignment,
}
impl FlexContainer {
    pub fn new(size: Size) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    pub fn with_alignments(
        size: Size,
        flex_direction: FlexDirection,
        main_alignment: Option<Alignment>,
        cross_alignment: Option<Alignment>,
    ) -> Self {
        Self {
            size,
            flex_direction,
            main_alignment: main_alignment
                .unwrap_or(Alignment::default()),
            cross_alignment: cross_alignment
                .unwrap_or(Alignment::default()),
            ..Default::default()
        }
    }
}

#[derive(Default, Debug, Clone)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}
impl FlexDirection {
    fn axis_pairs(&self) -> (Axis, Axis) {
        match self {
            FlexDirection::Row => (Axis::Horizontal, Axis::Vertical),
            // the formatter keeps putting these on different lines lol
            FlexDirection::Column => {
                (Axis::Vertical, Axis::Horizontal)
            }
        }
    }
}

/// -- Main Axis alignment --
/// Start
/// ┌───────────────────┐
/// │┌─┐┌─┐┌─┐          │
/// │└─┘└─┘└─┘          │
/// └───────────────────┘
///
/// End
/// ┌───────────────────┐
/// │          ┌─┐┌─┐┌─┐│
/// │          └─┘└─┘└─┘│
/// └───────────────────┘
///
/// Center
/// ┌───────────────────┐
/// │     ┌─┐┌─┐┌─┐     │
/// │     └─┘└─┘└─┘     │
/// └───────────────────┘
///
/// SpaceBetween
/// ┌───────────────────┐
/// │┌─┐     ┌─┐     ┌─┐│
/// │└─┘     └─┘     └─┘│
/// └───────────────────┘
///
/// SpaceEvenly
/// ┌─────────────────────┐
/// │   ┌─┐   ┌─┐   ┌─┐   │
/// │   └─┘   └─┘   └─┘   │
/// └─────────────────────┘
///
///
/// -- Cross axis alignment--
/// Start                        End                         Center, SpaceEvenly, SpaceBetween
/// ┌─────────────────────┐      ┌─────────────────────┐     ┌─────────────────────┐
/// │   ┌─┐   ┌─┐   ┌─┐   │      │                     │     │                     │
/// │   │ │   └─┘   │ │   │      │                     │     │   ┌─┐         ┌─┐   │
/// │   │ │         │ │   │      │   ┌─┐         ┌─┐   │     │   │ │   ┌─┐   │ │   │
/// │   └─┘         └─┘   │      │   │ │         │ │   │     │   │ │   └─┘   │ │   │
/// │                     │      │   │ │   ┌─┐   │ │   │     │   └─┘         └─┘   │
/// │                     │      │   └─┘   └─┘   └─┘   │     │                     │
/// └─────────────────────┘      └─────────────────────┘     └─────────────────────┘
///
#[derive(Default, Debug, Clone)]
pub enum Alignment {
    Start,
    End,
    #[default]
    Center,
    SpaceEvenly,
    SpaceBetween,
}
impl Alignment {
    pub fn position(
        &self,
        axis: Axis,
        parent: &Size,
        elements: &mut Vec<(&NodeId, Vec2, Size)>,
        is_main_axis: bool,
    ) {
        let parent_length = match axis {
            Axis::Horizontal => parent.width,
            Axis::Vertical => parent.height,
        };

        let element_length = |s: &Size| match axis {
            Axis::Horizontal => s.width,
            Axis::Vertical => s.height,
        };

        if is_main_axis {
            match self {
                Alignment::Start => {
                    let mut offset = 0.0;
                    for (_, v, s) in elements.iter_mut() {
                        v.set_coord(axis, offset);
                        offset += element_length(s);
                    }
                }
                Alignment::End => {
                    let total: f64 = elements
                        .iter()
                        .map(|(_, _, s)| element_length(s))
                        .sum();
                    let mut offset = parent_length - total;
                    for (_, v, s) in elements.iter_mut() {
                        v.set_coord(axis, offset);
                        offset += element_length(s);
                    }
                }
                Alignment::Center => {
                    let total: f64 = elements
                        .iter()
                        .map(|(_, _, s)| element_length(s))
                        .sum();
                    let mut offset = (parent_length - total) * 0.5;
                    for (_, v, s) in elements.iter_mut() {
                        v.set_coord(axis, offset);
                        offset += element_length(s);
                    }
                }
                Alignment::SpaceEvenly => {
                    let total: f64 = elements
                        .iter()
                        .map(|(_, _, s)| element_length(s))
                        .sum();
                    let count = elements.len();
                    if count > 0 {
                        let spacing = (parent_length - total)
                            / (count as f64 + 1.0);
                        let mut offset = spacing;
                        for (_, v, s) in elements.iter_mut() {
                            v.set_coord(axis, offset);
                            offset += element_length(s) + spacing;
                        }
                    }
                }
                Alignment::SpaceBetween => {
                    let count = elements.len();
                    if count <= 1 {
                        // Single or no element: center it
                        for (_, v, s) in elements.iter_mut() {
                            v.set_coord(
                                axis,
                                (parent_length - element_length(s))
                                    * 0.5,
                            );
                        }
                    } else {
                        let total: f64 = elements
                            .iter()
                            .map(|(_, _, s)| element_length(s))
                            .sum();
                        let spacing = (parent_length - total)
                            / (count as f64 - 1.0);
                        let mut offset = 0.0;
                        for (_, v, s) in elements.iter_mut() {
                            v.set_coord(axis, offset);
                            offset += element_length(s) + spacing;
                        }
                    }
                }
            }
        } else {
            match self {
                Alignment::Start => {
                    for (_, v, _) in elements.iter_mut() {
                        v.set_coord(axis, 0.0);
                    }
                }
                Alignment::End => {
                    for (_, v, s) in elements.iter_mut() {
                        v.set_coord(
                            axis,
                            parent_length - element_length(s),
                        );
                    }
                }
                Alignment::Center
                | Alignment::SpaceBetween
                | Alignment::SpaceEvenly => {
                    for (_, v, s) in elements.iter_mut() {
                        v.set_coord(
                            axis,
                            (parent_length - element_length(s)) * 0.5,
                        );
                    }
                }
            }
        }
    }
}

impl LayoutSolver for FlexContainer {
    fn build(
        &self,
        node: &RectNode,
        tree: &Rectree,
        positioner: &mut Positioner,
    ) -> Size {
        let mut elements = node
            .children()
            .iter()
            .map(|id| (id, tree.get(id)))
            .map(|(id, c)| (id, c.translation(), c.size()))
            .collect::<Vec<(&NodeId, Vec2, Size)>>();

        // Get main and cross axes
        let (main_axis, cross_axis) =
            self.flex_direction.axis_pairs();

        // Along main axis
        self.main_alignment.position(
            main_axis,
            &self.size,
            &mut elements,
            true,
        );

        // Along cross axis
        self.cross_alignment.position(
            cross_axis,
            &self.size,
            &mut elements,
            false,
        );

        // Apply
        for (id, translation, _) in elements {
            positioner.set(*id, translation);
        }

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
