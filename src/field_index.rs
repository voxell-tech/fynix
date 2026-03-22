use core::any::TypeId;

use alloc::boxed::Box;
use alloc::vec::Vec;
use field_path::field::UntypedField;
use hashbrown::{HashMap, HashSet};

pub struct FieldIndex {
    pub index_map: HashMap<TypeId, Span>,
    pub fields: Box<[UntypedField]>,
}

impl FieldIndex {
    pub fn get_fields(&self, id: &TypeId) -> Option<&[UntypedField]> {
        let span = self.index_map.get(id)?;
        Some(&self.fields[span.start..span.end])
    }
}

pub struct FieldIndexBuilder {
    field_map: HashMap<TypeId, HashSet<UntypedField>>,
}

impl FieldIndexBuilder {
    pub fn new() -> Self {
        Self {
            field_map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: TypeId, field: UntypedField) {
        self.field_map.entry(id).or_default().insert(field);
    }

    pub fn remove(&mut self, id: &TypeId, field: &UntypedField) {
        if let Some(fields) = self.field_map.get_mut(id) {
            fields.remove(field);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.field_map.is_empty()
    }

    pub fn compile(self) -> FieldIndex {
        let mut index_map = HashMap::new();
        let mut all_fields = Vec::new();

        for (id, fields) in self.field_map.into_iter() {
            if fields.is_empty() {
                continue;
            }

            let start = all_fields.len();
            all_fields.extend(fields);
            let end = all_fields.len();

            index_map.insert(id, Span::new(start, end));
        }

        FieldIndex {
            index_map,
            fields: all_fields.into_boxed_slice(),
        }
    }
}

impl Default for FieldIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::any::TypeId;

    // Dummy source types representing UI widget structs.
    struct Frame {
        width: f32,
        height: f32,
    }
    struct Label {
        font_size: f32,
    }

    const fn frame_width() -> UntypedField {
        field_path::field_accessor!(<Frame>::width).field.untyped()
    }

    const fn frame_height() -> UntypedField {
        field_path::field_accessor!(<Frame>::height).field.untyped()
    }

    const fn label_font_size() -> UntypedField {
        field_path::field_accessor!(<Label>::font_size)
            .field
            .untyped()
    }

    #[test]
    fn empty_builder_compiles_to_empty_index() {
        let index = FieldIndexBuilder::new().compile();
        assert!(index.index_map.is_empty());
        assert!(index.fields.is_empty());
    }

    #[test]
    fn single_key_single_field_span_is_length_one() {
        let mut builder = FieldIndexBuilder::new();
        let field = frame_width();
        builder.insert(TypeId::of::<Frame>(), field);
        let index = builder.compile();

        let span =
            index.index_map.get(&TypeId::of::<Frame>()).unwrap();
        assert_eq!(span.end - span.start, 1);
        assert_eq!(index.fields[span.start], field);
    }

    #[test]
    fn multiple_fields_for_same_key_are_contiguous() {
        let mut builder = FieldIndexBuilder::new();
        builder.insert(TypeId::of::<Frame>(), frame_width());
        builder.insert(TypeId::of::<Frame>(), frame_height());
        let index = builder.compile();

        let span =
            index.index_map.get(&TypeId::of::<Frame>()).unwrap();
        assert_eq!(span.end - span.start, 2);
    }

    #[test]
    fn multiple_keys_have_non_overlapping_spans() {
        let mut builder = FieldIndexBuilder::new();
        builder.insert(TypeId::of::<Frame>(), frame_width());
        builder.insert(TypeId::of::<Frame>(), frame_height());
        builder.insert(TypeId::of::<Label>(), label_font_size());
        let index = builder.compile();

        let span_frame =
            index.index_map.get(&TypeId::of::<Frame>()).unwrap();
        let span_label =
            index.index_map.get(&TypeId::of::<Label>()).unwrap();

        assert_eq!(span_frame.end - span_frame.start, 2);
        assert_eq!(span_label.end - span_label.start, 1);
        // Spans must not overlap.
        let no_overlap = span_frame.end <= span_label.start
            || span_label.end <= span_frame.start;
        assert!(no_overlap);
    }

    #[test]
    fn duplicate_field_for_same_key_is_deduplicated() {
        let mut builder = FieldIndexBuilder::new();
        builder.insert(TypeId::of::<Frame>(), frame_width());
        builder.insert(TypeId::of::<Frame>(), frame_width()); // duplicate
        let index = builder.compile();

        let span =
            index.index_map.get(&TypeId::of::<Frame>()).unwrap();
        assert_eq!(span.end - span.start, 1);
    }

    #[test]
    fn remove_field_before_compile_excludes_it() {
        let mut builder = FieldIndexBuilder::new();
        builder.insert(TypeId::of::<Frame>(), frame_width());
        builder.insert(TypeId::of::<Frame>(), frame_height());
        builder.remove(&TypeId::of::<Frame>(), &frame_height());
        let index = builder.compile();

        let span =
            index.index_map.get(&TypeId::of::<Frame>()).unwrap();
        assert_eq!(span.end - span.start, 1);
        assert_eq!(index.fields[span.start], frame_width());
    }

    #[test]
    fn remove_on_absent_key_does_not_panic() {
        let mut builder = FieldIndexBuilder::new();
        builder.remove(&TypeId::of::<Frame>(), &frame_width());
    }
}
