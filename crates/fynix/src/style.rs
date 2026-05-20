use core::any::TypeId;

use alloc::boxed::Box;
use alloc::vec::Vec;
use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use hashbrown::{HashMap, HashSet};

use crate::id::{GenId, IdGenerator};
use crate::init::Init;
use crate::type_table::TypeTable;

pub mod storage;

pub use field_path::field_accessor as path;
pub use storage::Styles;

pub enum StyleCommand {
    Set(StyleId, UntypedField),
    Replace(StyleId, StyleId, UntypedField),
}

/// An immutable, committed snapshot of field changes for one
/// style scope.
///
/// Each node links to its parent via `parent_id`, forming an inheritance
/// chain that [`Styles::apply`] walks to resolve defaults.
///
/// A node can have up to two children: `adjacent_child` (same scope,
/// next sibling) and `nested_child` (one scope deeper). When a style
/// is removed, all its descendants are also removed.
pub struct Style {
    parent_id: Option<StyleId>,
    index_map: HashMap<TypeId, Span>,
    fields: Box<[UntypedField]>,
    adjacent_child: Option<StyleId>,
    nested_child: Option<StyleId>,
}

impl Style {
    pub fn parent_id(&self) -> Option<StyleId> {
        self.parent_id
    }

    pub fn children(&self) -> [Option<&StyleId>; 2] {
        [self.adjacent_child.as_ref(), self.nested_child.as_ref()]
    }

    pub fn adjacent_child(&self) -> Option<&StyleId> {
        self.adjacent_child.as_ref()
    }

    pub fn nested_child(&self) -> Option<&StyleId> {
        self.nested_child.as_ref()
    }

    fn get_fields(&self, id: &TypeId) -> Option<&[UntypedField]> {
        let span = self.index_map.get(id)?;
        Some(&self.fields[span.start..span.end])
    }
}

struct StyleBuilder {
    field_map: HashMap<TypeId, HashSet<UntypedField>>,
}

impl StyleBuilder {
    fn new() -> Self {
        Self {
            field_map: HashMap::new(),
        }
    }

    fn insert(&mut self, id: TypeId, field: UntypedField) {
        self.field_map.entry(id).or_default().insert(field);
    }

    fn clear(&mut self) {
        self.field_map.clear();
    }

    fn is_empty(&self) -> bool {
        self.field_map.is_empty()
    }

    /// Consumes the builder and produces a committed [`Style`].
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
            adjacent_child: None,
            nested_child: None,
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

/// Monomorphized function signature for writing one typed value into a
/// source field.
///
/// Reads the value of type `T` from `values` at `key`, then writes it
/// into `source` via `accessor`. Returns `true` on success.
pub type SetStyleFn<S> = fn(
    &mut S,
    &UntypedAccessor,
    &StyleValueId,
    &TypeTable<StyleValueId>,
) -> bool;

/// Concrete implementation of [`SetStyleFn`] for the `(S, T)` pair.
#[inline]
pub fn set_style<S: 'static, T: StyleValue>(
    source: &mut S,
    accessor: &UntypedAccessor,
    key: &StyleValueId,
    values: &TypeTable<StyleValueId>,
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
pub struct SetStyle<S: 'static> {
    set_fn: SetStyleFn<S>,
}

impl<S> SetStyle<S> {
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

    /// Applies the setter. Returns `true` if both the accessor and the
    /// value were found.
    pub fn apply(
        &self,
        source: &mut S,
        accessor: &UntypedAccessor,
        key: &StyleValueId,
        values: &TypeTable<StyleValueId>,
    ) -> bool {
        (self.set_fn)(source, accessor, key, values)
    }
}

/// Type-erased [`SetStyle<S>`], recoverable via [`typed`](UntypedSetStyle::typed).
#[derive(Debug, Clone, Copy)]
pub struct UntypedSetStyle {
    source_id: TypeId,
    set_fn: *const (),
}

impl UntypedSetStyle {
    /// Recovers the typed [`SetStyle<S>`] if `S` matches the source type.
    pub fn typed<S>(&self) -> Option<SetStyle<S>> {
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
    pub const unsafe fn typed_unchecked<S>(&self) -> SetStyle<S> {
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

/// Composite key into the style value table: identifies one specific
/// field within one committed style node.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleValueId {
    pub id: StyleId,
    pub field: UntypedField,
}

impl StyleValueId {
    pub fn new(id: StyleId, field: UntypedField) -> Self {
        Self { id, field }
    }
}

/// Generational ID for committed style nodes.
pub type StyleId = GenId<_StyleMarker>;
pub type StyleIdGenerator = IdGenerator<_StyleMarker>;

#[doc(hidden)]
pub struct _StyleMarker;
