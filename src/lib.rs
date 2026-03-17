#![no_std]

extern crate alloc;

use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};
use vello::kurbo::Stroke;
use vello::peniko::Color;

pub mod any_wrapper;
pub mod field_index;
pub mod registry;
pub mod rule_set;
pub mod setter;
pub mod type_table;
pub mod widgets;

// any_wrapper!({
//     mod any_sparse_map {
//         trait AnySparseMap: SparseMap {}
//     }
// });

pub mod vision {
    /// Concept: Every widget is represented via style.
    /// ```rust,no_run,ignore
    /// // Instantiating a widget can be expressed via:
    /// let mut frame_a = ctx.instantiate_with::<Frame>(Style(|frame| { frame.height = Some(10.0) }));
    /// let frame_b = ctx.instantiate::<Frame>();
    ///
    /// // At any point in the layout flow, a style override can occur:
    /// ctx.set_style::<Frame>(Style(|frame| { frame.height = Some(20.0) }));
    /// let frame_c = ctx.instantiate::<Frame>(); // This will have height as `20.0`.
    ///
    /// // We can also support scoped style:
    /// ctx.scope(|ctx| {
    ///     ctx.set_style::<Frame>(Style(|frame| { frame.height = Some(5.0) }));
    ///     let frame_in_scope = ctx.instantiate::<Frame>(); // This will have height as `5.0`.
    /// });
    ///
    /// // Each widget should be able to hold style overrides:
    /// frame_a.set_style::<Frame>(Style(|frame| { frame.fill = None }));
    /// // The style override will only affect its descendants.
    /// // TODO: Better ergonomics? Should we not support this?
    /// // I can see some potential bad outcomes from this:
    /// // If the style is being set in the widget itself, this will have no effect, leading to confusion...
    /// ```
    pub struct Dummy;
}

// pub struct Container<'a> {
//     pub age: &'a mut u32,
// }

// struct Ctx<'a> {
//     pub world: &'a mut Fynix,
//     // .. bevy world,
//     // .. ui world
// }

// impl Container<'_> {
//     pub fn show(ctx: Ctx) {
//         // ctx.get_style
//     }
// }

pub struct Fynix {
    pub tree: Rectree,
}

impl LayoutWorld for Fynix {
    fn get_solver(&self, _id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

#[derive(Default)]
pub struct Frame {
    pub inner_margin: Option<Margin>,
    pub outer_margin: Option<Margin>,
    pub fill: Option<Color>,
    pub stroke: Option<(Stroke, Color)>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

// #[derive(FieldReg)]
pub struct Margin {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    // #[field_reg(skip)]
    pub down: f32,
}

pub struct CustomData;

pub struct CustomWidget<'a> {
    pub custom: &'a CustomData,
    pub width: u32,
    pub trans: Transform,
}

#[derive(Default)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
}

// impl CustomWidget<_> {
//     pub fn set_width() -> (Field, Accesor, u32) {}
//     pub fn set_transform() -> (Field, Accesor, Transform) {}
//     pub fn set_transform_x() -> (Field, Accesor, Transform) {}
// }

impl<'a> Widget for CustomWidget<'a> {
    type Essential = &'a CustomData;

    fn create(inputs: Self::Essential) -> Self {
        Self {
            custom: inputs,
            width: 10,
            trans: Transform::default(),
        }
    }
}

pub trait Widget {
    type Essential;

    fn create(inputs: Self::Essential) -> Self;
}
