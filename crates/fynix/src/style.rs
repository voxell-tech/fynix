use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;

use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use hashbrown::HashMap;

use crate::id::{GenId, IdGenerator};
use crate::init::Init;
use crate::typing::type_pool::{PoolKey, TypePool};

pub mod storage;

pub use field_path::field_accessor as path;
pub use storage::Styles;

// pub enum StyleCommand {
//     Set(StyleId, UntypedField),
//     Replace(StyleId, StyleId, UntypedField),
// }

/// An immutable, committed snapshot of field changes for one style
/// scope.
///
/// Each node links to its parent via `parent_id`, forming an
/// inheritance chain that [`Styles::apply`] walks to resolve
/// defaults.
///
/// A node can have up to two children, filled in commit order: the
/// first style of a deeper scope opened under it, or the next commit
/// in the same scope. When a style is removed, all its descendants
/// are also removed and its slot on the parent is freed.
pub struct Style {
    parent_id: Option<StyleId>,
    index_map: HashMap<TypeId, Span>,
    fields: Box<[(UntypedField, PoolKey)]>,
    children: [Option<StyleId>; 2],
}

impl Style {
    pub fn parent_id(&self) -> Option<StyleId> {
        self.parent_id
    }

    pub fn children(&self) -> [Option<&StyleId>; 2] {
        [self.children[0].as_ref(), self.children[1].as_ref()]
    }

    fn get_fields(
        &self,
        id: &TypeId,
    ) -> Option<&[(UntypedField, PoolKey)]> {
        let span = self.index_map.get(id)?;
        Some(&self.fields[span.start..span.end])
    }
}

struct StyleBuilder {
    field_map: HashMap<TypeId, HashMap<UntypedField, PoolKey>>,
}

impl StyleBuilder {
    fn new() -> Self {
        Self {
            field_map: HashMap::new(),
        }
    }

    /// Inserts or replaces the [`PoolKey`] for `field` under
    /// `type_id`. Returns the displaced key if one existed.
    fn insert(
        &mut self,
        type_id: TypeId,
        field: UntypedField,
        key: PoolKey,
    ) -> Option<PoolKey> {
        self.field_map
            .entry(type_id)
            .or_default()
            .insert(field, key)
    }

    fn clear(&mut self) {
        self.field_map.clear();
    }

    fn is_empty(&self) -> bool {
        self.field_map.is_empty()
    }

    /// Consumes the builder and produces a committed [`Style`].
    #[must_use]
    fn build(self, parent_id: Option<StyleId>) -> Style {
        let mut index_map = HashMap::new();
        let mut all_fields = Vec::new();

        for (id, fields) in self.field_map {
            if fields.is_empty() {
                continue;
            }

            let start = all_fields.len();
            all_fields.extend(fields);
            let end = all_fields.len();

            index_map.insert(id, Span::new(start, end));
        }

        Style {
            parent_id,
            index_map,
            fields: all_fields.into_boxed_slice(),
            children: [None; 2],
        }
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Half-open index range `[start, end)` into [`Style::fields`].
#[derive(Debug, Clone, Copy)]
struct Span {
    start: usize,
    end: usize,
}

impl Span {
    const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Monomorphized function signature for writing one typed value into
/// a source field.
///
/// Reads the value of type `T` from `values` at `key`, then
/// writes it into `source` via `accessor`. Returns `true` on success.
pub type SetStyleFn<S> =
    fn(&mut S, &UntypedAccessor, &PoolKey, &TypePool) -> bool;

/// Concrete implementation of [`SetStyleFn`] for the `(S, T)` pair.
#[inline]
pub fn set_style<S: Stylable, T: StyleValue>(
    source: &mut S,
    accessor: &UntypedAccessor,
    key: &PoolKey,
    values: &TypePool,
) -> bool {
    if let Some(accessor) = accessor.typed::<S, T>()
        && let Some(value) = values.get::<T>(key)
    {
        *accessor.get_mut(source) = value.clone();
        return true;
    }
    false
}

/// Typed wrapper around [`SetStyleFn<S>`].
///
/// Created once per `(S, T)` pair and stored type-erased as
/// [`UntypedSetStyle`] in the [`Styles`] registry.
pub struct SetStyle<S: Stylable> {
    set_fn: SetStyleFn<S>,
}

impl<S: Stylable> SetStyle<S> {
    /// Creates a `SetStyle` monomorphized for value type `T`.
    pub fn new<T: StyleValue>() -> Self {
        Self {
            set_fn: set_style::<S, T>,
        }
    }

    /// Erases the source type, storing the function pointer as a raw
    /// `*const ()` alongside the source [`TypeId`].
    pub fn untyped(&self) -> UntypedSetStyle {
        UntypedSetStyle {
            source_id: TypeId::of::<S>(),
            set_fn: self.set_fn as *const (),
        }
    }

    /// Applies the setter. Returns `true` if both the accessor and
    /// the value were found.
    pub fn apply(
        &self,
        source: &mut S,
        accessor: &UntypedAccessor,
        key: &PoolKey,
        values: &TypePool,
    ) -> bool {
        (self.set_fn)(source, accessor, key, values)
    }
}

/// Type-erased [`SetStyle<S>`], recoverable via
/// [`typed`](UntypedSetStyle::typed).
#[derive(Debug, Clone, Copy)]
pub struct UntypedSetStyle {
    source_id: TypeId,
    set_fn: *const (),
}

impl UntypedSetStyle {
    /// Recovers the typed [`SetStyle<S>`] if `S` matches the source
    /// type.
    pub fn typed<S: Stylable>(&self) -> Option<SetStyle<S>> {
        if TypeId::of::<S>() == self.source_id {
            return Some(unsafe { self.typed_unchecked() });
        }

        None
    }

    /// Recovers the typed [`SetStyle<S>`] without a type check.
    ///
    /// # Safety
    ///
    /// `S` must be the source type this setter was created for.
    pub const unsafe fn typed_unchecked<S: Stylable>(
        &self,
    ) -> SetStyle<S> {
        unsafe {
            use core::mem::transmute;
            SetStyle {
                set_fn: transmute::<*const (), SetStyleFn<S>>(
                    self.set_fn,
                ),
            }
        }
    }
}

/// Blanket trait alias for types whose fields can be targeted by
/// [`Styles::set()`].
///
/// Any `Init + 'static` type automatically implements this, allowing
/// both element types and non-element style structs to be used with
/// the style system.
pub trait Stylable: Init + 'static {}

impl<T: Init + 'static> Stylable for T {}

/// Blanket trait alias for values that can be stored as style
/// defaults.
///
/// Any `Clone + 'static` type automatically implements this.
pub trait StyleValue: Clone + 'static {}

impl<T: Clone + 'static> StyleValue for T {}

/// Generational ID for committed style nodes.
pub type StyleId = GenId<_StyleMarker>;
pub type StyleIdGenerator = IdGenerator<_StyleMarker>;

#[doc(hidden)]
pub struct _StyleMarker;
