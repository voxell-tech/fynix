#![no_std]

extern crate alloc;

use core::any::{Any, TypeId};
use core::marker::PhantomData;

use alloc::boxed::Box;
use alloc::vec::Vec;
use hashbrown::HashMap;
use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};
use sparse_map::{Key, SparseMap};
use vello::kurbo::Stroke;
use vello::peniko::Color;

use crate::type_map::TypeMaps;

pub mod any_wrapper;
pub mod type_map;
pub mod widgets;

pub mod style {
    /// Concept: Every widget is represented via style.
    /// ```no_compile
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

pub struct FynixWorld {
    pub tree: Rectree,
    pub style_maps: TypeMaps<NodeId>,
    pub style_caches: HashMap<TypeId, Vec<NodeId>>,
}

impl FynixWorld {
    pub fn create_widget<W>(&self, node_id: &NodeId) -> W
    where
        W: Widget,
    {
        let type_id = TypeId::of::<W>();
        let mut widget = None;

        if let Some(cache) = self.style_caches.get(&type_id) {
            let depth = self.tree.get(node_id).depth();
            for cache_id in cache.iter().rev() {
                let cache_depth = self.tree.get(cache_id).depth();

                if cache_depth < depth
                    || (cache_depth == depth && cache_id == node_id)
                {
                    widget = self.style_maps.get(node_id).cloned();
                    break;
                }
            }
        }

        widget.unwrap_or_default()
    }

    pub fn set_style<W, F>(
        &mut self,
        node_id: &NodeId,
        style: impl Into<Style<W, F>>,
    ) where
        W: Widget,
        F: FnOnce(&mut W),
    {
        let type_id = TypeId::of::<W>();
        let mut widget = self.create_widget(node_id);
        style.into().apply(&mut widget);

        self.style_maps.insert(*node_id, widget);
        self.style_caches.entry(type_id).or_default().push(*node_id);
    }
}

pub struct Style<W, F>
where
    W: Widget,
    F: FnOnce(&mut W),
{
    pub func: F,
    _marker: PhantomData<W>,
}

impl<W, F> Style<W, F>
where
    W: Widget,
    F: FnOnce(&mut W),
{
    pub fn apply(self, widget: &mut W) {
        (self.func)(widget)
    }
}

pub trait Widget: Default + Clone + 'static {}
impl<T> Widget for T where T: Default + Clone + 'static {}

pub struct WidgetNode<W>
where
    W: Widget,
{
    _marker: PhantomData<W>,
}

impl<W> WidgetNode<W>
where
    W: Widget,
{
    pub const fn type_id() -> TypeId {
        TypeId::of::<W>()
    }
}

impl LayoutWorld for FynixWorld {
    fn get_solver(&self, id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

/// A generic container that maps [`TypeId::of::<T>()`] to
/// [`SparseMap<T>`] with guarantee of correctness on construction.
#[derive(Default)]
pub struct TypeSparseMaps(HashMap<TypeId, Box<DynAnySparseMap>>);

impl TypeSparseMaps {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Returns a reference to the map for type T, creating it if it
    /// doesn't exist.
    pub fn get_or_create<T: 'static>(&mut self) -> &mut SparseMap<T> {
        self.0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| DynAnySparseMap::new::<T>())
            .as_map_mut()
            // SAFETY: We ensured the creation up there!
            .unwrap()
    }

    /// Returns a reference to the map for type `T` if it exists.
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&SparseMap<T>> {
        self.0.get(&TypeId::of::<T>()).map(|m| {
            // SAFETY: We ensured correctness on construction.
            m.as_map_ref().unwrap()
        })
    }

    /// Returns a mutable reference to the map for type `T` if it exists.
    #[must_use]
    pub fn get_mut<T: 'static>(
        &mut self,
    ) -> Option<&mut SparseMap<T>> {
        self.0.get_mut(&TypeId::of::<T>()).map(|m| {
            // SAFETY: We ensured correctness on construction.
            m.as_map_mut().unwrap()
        })
    }
}

trait AnySparseMap: Any + 'static {}

impl<T> AnySparseMap for T where T: Any + 'static {}

type DynAnySparseMap = dyn AnySparseMap;

impl DynAnySparseMap {
    fn new<T: 'static>() -> Box<dyn AnySparseMap> {
        Box::new(SparseMap::<T>::new())
    }

    fn as_map_ref<T: 'static>(&self) -> Option<&SparseMap<T>> {
        (self as &dyn Any).downcast_ref::<SparseMap<T>>()
    }

    fn as_map_mut<T: 'static>(
        &mut self,
    ) -> Option<&mut SparseMap<T>> {
        (self as &mut dyn Any).downcast_mut::<SparseMap<T>>()
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

fn example() {
    let default_frame = Frame {
        fill: Some(Color::WHITE),
        ..Default::default()
    };

    let set_frame = Frame {
        fill: None,
        ..Default::default()
    };
}

// #[derive(FieldReg)]
pub struct Margin {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    // #[field_reg(skip)]
    pub down: f32,
}

#[cfg(test)]
mod tests {
    use field_path::field;

    use super::*;

    #[test]
    fn set_get_style() {
        // let mut styles = Styles::new();
        // let field = field!(<Margin>::left);
        // styles.set(field, 4.0);
        // assert_eq!(styles.get(field).copied(), Some(4.0));

        // // Override.
        // styles.set(field, 42.0);
        // assert_eq!(styles.get(field).copied(), Some(42.0));
    }
}
