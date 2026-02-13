#![no_std]

extern crate alloc;

use core::any::{Any, TypeId};

use alloc::boxed::Box;
use field_path::accessor::{Accessor, FieldAccessorRegistry};
use field_path::field::{Field, UntypedField};
use hashbrown::HashMap;
use rectree::Rectree;
use sparse_map::{Key, SparseMap};
use vello::kurbo::Stroke;
use vello::peniko::Color;

/// A type-erased wrapper around a [`SparseMap<T>`]
pub struct AnyBucket {
    data: Box<dyn Any>,
}

impl AnyBucket {
    pub fn new<T: 'static>() -> Self {
        Self {
            data: Box::new(SparseMap::<T>::new()),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<&SparseMap<T>> {
        self.data.downcast_ref()
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut SparseMap<T>> {
        self.data.downcast_mut()
    }
}

pub struct FynixCtx {
    pub rectree: Rectree,
    pub registry: FieldAccessorRegistry,
    buckets: HashMap<TypeId, AnyBucket>,
    field_to_key: HashMap<UntypedField, Key>,
}

impl FynixCtx {
    pub fn register<S, T>(&mut self, field: Field<S, T>, accessor: Accessor<S, T>) {
        self.registry.register_typed(field, accessor);
    }

    pub fn set_style<S: 'static, T: 'static>(&mut self, field: Field<S, T>, value: T) {
        let type_id = TypeId::of::<T>();
        let untyped = field.untyped();

        // Get or create the bucket for type `T`.
        let bucket = self
            .buckets
            .entry(type_id)
            .or_insert_with(|| AnyBucket::new::<T>())
            .get_mut::<T>()
            // SAFETY: We already ensure the creation up there!
            .unwrap();

        // Update or insert.
        if let Some(original_value) = self
            .field_to_key
            .get(&untyped)
            .and_then(|key| bucket.get_mut(key))
        {
            *original_value = value;
        } else {
            let key = bucket.insert(value);
            self.field_to_key.insert(untyped, key);
        }
    }

    pub fn get_style<S: 'static, T: 'static>(&self, field: Field<S, T>) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let key = self.field_to_key.get(&field.untyped())?;
        let bucket = self.buckets.get(&type_id)?.get::<T>()?;
        bucket.get(key)
    }
}

pub struct Frame {
    pub inner_margin: Option<Margin>,
    pub outer_margin: Option<Margin>,
    pub fill: Option<Color>,
    pub stroke: Option<(Stroke, Color)>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub struct Margin {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub down: f32,
}
