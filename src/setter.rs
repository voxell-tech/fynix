use core::any::TypeId;
use core::hash::Hash;
use core::marker::PhantomData;

use field_path::field::UntypedField;
use field_path::registry::FieldAccessorRegistry;

use crate::type_table::TypeTable;

// TODO(nixon): Really improve the docs!
// TODO(nixon): Implement Clone/Copy/Eq/Ord/Hash (just like field_path::accessors).

pub type ValueTable<K> = TypeTable<ValueId<K>>;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct ValueId<K> {
    pub key: K,
    pub field: UntypedField,
}

impl<K> ValueId<K> {
    pub fn new(key: K, field: UntypedField) -> Self {
        Self { key, field }
    }
}

/// Signature for applying a setter.
/// It looks up a value in the [`ValueTable`] and applies it via [`Accessor`].
///
/// [`Accessor`]: field_path::accessor::Accessor
pub type SetFn<K, S> = fn(
    &mut S,
    &ValueId<K>,
    &FieldAccessorRegistry,
    &ValueTable<K>,
) -> bool;

pub struct Setter<K, S> {
    pub set_fn: SetFn<K, S>,
}

impl<K, S> Setter<K, S>
where
    K: Hash + Eq + 'static,
    S: 'static,
{
    pub const fn new<T: 'static + Clone>() -> Self {
        #[inline]
        fn apply_impl<
            K: Hash + Eq + 'static,
            S: 'static,
            T: Clone + 'static,
        >(
            source: &mut S,
            value_id: &ValueId<K>,
            registry: &FieldAccessorRegistry,
            table: &ValueTable<K>,
        ) -> bool {
            if let Ok(accessor) =
                registry.get::<S, T>(&value_id.field)
                && let Some(value) = table.get::<T>(value_id)
            {
                *accessor.get_mut(source) = value.clone();
                return true;
            }
            false
        }

        Self {
            set_fn: apply_impl::<K, S, T>,
        }
    }

    pub fn apply(
        &self,
        source: &mut S,
        value_id: &ValueId<K>,
        registry: &FieldAccessorRegistry,
        table: &ValueTable<K>,
    ) -> bool {
        (self.set_fn)(source, value_id, registry, table)
    }

    pub fn untyped(&self) -> UntypedSetter<K> {
        UntypedSetter {
            source_id: TypeId::of::<S>(),
            set_fn: self.set_fn as *const (),
            _marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct UntypedSetter<K> {
    source_id: TypeId,
    set_fn: *const (),
    _marker: PhantomData<K>,
}

impl<K> UntypedSetter<K> {
    pub fn typed<S: 'static>(&self) -> Option<Setter<K, S>> {
        if self.source_id == TypeId::of::<S>() {
            return Some(unsafe { self.typed_unchecked() });
        }
        None
    }

    // TODO(nixon): Write docs.
    /// ## Safety
    pub const unsafe fn typed_unchecked<S>(&self) -> Setter<K, S> {
        unsafe {
            Setter {
                set_fn: core::mem::transmute::<*const (), SetFn<K, S>>(
                    self.set_fn,
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use field_path::registry::FieldAccessorRegistry;

    #[derive(Default, Debug, PartialEq, Clone)]
    struct Frame {
        width: f32,
        opacity: f32,
    }

    #[derive(Default, Debug, PartialEq, Clone)]
    struct Label {
        font_size: f32,
    }

    #[test]
    fn untyped_setter_recovers_correct_type() {
        let setter = Setter::<u32, Frame>::new::<f32>();
        let untyped = setter.untyped();
        assert!(untyped.typed::<Frame>().is_some());
    }

    #[test]
    fn untyped_setter_returns_none_for_wrong_source_type() {
        let setter = Setter::<u32, Frame>::new::<f32>();
        let untyped = setter.untyped();
        assert!(untyped.typed::<Label>().is_none());
    }

    #[test]
    fn setter_writes_value_into_field() {
        let field_acc = field_path::field_accessor!(<Frame>::width);
        let untyped_field = field_acc.field.untyped();

        let mut registry = FieldAccessorRegistry::default();
        registry.register_field(field_acc);

        let mut values: ValueTable<u32> = TypeTable::new();
        let value_id = ValueId::new(1u32, untyped_field);
        values.insert(value_id, 42.0f32);

        let setter = Setter::<u32, Frame>::new::<f32>();
        let mut frame = Frame::default();
        let ok =
            setter.apply(&mut frame, &value_id, &registry, &values);

        assert!(ok);
        assert_eq!(frame.width, 42.0);
    }

    #[test]
    fn setter_is_no_op_when_value_is_absent() {
        let field_acc = field_path::field_accessor!(<Frame>::width);
        let untyped_field = field_acc.field.untyped();

        let mut registry = FieldAccessorRegistry::default();
        registry.register_field(field_acc);

        let values: ValueTable<u32> = TypeTable::new(); // empty
        let value_id = ValueId::new(1u32, untyped_field);

        let setter = Setter::<u32, Frame>::new::<f32>();
        let mut frame = Frame {
            width: 10.0,
            opacity: 1.0,
        };
        let ok =
            setter.apply(&mut frame, &value_id, &registry, &values);

        assert!(!ok);
        assert_eq!(frame.width, 10.0); // unchanged
    }

    #[test]
    fn setter_is_no_op_when_accessor_is_absent() {
        let field_acc = field_path::field_accessor!(<Frame>::width);
        let untyped_field = field_acc.field.untyped();

        // Registry has no registered accessor for Frame::width
        let registry = FieldAccessorRegistry::default();

        let mut values: ValueTable<u32> = TypeTable::new();
        let value_id = ValueId::new(1u32, untyped_field);
        values.insert(value_id, 99.0f32);

        let setter = Setter::<u32, Frame>::new::<f32>();
        let mut frame = Frame {
            width: 5.0,
            opacity: 1.0,
        };
        let ok =
            setter.apply(&mut frame, &value_id, &registry, &values);

        assert!(!ok);
        assert_eq!(frame.width, 5.0); // unchanged
    }
}
