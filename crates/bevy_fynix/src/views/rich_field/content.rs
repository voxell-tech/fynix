//! What a rich field holds: text, with atoms between it.

use core::ops::Range;
use std::borrow::Cow;

use crate::views::{Atom, LineColor, Span};

/// What stands for an atom in the text that is laid out. It is a
/// character of its own, so the caret steps over it as over any
/// other, and it has no width: shaping hides it. The atom's inline
/// box is before it, so the caret after it is after the box.
pub(super) const MARKER: char = '\u{200B}';

/// A piece of what a rich field holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Atom(Atom),
}

/// The text of a rich field with the atoms taken out, and where each
/// one stood in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plain {
    pub text: String,
    pub atoms: Vec<PlainAtom>,
}

/// An atom's place in [`Plain::text`].
#[derive(Clone, Debug, PartialEq)]
pub struct PlainAtom {
    pub key: String,
    /// The byte of [`Plain::text`] the atom stood before. Atoms side
    /// by side share it.
    pub at: usize,
}

/// Where the caret of a rich field can be put.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caret {
    Start,
    End,
    /// Before the atom with this key.
    Before(String),
    /// After the atom with this key.
    After(String),
    /// At a byte of [`Plain::text`], before the atoms that stood
    /// there. A byte past the end is the end, and one inside a
    /// character is the start of it.
    Offset(usize),
}

/// `text` without the characters that stand for atoms, so none can be
/// typed or pasted.
pub(super) fn without_markers(text: &str) -> Cow<'_, str> {
    if text.contains(MARKER) {
        text.replace(MARKER, "").into()
    } else {
        text.into()
    }
}

/// The pieces of a field, as one text with a [`MARKER`] where each
/// atom stands. Every index is a byte of that text.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Content {
    text: String,
    /// The index of each atom's marker, in order.
    atoms: Vec<(usize, Atom)>,
}

impl Content {
    pub fn new(pieces: impl IntoIterator<Item = Piece>) -> Self {
        let mut content = Self::default();
        for piece in pieces {
            match piece {
                Piece::Text(text) => {
                    content.text.push_str(&without_markers(&text));
                }
                Piece::Atom(atom) => {
                    content.atoms.push((content.text.len(), atom));
                    content.text.push(MARKER);
                }
            }
        }
        content
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn atoms(&self) -> &[(usize, Atom)] {
        &self.atoms
    }

    /// Replaces `range` with `text`. Atoms inside it are gone.
    pub fn splice(&mut self, range: Range<usize>, text: &str) {
        self.put(range, text, None);
    }

    /// Replaces `range` with `atom`.
    pub fn splice_atom(&mut self, range: Range<usize>, atom: Atom) {
        self.put(range, MARKER.encode_utf8(&mut [0; 4]), Some(atom));
    }

    fn put(
        &mut self,
        range: Range<usize>,
        text: &str,
        atom: Option<Atom>,
    ) {
        let Range { start, end } = range;
        self.text.replace_range(range, text);
        self.atoms.retain(|(at, _)| !(start..end).contains(at));
        for (at, _) in &mut self.atoms {
            if *at >= end {
                *at = *at - end + start + text.len();
            }
        }
        if let Some(atom) = atom {
            let place =
                self.atoms.partition_point(|(at, _)| *at < start);
            self.atoms.insert(place, (start, atom));
        }
    }

    /// The pieces in order, with no empty text and no two texts side
    /// by side.
    pub fn pieces(&self) -> Vec<Piece> {
        let mut pieces = Vec::new();
        let mut from = 0;
        for (at, atom) in &self.atoms {
            if *at > from {
                pieces.push(Piece::Text(self.text[from..*at].into()));
            }
            pieces.push(Piece::Atom(atom.clone()));
            from = at + MARKER.len_utf8();
        }
        if from < self.text.len() {
            pieces.push(Piece::Text(self.text[from..].into()));
        }
        pieces
    }

    /// Removes every atom with `key`, and says where each one's
    /// marker was.
    pub fn remove_atoms(&mut self, key: &str) -> Vec<usize> {
        let gone = self
            .atoms
            .iter()
            .filter(|(_, atom)| atom.key() == key)
            .map(|(at, _)| *at)
            .collect::<Vec<usize>>();
        // Last first, so the markers still to go stay where they
        // were.
        for at in gone.iter().rev() {
            self.put(*at..at + MARKER.len_utf8(), "", None);
        }
        gone
    }

    /// The index of `caret` in the text with the markers, or `None`
    /// for the side of an atom there is none of.
    pub fn index_of(&self, caret: &Caret) -> Option<usize> {
        let marker = |key: &str| {
            self.atoms
                .iter()
                .find(|(_, atom)| atom.key() == key)
                .map(|(at, _)| *at)
        };
        match caret {
            Caret::Start => Some(0),
            Caret::End => Some(self.text.len()),
            Caret::Before(key) => marker(key),
            Caret::After(key) => {
                marker(key).map(|at| at + MARKER.len_utf8())
            }
            Caret::Offset(offset) => {
                let plain = self.plain(0..self.text.len()).text;
                let mut offset = (*offset).min(plain.len());
                while !plain.is_char_boundary(offset) {
                    offset -= 1;
                }
                // Each atom before the byte adds a marker to the
                // text with them.
                let before = self
                    .atoms
                    .iter()
                    .enumerate()
                    .filter(|(place, (at, _))| {
                        at - place * MARKER.len_utf8() < offset
                    })
                    .count();
                Some(offset + before * MARKER.len_utf8())
            }
        }
    }

    /// `range` as text without its atoms, and where each stood.
    pub fn plain(&self, range: Range<usize>) -> Plain {
        self.flatten(range, |plain, atom| {
            plain.atoms.push(PlainAtom {
                key: atom.key().into(),
                at: plain.text.len(),
            });
        })
    }

    /// `range` as text to copy, with the copy of each atom in it.
    pub fn copy_text(&self, range: Range<usize>) -> String {
        self.flatten(range, |plain, atom| {
            plain.text.push_str(atom.text());
        })
        .text
    }

    /// The text of `range` without its markers, and `at_atom` run
    /// at each one with the text so far.
    fn flatten(
        &self,
        range: Range<usize>,
        mut at_atom: impl FnMut(&mut Plain, &Atom),
    ) -> Plain {
        let mut plain = Plain::default();
        let mut from = range.start;
        for (at, atom) in
            self.atoms.iter().filter(|(at, _)| range.contains(at))
        {
            plain.text.push_str(&self.text[from..*at]);
            at_atom(&mut plain, atom);
            from = at + MARKER.len_utf8();
        }
        plain.text.push_str(&self.text[from..range.end]);
        plain
    }

    /// The spans to lay out: the text, cut where each atom's marker
    /// is, with the atom's box before the marker. The caret after the
    /// marker is then after the box. A `preedit` is put at the byte
    /// `at`, underlined, before the box when `at` is a marker's.
    pub fn spans(&self, preedit: Option<(usize, &str)>) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut from = 0;
        let mut preedit = preedit;
        let cuts = self
            .atoms
            .iter()
            .map(|(at, atom)| (*at, Some(atom)))
            .chain([(self.text.len(), None)]);
        for (end, atom) in cuts {
            let run = &self.text[from..end];
            match preedit.filter(|(at, _)| (from..=end).contains(at))
            {
                Some((at, typing)) => {
                    preedit = None;
                    let (before, after) = run.split_at(at - from);
                    spans.push(Span::new(before));
                    spans.push(
                        Span::new(typing).underline(LineColor::Ink),
                    );
                    spans.push(Span::new(after));
                }
                None => spans.push(Span::new(run)),
            }
            if let Some(atom) = atom {
                spans.push(Span::atom(atom.clone()));
            }
            from = end;
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Plain as Theme;

    fn chip(key: &str) -> Atom {
        Atom::new::<Theme>(key, format!("<{key}>"), || {
            unreachable!("not built")
        })
    }

    fn text(text: &str) -> Piece {
        Piece::Text(text.into())
    }

    /// `ab`, a, `cd`, b, `ef`: the markers are at 2 and 7.
    fn content() -> Content {
        Content::new([
            text("ab"),
            Piece::Atom(chip("a")),
            text("cd"),
            Piece::Atom(chip("b")),
            text("ef"),
        ])
    }

    fn keys(content: &Content) -> Vec<(usize, &str)> {
        content
            .atoms()
            .iter()
            .map(|(at, atom)| (*at, atom.key()))
            .collect()
    }

    #[test]
    fn atoms_are_where_the_text_before_them_ends() {
        assert_eq!(keys(&content()), [(2, "a"), (7, "b")]);
    }

    #[test]
    fn deleting_across_an_atom_drops_it_and_moves_the_next() {
        let mut content = content();
        content.splice(1..6, "");

        assert_eq!(content.len(), 1 + 1 + 3 + 2);
        assert_eq!(keys(&content), [(2, "b")]);
        let pieces = content.pieces();
        assert_eq!(pieces[0], text("ad"));
        assert_eq!(pieces[2], text("ef"));
    }

    #[test]
    fn typing_next_to_an_atom_keeps_it_in_place() {
        let mut content = content();
        content.splice(2..2, "x");
        content.splice(6..6, "y");

        assert_eq!(keys(&content), [(3, "a"), (9, "b")]);
        let marker = MARKER.to_string();
        assert_eq!(content.text()[3..6], marker);
        assert_eq!(content.text()[9..12], marker);
    }

    #[test]
    fn an_atom_replaces_the_selection() {
        let mut content = content();
        content.splice_atom(0..5, chip("c"));

        assert_eq!(keys(&content), [(0, "c"), (5, "b")]);
        assert_eq!(content.plain(0..content.len()).text, "cdef");
        assert_eq!(content.copy_text(0..content.len()), "<c>cd<b>ef");
    }

    #[test]
    fn plain_text_has_no_atom_in_it_but_where_each_stood() {
        let content = content();
        let plain = content.plain(0..content.len());

        assert_eq!(plain.text, "abcdef");
        assert_eq!(
            plain.atoms,
            [
                PlainAtom {
                    key: "a".into(),
                    at: 2
                },
                PlainAtom {
                    key: "b".into(),
                    at: 4
                },
            ]
        );
    }

    #[test]
    fn copied_text_has_the_copy_of_each_atom_in_it() {
        let content = content();

        assert_eq!(
            content.copy_text(0..content.len()),
            "ab<a>cd<b>ef"
        );
        assert_eq!(content.copy_text(5..7), "cd");
        assert_eq!(content.copy_text(1..7), "b<a>cd");
    }

    #[test]
    fn removing_an_atom_leaves_the_rest_and_says_where_it_was() {
        let mut content = content();

        assert_eq!(content.remove_atoms("a"), [2]);
        assert_eq!(keys(&content), [(4, "b")]);
        assert_eq!(content.plain(0..content.len()).text, "abcdef");
        assert!(content.remove_atoms("a").is_empty());
    }

    #[test]
    fn a_caret_is_found_by_atom_or_by_byte_of_the_plain_text() {
        let content = content();
        let index = |caret| content.index_of(&caret);

        assert_eq!(index(Caret::Before("b".into())), Some(7));
        assert_eq!(index(Caret::After("b".into())), Some(10));
        assert_eq!(index(Caret::After("x".into())), None);
        assert_eq!(index(Caret::End), Some(12));
        // Before the atom standing at byte 2 of "abcdef".
        assert_eq!(index(Caret::Offset(2)), Some(2));
        assert_eq!(index(Caret::Offset(3)), Some(6));
        assert_eq!(index(Caret::Offset(99)), Some(12));

        // Inside "é" is its start, which is before the atom there.
        let content =
            Content::new([Piece::Atom(chip("a")), text("é")]);
        assert_eq!(content.index_of(&Caret::Offset(1)), Some(0));
    }

    #[test]
    fn a_stand_in_for_an_atom_cannot_be_typed() {
        let typed = format!("a{MARKER}b");
        let content = Content::new([Piece::Text(typed)]);

        assert_eq!(content.text(), "ab");
    }

    fn shape(spans: &[Span]) -> Vec<String> {
        spans
            .iter()
            .map(|span| match &span.atom {
                Some(atom) => format!("[{}]", atom.key()),
                None => span.text.replace(MARKER, "|"),
            })
            .collect()
    }

    #[test]
    fn each_atom_is_before_its_marker() {
        assert_eq!(
            shape(&content().spans(None)),
            ["ab", "[a]", "|cd", "[b]", "|ef"]
        );
    }

    #[test]
    fn a_preedit_is_before_the_box_of_the_atom_after_it() {
        let content = content();
        let at = |at| shape(&content.spans(Some((at, "~"))));

        assert_eq!(
            at(1),
            ["a", "~", "b", "[a]", "|cd", "[b]", "|ef"]
        );
        assert_eq!(
            at(2),
            ["ab", "~", "", "[a]", "|cd", "[b]", "|ef"]
        );
        assert_eq!(
            at(5),
            ["ab", "[a]", "|", "~", "cd", "[b]", "|ef"]
        );
        assert_eq!(
            at(12),
            ["ab", "[a]", "|cd", "[b]", "|ef", "~", ""]
        );
    }
}
