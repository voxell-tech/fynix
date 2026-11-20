//! A field that draws a little Markdown as it is typed: `#` headings,
//! `>` quotes, `*italic*` or `_italic_`, `**bold**` and `` `code` ``,
//! their signs hidden until the caret comes to them.

use core::ops::Range;

use bevy::color::Color;
use bevy::text::{FontStyle, FontWeight};
use bevy::ui::px;
use bevy_fynix::views::{
    FrameProps, Look, Mark, MarkKind, Piece, rich_field,
};
use bevy_fynix::{Bevy, View};

use crate::common::Monokai;

const DIM: Color = Color::srgb(0.6, 0.6, 0.55);
const BAR: Color = Color::srgb(0.9, 0.75, 0.3);
const CODE: Color = Color::srgba(1.0, 1.0, 1.0, 0.1);

pub fn markdown() -> impl View<Bevy, Monokai> {
    rich_field()
        .max_lines(8)
        .content([Piece::Text(
            "# A heading\n> A quote, with **bold** in it\nSome \
             **bold**, *italic*, _also italic_ and ***both*** \
             words, and `code`."
                .into(),
        )])
        .marks(marks)
        .width(px(420.0))
}

fn hidden(range: Range<usize>, shown_in: Range<usize>) -> Mark {
    Mark {
        range,
        kind: MarkKind::Hidden { shown_in },
    }
}

fn look(range: Range<usize>, look: Look) -> Mark {
    Mark {
        range,
        kind: MarkKind::Look(look),
    }
}

fn marks(text: &str) -> Vec<Mark> {
    let mut marks = Vec::new();
    let mut start = 0;
    for line in text.split('\n') {
        let end = start + line.len();
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        let rest = &line[hashes..];
        if (1..=3).contains(&hashes)
            && rest.len() > 1
            && rest.starts_with(' ')
        {
            let signs = start..start + hashes + 1;
            marks.push(hidden(signs.clone(), signs.clone()));
            marks.push(look(
                signs.end..end,
                Look {
                    size: Some(24.0 - 3.0 * hashes as f32),
                    weight: Some(FontWeight::BOLD),
                    ..Look::default()
                },
            ));
        } else if line.len() > 2 && line.starts_with("> ") {
            let signs = start..start + 2;
            marks.push(hidden(signs.clone(), signs));
            marks.push(Mark {
                range: start..end,
                kind: MarkKind::Bar {
                    color: BAR,
                    x: -7.0,
                    width: 3.0,
                },
            });
            marks.push(look(
                start..end,
                Look {
                    color: Some(DIM),
                    ..Look::default()
                },
            ));
        }
        let code = code(text, start..end, &mut marks);
        emphasis(text, start..end, &code, &mut marks);
        start = end + 1;
    }
    marks
}

/// Marks each `` `code` `` on `line`, and gives where each is.
fn code(
    text: &str,
    line: Range<usize>,
    marks: &mut Vec<Mark>,
) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut at = line.start;
    while let Some(open) = text[at..line.end].find('`') {
        let open = at + open;
        let Some(close) = text[open + 1..line.end].find('`') else {
            break;
        };
        let close = open + 1 + close;
        let whole = open..close + 1;
        if close > open + 1 {
            marks.push(hidden(open..open + 1, whole.clone()));
            marks.push(look(
                open + 1..close,
                Look {
                    background: Some(CODE),
                    ..Look::default()
                },
            ));
            marks.push(hidden(close..whole.end, whole.clone()));
        }
        spans.push(whole.clone());
        at = whole.end;
    }
    spans
}

/// Marks each run of `*` or `_` on `line`, outside `code`, that a
/// like run closes: one for italic, two for bold, three for both. A
/// `_` inside a word, as in `snake_case`, is a letter.
fn emphasis(
    text: &str,
    line: Range<usize>,
    code: &[Range<usize>],
    marks: &mut Vec<Mark>,
) {
    let bytes = text.as_bytes();
    let word_before = |at: usize| {
        text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
    };
    let word_after = |at: usize| {
        text[at..].chars().next().is_some_and(char::is_alphanumeric)
    };
    // Each open run: its sign, where it starts, and how long it is.
    let mut open = Vec::<(u8, usize, usize)>::new();
    let mut at = line.start;
    // The code spans are in order, so the one that may hold `at` only
    // moves forward.
    let mut span = 0;
    while at < line.end {
        let sign = bytes[at];
        while code.get(span).is_some_and(|s| s.end <= at) {
            span += 1;
        }
        if !matches!(sign, b'*' | b'_')
            || code.get(span).is_some_and(|s| s.contains(&at))
        {
            at += 1;
            continue;
        }
        let mut end = at;
        while end < line.end && bytes[end] == sign {
            end += 1;
        }
        let len = end - at;
        let closes = open
            .iter()
            .rposition(|(was, _, long)| *was == sign && *long == len)
            .filter(|_| sign == b'*' || !word_after(end));
        match closes {
            Some(index) => {
                let (_, start, _) = open[index];
                open.truncate(index);
                let whole = start..end;
                if at > start + len && len <= 3 {
                    marks.push(hidden(
                        start..start + len,
                        whole.clone(),
                    ));
                    marks
                        .push(look(start + len..at, emphasized(len)));
                    marks.push(hidden(at..end, whole));
                }
            }
            None if sign == b'*' || !word_before(at) => {
                open.push((sign, at, len));
            }
            None => {}
        }
        at = end;
    }
}

fn emphasized(signs: usize) -> Look {
    Look {
        weight: (signs >= 2).then_some(FontWeight::BOLD),
        style: (signs != 2).then_some(FontStyle::Italic),
        ..Look::default()
    }
}
