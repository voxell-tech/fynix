use alloc::string::String;
use alloc::vec::Vec;
use rectree::kurbo::{Size, Vec2};
use rectree::layout::{Constraint, LayoutSolver, Positioner};
use rectree::node::RectNode;
use rectree::{NodeId, Rectree};
use vello::Scene;
use vello::peniko::Color;

use crate::type_table::TypeTable;

pub struct BuildCtx;

pub type ElementTable = TypeTable<NodeId>;

pub trait Element: 'static {
    fn build(
        &self,
        ctx: &BuildCtx,
        node: &RectNode,
        tree: &Rectree,
        positioner: &mut Positioner,
    ) -> Size;

    fn constraint(
        &self,
        parent_constraint: Constraint,
    ) -> Constraint {
        parent_constraint
    }

    #[expect(unused_variables)]
    fn push_child(&mut self, id: NodeId) {}

    fn draw(&self) -> Option<Scene> {
        None
    }
}

// pub struct ElementWrapper<'a, E>
// where
//     E: Element,
// {
//     element: &'a E,
//     context: &'a BuildCtx,
// }

// impl<E> LayoutSolver for ElementWrapper<'_, E>
// where
//     E: Element,
// {
//     fn build(
//         &self,
//         node: &RectNode,
//         tree: &Rectree,
//         positioner: &mut Positioner,
//     ) -> Size {
//         self.element.build(self.context, node, tree, positioner)
//     }

//     fn constraint(
//         &self,
//         parent_constraint: Constraint,
//     ) -> Constraint {
//         self.element.constraint(parent_constraint)
//     }
// }

pub struct Label {
    pub text: String,
    pub fill: Color,
    pub stroke: Color,
}

impl Element for Label {
    fn build(
        &self,
        ctx: &BuildCtx,
        _: &RectNode,
        _: &Rectree,
        _: &mut Positioner,
    ) -> Size {
        todo!()
    }

    fn draw(&self) -> Option<Scene> {
        todo!()
    }
}

// /// Spacing used within a block.
// pub struct Space {
//     spacing: f32,
// }

/// The fundamental building block of nodes in Fynix. Everything
/// within it will only flow in 1 [`Direction`].
#[derive(Default)]
pub struct Block {
    pub direction: Direction,
    pub clip: bool,
    children: Vec<NodeId>,
}

impl Element for Block {
    fn build(
        &self,
        _: &BuildCtx,
        _: &RectNode,
        tree: &Rectree,
        positioner: &mut Positioner,
    ) -> Size {
        let mut max_height = 0.0;
        let mut cursor = 0.0;

        // TODO: This is only horizontal.
        for id in self.children.iter() {
            let child_node = tree.get(id);
            let child_size = child_node.size();

            positioner.set(*id, Vec2::new(cursor, 0.0));
            cursor += child_size.width;

            // Track the tallest child
            if child_size.height > max_height {
                max_height = child_size.height;
            }
        }

        Size::new(cursor, max_height)
    }
}

/// Orthogonal direction in 2D space.
#[derive(Default)]
pub enum Direction {
    /// Top to bottom.
    #[default]
    TTB,
    /// Bottom to top.
    BTT,
    /// Left to right.
    LTR,
    /// Right to left.
    RTL,
}
