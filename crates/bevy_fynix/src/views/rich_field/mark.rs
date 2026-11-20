//! How ranges of a rich field's text are drawn: in a look of their
//! own, hidden until the caret comes to them, or with a bar or a fill
//! along their lines.

use core::ops::Range;
use std::sync::Arc;

use bevy::color::Color;

use crate::views::Look;

/// How a range of a [`RichField`](super::RichField)'s text is drawn.
/// The range is in bytes of the text the marks were asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub range: Range<usize>,
    pub kind: MarkKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkKind {
    /// Drawn in this look, over the field's own and the looks of the
    /// marks before it.
    Look(Look),
    /// Not drawn and taking no room, unless the field has the focus
    /// and its selection touches `shown_in`, the range of what the
    /// mark belongs to, such as all of `**bold**` for its `**`. The
    /// text stays in the field and is edited as any other: the caret
    /// that comes to it shows it.
    Hidden { shown_in: Range<usize> },
    /// A bar along each line the range is laid out on, `x` from the
    /// left of the text, where a negative `x` is left of it.
    Bar { color: Color, x: f32, width: f32 },
    /// A fill across each line the range is laid out on, behind the
    /// text.
    Fill(Color),
}

/// What a field asks for the marks of its text.
pub(super) type Marking =
    Arc<dyn Fn(&str) -> Vec<Mark> + Send + Sync>;

/// The size hidden text is laid out at. Bevy leaves out a span of
/// size 0, and the text would lose its bytes.
const HIDDEN_SIZE: f32 = 0.01;

/// The marks `marking` gives for `text`, without those whose range
/// is not one of `text`.
pub(super) fn marks_of(marking: &Marking, text: &str) -> Vec<Mark> {
    let mut marks = marking(text);
    marks.retain(|mark| {
        let Range { start, end } = mark.range;
        start <= end
            && text.is_char_boundary(start)
            && text.is_char_boundary(end)
    });
    marks
}

/// The marks drawn while `selected` is the selection, or while the
/// field does not have the focus when it is `None`: `marks` without
/// the hidden ones the selection shows.
pub(super) fn drawn(
    marks: &[Mark],
    selected: Option<Range<usize>>,
) -> Vec<Mark> {
    marks
        .iter()
        .filter(|mark| match (&mark.kind, &selected) {
            (MarkKind::Hidden { shown_in }, Some(selected)) => {
                !touches(shown_in, selected)
            }
            _ => true,
        })
        .cloned()
        .collect()
}

/// Whether `a` and `b` overlap or meet, so a caret at either end of a
/// range touches it.
fn touches(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && b.start <= a.end
}

/// The look of the text from `at` to `to`, under `marks`.
pub(super) fn look_of(marks: &[Mark], at: usize, to: usize) -> Look {
    let over = marks.iter().filter(|mark| {
        mark.range.start <= at && to <= mark.range.end
    });
    let mut look = Look::default();
    for mark in over {
        match &mark.kind {
            MarkKind::Look(top) => look = layered(look, top),
            MarkKind::Hidden { .. } => {
                return Look {
                    color: Some(Color::NONE),
                    size: Some(HIDDEN_SIZE),
                    ..Look::default()
                };
            }
            _ => {}
        }
    }
    look
}

/// `top` over `base`: each part `top` leaves unset is `base`'s.
fn layered(base: Look, top: &Look) -> Look {
    Look {
        color: top.color.or(base.color),
        font: top.font.clone().or(base.font),
        size: top.size.or(base.size),
        weight: top.weight.or(base.weight),
        style: top.style.or(base.style),
        background: top.background.or(base.background),
        underline: top.underline.or(base.underline),
        strikethrough: top.strikethrough.or(base.strikethrough),
    }
}

/// The bytes where a mark that changes the text's look starts or
/// ends, strictly inside `range`.
pub(super) fn cuts_in(
    marks: &[Mark],
    range: &Range<usize>,
) -> impl Iterator<Item = usize> {
    marks
        .iter()
        .filter(|mark| {
            matches!(
                mark.kind,
                MarkKind::Look(_) | MarkKind::Hidden { .. }
            )
        })
        .flat_map(|mark| [mark.range.start, mark.range.end])
        .filter(|at| range.start < *at && *at < range.end)
        .collect::<Vec<usize>>()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hidden(range: Range<usize>, shown_in: Range<usize>) -> Mark {
        Mark {
            range,
            kind: MarkKind::Hidden { shown_in },
        }
    }

    #[test]
    fn hidden_text_shows_while_the_selection_touches_what_it_marks() {
        // `x **b** y`, its stars hidden.
        let marks = [hidden(2..4, 2..7), hidden(5..7, 2..7)];
        let count = |selected| drawn(&marks, selected).len();

        assert_eq!(count(None), 2);
        assert_eq!(count(Some(1..1)), 2);
        // At either edge, inside, or a selection over part of it.
        assert_eq!(count(Some(2..2)), 0);
        assert_eq!(count(Some(4..4)), 0);
        assert_eq!(count(Some(7..7)), 0);
        assert_eq!(count(Some(0..3)), 0);
        assert_eq!(count(Some(8..8)), 2);
    }
}
