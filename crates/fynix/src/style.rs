use core::any::TypeId;

use alloc::boxed::Box;
use alloc::vec::Vec;
use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};

use crate::element::Element;
use crate::id::{GenId, IdGenerator};
use crate::type_table::TypeTable;

pub use field_path::field_accessor as path;

/// Central style manager.
///
/// Maintains the registry of field setters, the stored style values, and the
/// committed chain of [`Style`] nodes. The build context calls [`set`],
/// [`commit_styles`], and [`apply`] on this to propagate style defaults to
/// elements.
///
/// [`set`]: Styles::set
/// [`commit_styles`]: Styles::commit_styles
/// [`apply`]: Styles::apply
pub struct Styles {
    /// Maps each field to its accessor and type-erased setter, registered
    /// once per `(E, T)` pair on the first [`set`](Styles::set) call.
    registry:
        HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>,
    /// Stores the actual style values keyed by `(StyleId, T)`.
    pub style_values: TypeTable<StyleId>,
    /// Committed style nodes, each forming a singly-linked
    /// inheritance chain via their `parent_id`.
    pub styles: HashMap<StyleId, Style>,
    /// Accumulates field changes for the *current* (open) style
    /// node until the next [`commit_styles`](Styles::commit_styles)
    /// call.
    style_builder: StyleBuilder,
    /// The ID of the open style node currently being built.
    current_id: StyleId,
    id_generator: StyleIdGenerator,
}

impl Styles {
    pub fn new() -> Self {
        let mut id_generator = StyleIdGenerator::new();

        Self {
            registry: HashMap::new(),
            style_values: TypeTable::new(),
            styles: HashMap::new(),
            style_builder: StyleBuilder::new(),
            current_id: id_generator.new_id(),
            id_generator,
        }
    }

    /// Returns the ID of the currently open (uncommitted) style node.
    pub fn current_id(&self) -> StyleId {
        self.current_id
    }

    /// Returns `true` when there are pending field changes that need to be
    /// committed before the next element is created.
    pub fn should_commit(&self) -> bool {
        !self.style_builder.is_empty()
    }

    pub fn clear_builder(&mut self) {
        self.style_builder.clear();
    }

    /// Flushes pending field changes into a new committed [`Style`] node
    /// and advances to a fresh [`StyleId`].
    ///
    /// `parent_id` links the new node into the inheritance chain so that
    /// [`apply`](Styles::apply) can walk up to ancestor defaults.
    ///
    /// The parent's children is also updated accordingly, where the newly
    /// committed style is added as the parent's child
    pub fn commit_styles(
        &mut self,
        parent_id: Option<StyleId>,
        is_nested: bool,
    ) {
        let committed_id = self.current_id;
        let style =
            core::mem::take(&mut self.style_builder).build(parent_id);

        self.styles.insert(committed_id, style);

        if let Some(parent) = parent_id {
            self.add_child_to_style(parent, committed_id, is_nested);
        }

        self.current_id = self.id_generator.new_id();
    }

    /// Adds `child_id` to the children of `parent_id`.
    ///
    /// If `is_nested` is true, child is set as the `nested_child`
    /// (one scope deeper). Otherwise, it is set as the `adjacent_child`
    /// (same scope, next sibling node).
    fn add_child_to_style(
        &mut self,
        parent_id: StyleId,
        child_id: StyleId,
        is_nested: bool,
    ) {
        let Some(parent) = self.styles.get_mut(&parent_id) else {
            return;
        };

        if is_nested {
            debug_assert!(parent.nested_child.is_none());
            parent.nested_child = Some(child_id);
        } else {
            debug_assert!(parent.adjacent_child.is_none());
            parent.adjacent_child = Some(child_id);
        }
    }

    /// Queues a style default: field `field_accessor` on element type `E`
    /// will be set to `value` for all elements created under the current
    /// style scope.
    ///
    /// The setter is registered in the registry on the first call for a
    /// given field; subsequent calls only update the stored value.
    pub fn set<E: Element, T: StyleValue>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) {
        let untyped_field = field_accessor.field.untyped();
        let type_id = TypeId::of::<E>();

        if !self.registry.contains_key(&untyped_field) {
            self.registry.insert(
                untyped_field,
                (
                    field_accessor.accessor.untyped(),
                    SetStyle::<E>::new::<T>().untyped(),
                ),
            );
        }

        self.style_values.insert(self.current_id, value);
        self.style_builder.insert(type_id, untyped_field);
    }

    /// Recursively removes the style and their descendants.
    pub fn remove(&mut self, id: &StyleId) -> bool {
        let Some(style) = self.styles.remove(id) else {
            return false;
        };
        self.id_generator.recycle(*id);

        for c in style.children().into_iter().flatten() {
            self.remove(c);
        }

        true
    }

    /// Applies the style chain rooted at `id` to `element`.
    ///
    /// Walks the parent chain from leaf to root. The first value encountered
    /// for each field wins (leaf takes precedence over ancestors).
    pub fn apply<E: Element>(&self, element: &mut E, id: &StyleId) {
        let type_id = TypeId::of::<E>();
        let mut applied = HashSet::new();

        // Walk leaf-to-root; the first value seen for a field wins.
        let mut current = Some(*id);
        while let Some(id) = current {
            let Some(style) = self.styles.get(&id) else {
                break;
            };
            current = style.parent_id;

            let Some(fields) = style.get_fields(&type_id) else {
                continue;
            };

            for field in fields {
                if applied.contains(field) {
                    continue;
                }

                if let Some((accessor, untyped_set)) =
                    self.registry.get(field)
                    && let Some(set_style) = untyped_set.typed::<E>()
                {
                    set_style.apply(
                        element,
                        accessor,
                        &id,
                        &self.style_values,
                    );
                    applied.insert(*field);
                }
            }
        }
    }
}

pub enum StyleCommand {
    Set(StyleId, UntypedField),
    Replace(StyleId, StyleId, UntypedField),
}

impl Default for Styles {
    fn default() -> Self {
        Self::new()
    }
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

/// Mutable builder that accumulates pending field changes and
/// produces an immutable [`Style`] via [`build`](StyleBuilder::build).
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

/// Monomorphized function signature for writing one typed value into an
/// element field.
///
/// Reads the value of type `T` from `values` at `style_id`, then writes it
/// into `element` via `accessor`. Returns `true` on success.
pub type SetStyleFn<E> = fn(
    &mut E,
    &UntypedAccessor,
    &StyleId,
    &TypeTable<StyleId>,
) -> bool;

/// Concrete implementation of [`SetStyleFn`] for the `(E, T)` pair.
#[inline]
pub fn set_style<E: Element, T: StyleValue>(
    element: &mut E,
    accessor: &UntypedAccessor,
    style_id: &StyleId,
    values: &TypeTable<StyleId>,
) -> bool {
    if let Some(accessor) = accessor.typed::<E, T>()
        && let Some(value) = values.get::<T>(style_id)
    {
        *accessor.get_mut(element) = value.clone();
        return true;
    }
    false
}

/// Typed wrapper around [`SetStyleFn<E>`].
///
/// Created once per `(E, T)` pair and stored type-erased as
/// [`UntypedSetStyle`] in the [`Styles`] registry.
pub struct SetStyle<E: Element> {
    set_fn: SetStyleFn<E>,
}

impl<E: Element> SetStyle<E> {
    /// Creates a `SetStyle` monomorphized for value type `T`.
    pub fn new<T: StyleValue>() -> Self {
        Self {
            set_fn: set_style::<E, T>,
        }
    }

    /// Erases the element type, storing the function pointer as a raw
    /// `*const ()` alongside the source [`TypeId`].
    pub fn untyped(&self) -> UntypedSetStyle {
        UntypedSetStyle {
            source_id: TypeId::of::<E>(),
            set_fn: self.set_fn as *const (),
        }
    }

    /// Applies the setter. Returns `true` if both the accessor and the value
    /// were found.
    pub fn apply(
        &self,
        element: &mut E,
        accessor: &UntypedAccessor,
        style_id: &StyleId,
        values: &TypeTable<StyleId>,
    ) -> bool {
        (self.set_fn)(element, accessor, style_id, values)
    }
}

/// Type-erased [`SetStyle<E>`], recoverable via [`typed`](UntypedSetStyle::typed).
#[derive(Debug, Clone, Copy)]
pub struct UntypedSetStyle {
    source_id: TypeId,
    set_fn: *const (),
}

impl UntypedSetStyle {
    /// Recovers the typed [`SetStyle<E>`] if `E` matches the source type.
    pub fn typed<E: Element>(&self) -> Option<SetStyle<E>> {
        if TypeId::of::<E>() == self.source_id {
            return Some(unsafe { self.typed_unchecked() });
        }

        None
    }

    /// Recovers the typed [`SetStyle<E>`] without a type check.
    ///
    /// # Safety
    ///
    /// `E` must be the element type this setter was created for.
    pub const unsafe fn typed_unchecked<E: Element>(
        &self,
    ) -> SetStyle<E> {
        unsafe {
            use core::mem::transmute;
            SetStyle {
                set_fn: transmute::<*const (), SetStyleFn<E>>(
                    self.set_fn,
                ),
            }
        }
    }
}

/// Blanket trait alias for values that can be stored as style defaults.
///
/// Any `Clone + 'static` type automatically implements this.
pub trait StyleValue: Clone + 'static {}

impl<T: Clone + 'static> StyleValue for T {}

/// Generational ID for committed style nodes.
pub type StyleId = GenId<_StyleMarker>;
pub type StyleIdGenerator = IdGenerator<_StyleMarker>;

#[doc(hidden)]
pub struct _StyleMarker;
