//! A rich field that holds more lines than it shows: the height it
//! stops growing at, and the scrolling that keeps the caret in view.
//!
//! The view always starts at the top of a line and shows `lines`
//! whole ones, so its height is that of the lines it shows: a line
//! with a tall atom in it makes the field taller while it is in view.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::Query;
use bevy::math::Vec2;
use bevy::text::ComputedTextBlock;
use bevy::ui::{ComputedNode, Node, ScrollPosition, Val, px};
use parley::{Brush, Layout};

use super::RichField;
use crate::scroll::ScrollGoal;

/// On a field's root: how many lines its text shows before it
/// scrolls.
#[derive(Component)]
pub(super) struct MaxLines(pub usize);

/// What the view last scrolled to show the caret for: the line it
/// was on, how many lines there were, and how high the view was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Seen {
    line: usize,
    count: usize,
    view: f32,
}

/// The line a view scrolled to `y`, in the pixels of the layout,
/// starts at: the one that reaches below `y`.
fn first_line<B: Brush>(layout: &Layout<B>, y: f32) -> usize {
    layout
        .lines()
        .position(|line| line.metrics().block_max_coord > y + 0.5)
        .unwrap_or(layout.len().saturating_sub(1))
}

/// Where a view starting at line `first` starts, in the pixels of the
/// layout: the top of the line, or the bottom of the line before it
/// when that reaches further, as the line of a tall atom does.
fn view_top<B: Brush>(layout: &Layout<B>, first: usize) -> f32 {
    let top = |line: usize| {
        layout.get(line).map(|line| line.metrics().block_min_coord)
    };
    let before = first
        .checked_sub(1)
        .and_then(|line| layout.get(line))
        .map(|line| line.metrics().block_max_coord);
    top(first).unwrap_or(0.0).max(before.unwrap_or(f32::MIN))
}

/// The line the point `y` of the layout is in.
fn line_at<B: Brush>(layout: &Layout<B>, y: f32) -> usize {
    layout
        .lines()
        .position(|line| {
            let line = line.metrics();
            line.block_min_coord <= y && y < line.block_max_coord
        })
        .unwrap_or(layout.len().saturating_sub(1))
}

/// The first line of a view of `lines` of the `count` lines that
/// shows `caret`, when it starts at `first`: the same one if it shows
/// the caret already.
fn first_in_view(
    first: usize,
    caret: usize,
    lines: usize,
    count: usize,
) -> usize {
    let first = if caret < first {
        caret
    } else if caret >= first + lines {
        caret + 1 - lines
    } else {
        first
    };
    first.min(count.saturating_sub(lines))
}

/// Gives the box of each field's text the height of the lines its
/// view shows, or none to grow by while there are no more than it
/// shows.
pub(super) fn cap(
    fields: Query<(&RichField, &MaxLines)>,
    texts: Query<(&ComputedTextBlock, &ComputedNode, &ChildOf)>,
    mut bodies: Query<&mut Node>,
) {
    for (field, MaxLines(lines)) in &fields {
        let Ok((block, text, body)) = texts.get(field.text) else {
            continue;
        };
        let layout = block.buffer();
        if field.is_stale(layout) {
            continue;
        }
        let Ok(mut node) = bodies.get_mut(body.parent()) else {
            continue;
        };
        let first =
            field.first.min(layout.len().saturating_sub(*lines));
        let last = layout.get(first + lines - 1);
        let height = match last {
            // Rounded down, so the view can always scroll as far as
            // the top of its first line.
            Some(last) if layout.len() > *lines => {
                px((last.metrics().block_max_coord
                    - view_top(layout, first))
                .floor()
                    * text.inverse_scale_factor())
            }
            _ => Val::Auto,
        };
        if node.max_height != height {
            node.max_height = height;
        }
    }
}

/// Scrolls each field whose caret or view changed so its view starts
/// at a line, and shows the caret.
pub(super) fn reveal(
    mut fields: Query<(&mut RichField, &MaxLines)>,
    texts: Query<(&ComputedTextBlock, &ComputedNode, &ChildOf)>,
    mut bodies: Query<(
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
    )>,
) {
    for (mut field, MaxLines(lines)) in &mut fields {
        let field = field.bypass_change_detection();
        let Ok((block, text, body)) = texts.get(field.text) else {
            continue;
        };
        let layout = block.buffer();
        if field.is_stale(layout) {
            continue;
        }
        let Ok((area, mut position, mut goal)) =
            bodies.get_mut(body.parent())
        else {
            continue;
        };
        let scale = text.inverse_scale_factor();
        let caret =
            field.selection(layout).focus().geometry(layout, 1.0);
        let line =
            line_at(layout, ((caret.y0 + caret.y1) / 2.0) as f32);
        let seen = Seen {
            line,
            count: layout.len(),
            view: (area.size() - area.scrollbar_size).y
                * area.inverse_scale_factor(),
        };
        if field.seen == Some(seen) {
            continue;
        }
        field.seen = Some(seen);
        let first = if layout.len() > *lines {
            first_in_view(
                first_line(layout, position.y / scale),
                line,
                *lines,
                layout.len(),
            )
        } else {
            0
        };
        field.first = first;
        // A whole pixel, so no row of the line before shows.
        let to = view_top(layout, first).ceil() * scale;
        let by = to - position.y;
        if by != 0.0 {
            // An ease in flight would carry on from where it was
            // headed.
            *goal = ScrollGoal::default();
            goal.scroll_by(
                &mut position,
                Vec2::Y * by,
                Vec2::Y * to,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caret_in_view_leaves_the_view_where_it_is() {
        assert_eq!(first_in_view(2, 3, 3, 10), 2);
        assert_eq!(first_in_view(2, 4, 3, 10), 2);
    }

    #[test]
    fn a_caret_out_of_view_brings_the_nearest_line_to_the_edge() {
        assert_eq!(first_in_view(5, 2, 3, 10), 2);
        assert_eq!(first_in_view(2, 9, 3, 10), 7);
    }

    #[test]
    fn a_view_never_starts_so_late_that_it_runs_out_of_lines() {
        assert_eq!(first_in_view(8, 8, 3, 10), 7);
    }
}
