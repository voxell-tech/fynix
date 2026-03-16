use core::any::TypeId;
use core::hash::Hash;
use core::marker::PhantomData;

use field_path::field::UntypedField;
use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;

use crate::style_map::ValueId;
use crate::type_map::TypeTable;

// TODO(nixon): Really improve the docs!
// TODO(nixon): Implement Clone/Copy/Eq/Ord/Hash (just like field_path::accessors).

#[derive(Debug)]
pub struct StyleRegistry<K> {
    pub styles: HashMap<UntypedField, UntypedStyle<K>>,
}

impl<K> StyleRegistry<K> {
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
        }
    }
}

impl<K> Default for StyleRegistry<K> {
    fn default() -> Self {
        Self::new()
    }
}

pub type ValueTable<K> = TypeTable<ValueId<K>>;

/// Signature for applying a style.
/// It looks up a value in the [`ValueTable`] and apply it via [`Accessor`].
///
/// [`Accessor`]: field_path::accessor::Accessor
pub type SetFn<K, S> =
    fn(&mut S, &ValueId<K>, &FieldAccessorRegistry, &ValueTable<K>);

pub struct Style<K, S>(SetFn<K, S>);

impl<K, S> Style<K, S>
where
    K: Hash + Eq + 'static,
    S: 'static,
{
    pub const fn new<T: 'static + Clone>() -> Self {
        Self(
            #[inline]
            |source, value_id, registry, table| {
                if let Ok(accessor) =
                    registry.get::<S, T>(&value_id.field)
                    && let Some(value) = table.get::<T>(value_id)
                {
                    *accessor.get_mut(source) = value.clone();
                }
            },
        )
    }

    pub fn apply(
        &self,
        source: &mut S,
        value_id: &ValueId<K>,
        registry: &FieldAccessorRegistry,
        table: &ValueTable<K>,
    ) {
        (self.0)(source, value_id, registry, table);
    }

    pub fn untyped(&self) -> UntypedStyle<K> {
        UntypedStyle {
            source_id: TypeId::of::<S>(),
            set_fn: self.0 as *const (),
            _marker: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct UntypedStyle<K> {
    source_id: TypeId,
    set_fn: *const (),
    _marker: PhantomData<K>,
}

impl<K> UntypedStyle<K> {
    pub fn typed<S: 'static>(&self) -> Option<Style<K, S>> {
        if self.source_id == TypeId::of::<S>() {
            return Some(unsafe { self.typed_unchecked() });
        }
        None
    }

    // TODO(nixon): Write docs.
    /// ## Safety
    pub const unsafe fn typed_unchecked<S>(&self) -> Style<K, S> {
        unsafe {
            Style(core::mem::transmute::<*const (), SetFn<K, S>>(
                self.set_fn,
            ))
        }
    }
}
