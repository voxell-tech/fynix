//! The edits a rich field makes, over parley's selection and cursor,
//! and the keys that ask for them.

use core::ops::Range;

use bevy::clipboard::Clipboard;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::{Rect, Vec2};
use parley::editing::{Cursor, Selection};
use parley::layout::Affinity;
use parley::{Brush, Layout};

use super::RichField;
use super::content::{Caret, MARKER, without_markers};
use crate::views::Atom;

/// What the caret can be moved by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    LineStart,
    LineEnd,
    HardLineStart,
    HardLineEnd,
    TextStart,
    TextEnd,
}

/// What a deletion removes when nothing is selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Erase {
    Back,
    Forward,
    BackWord,
    ForwardWord,
}

#[derive(Clone, Debug)]
pub(super) enum Edit {
    /// Moves the caret, or the selection's end when `true`.
    Move(Motion, bool),
    SelectAll,
    /// Drops the selection, leaving the caret where it ended.
    Collapse,
    Insert(String),
    InsertAtom(Atom),
    /// Takes out the atoms with this key.
    RemoveAtom(String),
    /// Puts the caret there, dropping the selection.
    SetCaret(Caret),
    Erase(Erase),
    Copy,
    Cut,
    Paste,
    /// A press at a point of the text, `taps` presses in a row.
    Press {
        at: Vec2,
        taps: usize,
        extend: bool,
    },
    /// The pointer moved, with the button down, to a point of the
    /// text.
    Drag(Vec2),
}

/// What replaces a range of the text.
enum With {
    Text(String),
    Atom(Atom),
}

impl RichField {
    /// Makes `edit` on the layout of the text, whose atoms' boxes are
    /// `boxes`, and says whether the next edit has to wait: when the
    /// content changed, which the layout does not show yet, or the
    /// clipboard has yet to give what is pasted.
    pub(super) fn apply<B: Brush>(
        &mut self,
        edit: Edit,
        layout: &Layout<B>,
        boxes: &[Rect],
        clipboard: Option<&mut Clipboard>,
    ) -> bool {
        let selection = self.selection(layout);
        let len = self.content.len();
        let range = selection.text_range();
        let range = range.start.min(len)..range.end.min(len);
        let focus = selection.focus();
        let is_cut = matches!(edit, Edit::Cut);
        let moved = match edit {
            Edit::Move(motion, extend) => {
                motion.apply(selection, layout, extend)
            }
            Edit::SelectAll => Selection::new(
                Cursor::from_byte_index(
                    layout,
                    0,
                    Affinity::Downstream,
                ),
                Cursor::from_byte_index(
                    layout,
                    len,
                    Affinity::Downstream,
                ),
            ),
            Edit::Collapse => selection.collapse(),
            Edit::Press { at, taps, extend } => {
                match (taps, self.beside(at, boxes, layout)) {
                    (1, Some((cursor, _))) if extend => {
                        selection.extend(cursor)
                    }
                    (1, Some((cursor, _))) => cursor.into(),
                    (1, None) if extend => selection
                        .shift_click_extension(layout, at.x, at.y),
                    (1, None) => {
                        Selection::from_point(layout, at.x, at.y)
                    }
                    (2, Some((_, marker))) => {
                        marker_selection(layout, marker)
                    }
                    (2, None) => {
                        Selection::word_from_point(layout, at.x, at.y)
                    }
                    _ => select_all(layout, len),
                }
            }
            Edit::Drag(at) => match self.beside(at, boxes, layout) {
                Some((cursor, _)) => selection.extend(cursor),
                None => selection.extend_to_point(layout, at.x, at.y),
            },
            Edit::Insert(text) => {
                return !text.is_empty()
                    && self.replace(range, With::Text(text));
            }
            Edit::InsertAtom(atom) => {
                return self.replace(range, With::Atom(atom));
            }
            Edit::RemoveAtom(key) => {
                let gone = self.content.remove_atoms(&key);
                if gone.is_empty() {
                    return false;
                }
                // The caret stays by what it was by, and the
                // selection is dropped.
                let focus = focus.index().min(len);
                let shift = gone
                    .iter()
                    .filter(|marker| **marker < focus)
                    .count();
                self.at = Some(focus - shift * MARKER.len_utf8());
                self.version += 1;
                return true;
            }
            Edit::SetCaret(caret) => {
                match self.content.index_of(&caret) {
                    Some(index) => Selection::from_byte_index(
                        layout,
                        index,
                        Affinity::Downstream,
                    ),
                    None => selection,
                }
            }
            Edit::Copy | Edit::Cut => {
                let copied = !range.is_empty()
                    && clipboard.is_some_and(|clipboard| {
                        let copy =
                            self.content.copy_text(range.clone());
                        clipboard.set_text(copy).is_ok()
                    });
                return copied
                    && is_cut
                    && self
                        .replace(range, With::Text(String::new()));
            }
            Edit::Paste => {
                let Some(clipboard) = clipboard else {
                    return false;
                };
                let mut read = clipboard.fetch_text();
                return match read.poll_result() {
                    Some(Ok(text)) => {
                        let text = pasted(&text);
                        !text.is_empty()
                            && self.replace(range, With::Text(text))
                    }
                    Some(Err(_)) => false,
                    // Read later, before the edits after it.
                    None => {
                        self.paste = Some(read);
                        true
                    }
                };
            }
            Edit::Erase(erase) => {
                let span = if selection.is_collapsed() {
                    self.erased(erase, focus, layout)
                } else {
                    range
                };
                let span = span.start.min(len)..span.end.min(len);
                return !span.is_empty()
                    && self.replace(span, With::Text(String::new()));
            }
        };
        self.selection = moved;
        self.at = None;
        false
    }

    /// The selection of the layout, or the caret last asked for.
    pub(super) fn selection<B: Brush>(
        &self,
        layout: &Layout<B>,
    ) -> Selection {
        match self.at {
            Some(at) => Selection::from_byte_index(
                layout,
                at.min(self.content.len()),
                Affinity::Downstream,
            ),
            None => self.selection.refresh(layout),
        }
    }

    /// For a point `at` inside the box of an atom, the cursor on the
    /// side of it that `at` is nearer, and the atom's marker. The
    /// layout puts the caret by the cluster, and a box has none of
    /// its own. The boxes are in the order of the atoms.
    fn beside<B: Brush>(
        &self,
        at: Vec2,
        boxes: &[Rect],
        layout: &Layout<B>,
    ) -> Option<(Cursor, usize)> {
        let held = boxes.iter().position(|rect| rect.contains(at))?;
        let (marker, _) = self.content.atoms().get(held)?;
        let index = if at.x < boxes[held].center().x {
            *marker
        } else {
            marker + MARKER.len_utf8()
        };
        let cursor = Cursor::from_byte_index(
            layout,
            index,
            Affinity::Downstream,
        );
        Some((cursor, *marker))
    }

    fn erased<B: Brush>(
        &self,
        erase: Erase,
        focus: Cursor,
        layout: &Layout<B>,
    ) -> Range<usize> {
        let at = focus.index();
        let clusters = focus.logical_clusters(layout);
        match erase {
            Erase::Back => clusters[0]
                .as_ref()
                .map(|cluster| {
                    let range = cluster.text_range();
                    // The last character, but a whole line break or
                    // emoji.
                    let start = self.content.text()[..range.end]
                        .char_indices()
                        .next_back()
                        .map_or(range.start, |(start, _)| start);
                    if cluster.is_hard_line_break()
                        || cluster.is_emoji()
                    {
                        range
                    } else {
                        start..range.end
                    }
                })
                .unwrap_or(at..at),
            Erase::Forward => clusters[1]
                .as_ref()
                .map_or(at..at, |cluster| cluster.text_range()),
            Erase::BackWord => {
                focus.previous_logical_word(layout).index()..at
            }
            Erase::ForwardWord => {
                at..focus.next_logical_word(layout).index()
            }
        }
    }

    /// Puts `with` in place of `range`, and the caret after it.
    fn replace(&mut self, range: Range<usize>, with: With) -> bool {
        let start = range.start;
        let caret = match with {
            With::Text(text) => {
                let text = without_markers(&text);
                self.content.splice(range, &text);
                start + text.len()
            }
            With::Atom(atom) => {
                self.content.splice_atom(range, atom);
                start + MARKER.len_utf8()
            }
        };
        self.at = Some(caret);
        self.version += 1;
        true
    }
}

fn select_all<B: Brush>(layout: &Layout<B>, len: usize) -> Selection {
    Selection::new(
        Cursor::from_byte_index(layout, 0, Affinity::Downstream),
        Cursor::from_byte_index(layout, len, Affinity::Downstream),
    )
}

/// The selection of the marker at `marker`, so of its atom.
fn marker_selection<B: Brush>(
    layout: &Layout<B>,
    marker: usize,
) -> Selection {
    Selection::new(
        Cursor::from_byte_index(layout, marker, Affinity::Downstream),
        Cursor::from_byte_index(
            layout,
            marker + MARKER.len_utf8(),
            Affinity::Downstream,
        ),
    )
}

impl Motion {
    fn apply<B: Brush>(
        self,
        selection: Selection,
        layout: &Layout<B>,
        extend: bool,
    ) -> Selection {
        match self {
            Self::Left => selection.previous_visual(layout, extend),
            Self::Right => selection.next_visual(layout, extend),
            Self::WordLeft => {
                selection.previous_visual_word(layout, extend)
            }
            Self::WordRight => {
                selection.next_visual_word(layout, extend)
            }
            Self::Up => selection.previous_line(layout, extend),
            Self::Down => selection.next_line(layout, extend),
            Self::LineStart => selection.line_start(layout, extend),
            Self::LineEnd => selection.line_end(layout, extend),
            Self::HardLineStart => {
                selection.hard_line_start(layout, extend)
            }
            Self::HardLineEnd => {
                selection.hard_line_end(layout, extend)
            }
            Self::TextStart => {
                selection.move_lines(layout, isize::MIN, extend)
            }
            Self::TextEnd => {
                selection.move_lines(layout, isize::MAX, extend)
            }
        }
    }
}

/// `text` from the clipboard, with its line breaks as `\n`.
pub(super) fn pasted(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

const NONE: u8 = 0;
const SUPER: u8 = 1;
const CTRL: u8 = 2;
const ALT: u8 = 4;
const SHIFT: u8 = 8;

/// Which of the modifiers are down, for [`edit_for`].
pub(super) fn modifiers(mut is_down: impl FnMut(Key) -> bool) -> u8 {
    [
        (SUPER, Key::Super),
        (CTRL, Key::Control),
        (ALT, Key::Alt),
        (SHIFT, Key::Shift),
    ]
    .into_iter()
    .filter(|(_, key)| is_down(key.clone()))
    .fold(NONE, |held, (flag, _)| held | flag)
}

/// Whether `input` is the letter of a shortcut: by what it types, or
/// by its place on the keyboard when that is not a Latin letter.
fn is_shortcut(
    input: &KeyboardInput,
    letter: &str,
    code: KeyCode,
) -> bool {
    match &input.logical_key {
        Key::Character(typed) if typed.is_ascii() => {
            typed.eq_ignore_ascii_case(letter)
        }
        Key::Character(_) => input.key_code == code,
        _ => false,
    }
}

/// The edit a key press asks for, with the modifiers `held` as
/// [`modifiers`] gives them: the keys of Bevy's `EditableText`. Enter
/// and Escape are the field's own.
pub(super) fn edit_for(
    input: &KeyboardInput,
    held: u8,
    mac: bool,
) -> Option<Edit> {
    let shift = held & SHIFT != 0;
    let command = if mac { SUPER } else { CTRL };
    let word = if mac { ALT } else { CTRL };
    let with_shift = |modifier: u8| [modifier, modifier | SHIFT];
    let move_by = |motion| Some(Edit::Move(motion, shift));
    let shortcut = |letter, code| {
        held == command && is_shortcut(input, letter, code)
    };
    match &input.logical_key {
        Key::Copy if held == NONE => Some(Edit::Copy),
        Key::Cut if held == NONE => Some(Edit::Cut),
        Key::Paste if held == NONE => Some(Edit::Paste),
        Key::Character(_) if shortcut("a", KeyCode::KeyA) => {
            Some(Edit::SelectAll)
        }
        Key::Character(_) if shortcut("c", KeyCode::KeyC) => {
            Some(Edit::Copy)
        }
        Key::Character(_) if shortcut("x", KeyCode::KeyX) => {
            Some(Edit::Cut)
        }
        Key::Character(_) if shortcut("v", KeyCode::KeyV) => {
            Some(Edit::Paste)
        }
        Key::Delete if held == SHIFT && !mac => Some(Edit::Cut),
        Key::Backspace if held == word => {
            Some(Edit::Erase(Erase::BackWord))
        }
        Key::Delete if held == word => {
            Some(Edit::Erase(Erase::ForwardWord))
        }
        Key::Backspace if with_shift(NONE).contains(&held) => {
            Some(Edit::Erase(Erase::Back))
        }
        Key::Delete if held == NONE => {
            Some(Edit::Erase(Erase::Forward))
        }
        Key::ArrowLeft
            if mac && with_shift(SUPER).contains(&held) =>
        {
            move_by(Motion::HardLineStart)
        }
        Key::ArrowRight
            if mac && with_shift(SUPER).contains(&held) =>
        {
            move_by(Motion::HardLineEnd)
        }
        Key::Home if !mac && with_shift(ALT).contains(&held) => {
            move_by(Motion::HardLineStart)
        }
        Key::End if !mac && with_shift(ALT).contains(&held) => {
            move_by(Motion::HardLineEnd)
        }
        Key::ArrowLeft if with_shift(word).contains(&held) => {
            move_by(Motion::WordLeft)
        }
        Key::ArrowRight if with_shift(word).contains(&held) => {
            move_by(Motion::WordRight)
        }
        Key::ArrowLeft if with_shift(NONE).contains(&held) => {
            move_by(Motion::Left)
        }
        Key::ArrowRight if with_shift(NONE).contains(&held) => {
            move_by(Motion::Right)
        }
        Key::ArrowUp
            if mac && with_shift(command).contains(&held) =>
        {
            move_by(Motion::TextStart)
        }
        Key::ArrowDown
            if mac && with_shift(command).contains(&held) =>
        {
            move_by(Motion::TextEnd)
        }
        Key::ArrowUp if with_shift(NONE).contains(&held) => {
            move_by(Motion::Up)
        }
        Key::ArrowDown if with_shift(NONE).contains(&held) => {
            move_by(Motion::Down)
        }
        Key::Home if with_shift(command).contains(&held) => {
            move_by(Motion::TextStart)
        }
        Key::End if with_shift(command).contains(&held) => {
            move_by(Motion::TextEnd)
        }
        Key::Home if with_shift(NONE).contains(&held) => {
            move_by(Motion::LineStart)
        }
        Key::End if with_shift(NONE).contains(&held) => {
            move_by(Motion::LineEnd)
        }
        Key::Character(_) | Key::Unidentified(_) | Key::Space
            if with_shift(NONE).contains(&held) =>
        {
            input
                .text
                .as_ref()
                .map(|typed| {
                    typed
                        .chars()
                        .filter(|typed| !typed.is_control())
                        .collect::<String>()
                })
                .filter(|typed| !typed.is_empty())
                .map(Edit::Insert)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use bevy::ecs::entity::Entity;
    use parley::fontique::Blob;
    use parley::{
        FontContext, FontFamilyName, InlineBox, InlineBoxKind,
        LayoutContext, StyleProperty,
    };

    use super::super::content::Piece;
    use super::*;
    use crate::tests::Plain as Theme;

    const INTER: &[u8] =
        include_bytes!("../../../../../assets/fonts/Inter.ttf");
    const CHIP: f32 = 40.0;

    fn chip(key: &str) -> Atom {
        Atom::new::<Theme>(key, format!("<{key}>"), || {
            unreachable!("not built")
        })
    }

    /// `ab`, a chip, `cd`: the chip's marker is at 2, and the text
    /// after it at 5.
    fn field() -> RichField {
        RichField::new(
            Entity::PLACEHOLDER,
            [
                Piece::Text("ab".into()),
                Piece::Atom(chip("a")),
                Piece::Text("cd".into()),
            ],
        )
    }

    /// The layout the field's spans make: a box after each marker,
    /// and a last marker.
    fn layout_of(field: &RichField) -> Layout<()> {
        let mut fonts = FontContext::new();
        let registered = fonts
            .collection
            .register_fonts(Blob::from(INTER.to_vec()), None);
        let family = fonts
            .collection
            .family_name(registered[0].0)
            .expect("a family")
            .to_owned();
        let text = field.content.text().to_owned();
        let mut contexts = LayoutContext::<()>::new();
        let mut builder =
            contexts.ranged_builder(&mut fonts, &text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(
            FontFamilyName::Named(Cow::Owned(family)).into(),
        ));
        builder.push_default(StyleProperty::FontSize(16.0));
        for (marker, _) in field.content.atoms() {
            builder.push_inline_box(InlineBox {
                id: *marker as u64,
                kind: InlineBoxKind::InFlow,
                index: *marker,
                width: CHIP,
                height: 20.0,
            });
        }
        let mut layout = builder.build(&text);
        layout.break_all_lines(Some(400.0));
        layout
    }

    fn edit(field: &mut RichField, edit: Edit) -> bool {
        let layout = layout_of(field);
        field.apply(edit, &layout, &[], None)
    }

    fn caret_at(field: &mut RichField, at: usize) {
        field.at = Some(at);
    }

    fn caret(field: &RichField) -> usize {
        field.selection(&layout_of(field)).focus().index()
    }

    /// The text of the field with each atom as its copy.
    fn text(field: &RichField) -> String {
        field.content.copy_text(0..field.content.len())
    }

    #[test]
    fn the_caret_is_one_box_wide_from_one_side_of_an_atom_to_the_other()
     {
        let field = field();
        let layout = layout_of(&field);
        let x = |at| {
            Cursor::from_byte_index(&layout, at, Affinity::Downstream)
                .geometry(&layout, 1.0)
                .x0 as f32
        };

        assert!((x(5) - x(2) - CHIP).abs() < 0.01);
    }

    /// The box of the chip, as the text node reports it.
    fn chip_box(field: &RichField) -> Rect {
        let layout = layout_of(field);
        let side = |at| {
            Cursor::from_byte_index(&layout, at, Affinity::Downstream)
                .geometry(&layout, 1.0)
                .x0 as f32
        };
        Rect::new(side(2), 0.0, side(5), 20.0)
    }

    fn press(field: &mut RichField, at: Vec2, taps: usize) {
        let boxes = [chip_box(field)];
        let layout = layout_of(field);
        let edit = Edit::Press {
            at,
            taps,
            extend: false,
        };
        field.apply(edit, &layout, &boxes, None);
    }

    #[test]
    fn a_press_on_an_atom_puts_the_caret_beside_it_on_the_nearer_side()
     {
        let mut field = field();
        let chip = chip_box(&field);
        let y = 10.0;

        press(&mut field, Vec2::new(chip.min.x + 5.0, y), 1);
        assert_eq!(caret(&field), 2);
        press(&mut field, Vec2::new(chip.max.x - 5.0, y), 1);
        assert_eq!(caret(&field), 5);
    }

    #[test]
    fn a_double_press_on_an_atom_selects_it() {
        let mut field = field();
        let chip = chip_box(&field);

        press(&mut field, chip.center(), 2);

        let selected =
            field.selection(&layout_of(&field)).text_range();
        assert_eq!(selected, 2..5);
    }

    #[test]
    fn a_selection_over_an_atom_is_as_wide_as_its_box() {
        let layout = layout_of(&field());
        let width = |from, to| {
            let selection = Selection::new(
                Cursor::from_byte_index(
                    &layout,
                    from,
                    Affinity::Downstream,
                ),
                Cursor::from_byte_index(
                    &layout,
                    to,
                    Affinity::Downstream,
                ),
            );
            selection
                .geometry(&layout)
                .iter()
                .map(|(bounds, _)| bounds.x1 - bounds.x0)
                .sum::<f64>() as f32
        };
        let text_only = width(0, 2);
        assert!((width(0, 5) - text_only - CHIP).abs() < 0.01);
    }

    #[test]
    fn the_caret_steps_over_an_atom_in_one_key() {
        let mut field = field();
        caret_at(&mut field, 2);

        edit(&mut field, Edit::Move(Motion::Right, false));
        assert_eq!(caret(&field), 5);
        edit(&mut field, Edit::Move(Motion::Left, false));
        assert_eq!(caret(&field), 2);
    }

    #[test]
    fn backspace_after_an_atom_removes_it_whole() {
        let mut field = field();
        caret_at(&mut field, 5);

        assert!(edit(&mut field, Edit::Erase(Erase::Back)));

        assert_eq!(text(&field), "abcd");
        assert!(field.content.atoms().is_empty());
        assert_eq!(field.at, Some(2));
    }

    #[test]
    fn delete_before_an_atom_removes_it_whole() {
        let mut field = field();
        caret_at(&mut field, 2);

        assert!(edit(&mut field, Edit::Erase(Erase::Forward)));

        assert_eq!(text(&field), "abcd");
        assert_eq!(field.at, Some(2));
    }

    #[test]
    fn delete_at_the_very_end_removes_nothing() {
        let mut field = field();
        let end = field.content.len();
        caret_at(&mut field, end);

        assert!(!edit(&mut field, Edit::Erase(Erase::Forward)));
        assert_eq!(text(&field), "ab<a>cd");
    }

    #[test]
    fn removing_an_atom_keeps_the_caret_by_what_it_was_by() {
        let mut field = field();
        let end = field.content.len();
        caret_at(&mut field, end);

        assert!(edit(&mut field, Edit::RemoveAtom("a".into())));

        assert_eq!(text(&field), "abcd");
        assert!(field.content.atoms().is_empty());
        assert_eq!(field.at, Some(4));
        assert!(!edit(&mut field, Edit::RemoveAtom("a".into())));

        let mut field = self::field();
        caret_at(&mut field, 1);
        edit(&mut field, Edit::RemoveAtom("a".into()));
        assert_eq!(field.at, Some(1));
    }

    #[test]
    fn the_caret_is_put_beside_an_atom_or_at_a_byte_of_the_plain_text()
     {
        let mut field = field();
        let mut put = |at| {
            edit(&mut field, Edit::SetCaret(at));
            caret(&field)
        };

        assert_eq!(put(Caret::After("a".into())), 5);
        assert_eq!(put(Caret::Before("a".into())), 2);
        assert_eq!(put(Caret::Offset(3)), 6);
        assert_eq!(put(Caret::After("none".into())), 6);
    }

    #[test]
    fn typing_on_both_sides_of_an_atom_keeps_it_between() {
        let mut field = field();
        caret_at(&mut field, 2);
        edit(&mut field, Edit::Insert("x".into()));
        caret_at(&mut field, 6);
        edit(&mut field, Edit::Insert("y".into()));

        assert_eq!(text(&field), "abx<a>ycd");
        assert_eq!(field.at, Some(7));
    }

    #[test]
    fn a_selection_over_an_atom_is_replaced_whole() {
        let mut field = field();
        edit(&mut field, Edit::SelectAll);

        edit(&mut field, Edit::Insert("z".into()));

        assert_eq!(text(&field), "z");
        assert!(field.content.atoms().is_empty());
    }

    #[test]
    fn the_layout_of_an_empty_text_or_one_ending_in_a_new_line_is_current()
     {
        for text in ["", "ab\n", "ab\n\n", "ab"] {
            let field = RichField::new(
                Entity::PLACEHOLDER,
                [Piece::Text(text.into())],
            );
            let layout = layout_of(&field);

            assert!(!field.is_stale(&layout), "{text:?}");
        }
        let layout = layout_of(&RichField::new(
            Entity::PLACEHOLDER,
            [Piece::Text("abc".into())],
        ));
        let shorter = RichField::new(
            Entity::PLACEHOLDER,
            [Piece::Text("ab".into())],
        );
        assert!(shorter.is_stale(&layout));
    }

    #[test]
    fn the_caret_cannot_go_past_the_content() {
        let mut field = field();
        caret_at(&mut field, 5);

        edit(&mut field, Edit::Move(Motion::TextEnd, false));
        assert_eq!(caret(&field), field.content.len());
        edit(&mut field, Edit::Move(Motion::Right, false));
        assert_eq!(caret(&field), field.content.len());
    }
}
