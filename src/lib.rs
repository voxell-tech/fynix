#![no_std]

extern crate alloc;

use core::any::{Any, TypeId};

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use field_path::accessor::FieldAccessorRegistry;
use field_path::field::{Field, UntypedField};
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use rectree::Rectree;
use sparse_map::{Key, SparseMap};
use vello::kurbo::Stroke;
use vello::peniko::Color;

#[derive(Default)]
pub struct Styles {
    maps: TypeSparseMaps,
    // TODO(nixon): Possibly upstream the concept of partial field/field path to the `field_path` crate.
    // This helps reduce memory footprint here because we only need to store parts of the `UntypedField`.
    /// Stores all the fields associated to the target source type.
    type_to_fields: HashMap<TypeId, Vec<UntypedField>>,
    field_to_key: HashMap<UntypedField, Key>,
}

impl Styles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set<S: 'static, T: 'static>(
        &mut self,
        field: Field<S, T>,
        value: T,
    ) {
        let untyped = field.untyped();
        // Get or create the map for type `T`.
        let map = self.maps.get_or_create::<T>();
        match self.type_to_fields.entry(TypeId::of::<S>()) {
            Entry::Occupied(mut entry) => {
                if !entry.get_mut().contains(&untyped) {
                    entry.get_mut().push(untyped);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![untyped]);
            }
        }

        // Update or insert.
        if let Some(original_value) = self
            .field_to_key
            .get(&untyped)
            .and_then(|key| map.get_mut(key))
        {
            *original_value = value;
        } else {
            let key = map.insert(value);
            self.field_to_key.insert(untyped, key);
        }
    }

    pub fn get<S: 'static, T: 'static>(
        &self,
        field: Field<S, T>,
    ) -> Option<&T> {
        let untyped = field.untyped();
        self.maps
            .get::<T>()
            .and_then(|m| m.get(self.field_to_key.get(&untyped)?))
    }

    pub fn fields_of<S: 'static>(&self) -> Option<&[UntypedField]> {
        self.type_to_fields
            .get(&TypeId::of::<S>())
            .map(|v| v.as_slice())
    }

    // TODO(nixon): Validate that we don't need this!
    // TODO(nixon): If so, use Vec<T> instead of SparseMap<T>!
    //
    // pub fn remove<S: 'static, T: 'static>(
    //     &mut self,
    //     field: Field<S, T>,
    // ) -> Option<T> {
    //     let untyped = field.untyped();
    //     self.maps
    //         .get_mut::<T>()
    //         .and_then(|m| m.remove(self.field_to_key.get(&untyped)?))
    // }
}

// pub struct FieldPath {
//     /// See [`Field::field_path`].
//     field_path: &'static str,
// }

// impl FieldPath {
//     pub fn new(untyped: &UntypedField) -> Self {
//         Self {
//             field_path: untyped.field_path(),
//         }
//     }

//     pub fn into_untyped<S: 'static, T: 'static>(
//         &self,
//     ) -> UntypedField {
//         UntypedField::new::<S, T>(self.field_path)
//     }
// }

pub struct FynixCtx {
    pub rectree: Rectree,
    pub registry: FieldAccessorRegistry,
    pub style_chain: Vec<Styles>,
}

impl FynixCtx {}

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

// pub trait CloneBox<T: ?Sized>: Any {
//     fn clone_box(&self) -> Box<T>;
// }

// impl<T> CloneBox<DynAnySparseMap> for T
// where
//     T: Clone + Any + 'static,
// {
//     fn clone_box(&self) -> Box<DynAnySparseMap> {
//         Box::new(self.clone())
//     }
// }

// impl Clone for Box<DynAnySparseMap> {
//     fn clone(&self) -> Self {
//         self.clone_box()
//     }
// }

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

#[cfg(test)]
mod tests {
    use field_path::field;

    use super::*;

    #[test]
    fn set_get_style() {
        let mut styles = Styles::new();
        let field = field!(<Margin>::left);
        styles.set(field, 4.0);
        assert_eq!(styles.get(field).copied(), Some(4.0));

        // Override.
        styles.set(field, 42.0);
        assert_eq!(styles.get(field).copied(), Some(42.0));
    }
}
