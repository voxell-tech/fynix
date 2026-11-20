//! A rich field that holds more lines than it shows: the height it
//! stops growing at, and the scrolling that keeps the caret in view.
//!
//! The view is `lines` lines of its text high, whatever lines it
//! shows, and scrolls by the pixel to keep the caret's line whole in
//! it. A line with an atom taller than the text is taller, and shows
//! cut at an edge while the caret is elsewhere.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::system::Query;
use bevy::math::Vec2;
use bevy::text::{FontSize, TextFont};
use bevy::ui::{
    ComputedNode, Node, ScrollPosition, UiGlobalTransform, px,
};
use parley::{Brush, Layout};

use super::RichField;
use crate::scroll::ScrollGoal;
use crate::views::overlay::{DrawnLayout, line_tops, shift_under};
use crate::views::rich::whole_line;

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

/// The room each line of `layout` takes, one under the other, as
/// where it starts and ends in the layout's pixels.
fn rooms<B: Brush>(layout: &Layout<B>) -> Vec<(f32, f32)> {
    line_tops(layout)
        .into_iter()
        .zip(layout.lines())
        .map(|(top, line)| (top, top + line.metrics().line_height))
        .collect()
}

/// The line the point `y` of the layout is in.
fn line_at(rooms: &[(f32, f32)], y: f32) -> usize {
    rooms
        .iter()
        .position(|(top, bottom)| *top <= y && y < *bottom)
        .unwrap_or(rooms.len().saturating_sub(1))
}

/// How high the view of `lines` lines of a text in `font` is, in
/// logical pixels.
fn view_height(font: &TextFont, lines: usize) -> f32 {
    let size = match font.font_size {
        FontSize::Px(size) => size,
        _ => 0.0,
    };
    lines as f32 * whole_line(size)
}

/// Where a view `view` high, scrolled to `at` over `content` that
/// high, scrolls to for the line from `top` to `bottom` to be whole
/// in it: as little as it takes, and not past either end.
fn scrolled_to(
    (top, bottom): (f32, f32),
    at: f32,
    view: f32,
    content: f32,
) -> f32 {
    let to = if top < at {
        top
    } else if bottom > at + view {
        bottom - view
    } else {
        at
    };
    to.clamp(0.0, (content - view).max(0.0))
}

/// Whether a field's text is laid out nowhere, as in a tab not shown:
/// its view has no height then, and what it shows is left as it was
/// for when it shows again.
fn is_hidden(text: &ComputedNode) -> bool {
    text.size().x <= 0.0 || text.size().y <= 0.0
}

/// Gives the box of each field's text the height of the lines its
/// view shows at most, before the frame is laid out: one with fewer
/// lines is as high as they are, and one with more scrolls from the
/// frame it has them.
pub(super) fn cap(
    fields: Query<(&RichField, &MaxLines)>,
    texts: Query<(&TextFont, &ChildOf)>,
    mut bodies: Query<&mut Node>,
) {
    for (field, MaxLines(lines)) in &fields {
        let Ok((font, body)) = texts.get(field.text) else {
            continue;
        };
        let Ok(mut node) = bodies.get_mut(body.parent()) else {
            continue;
        };
        let height = px(view_height(font, *lines));
        if node.max_height != height {
            node.max_height = height;
        }
    }
}

/// Scrolls each field whose caret or view changed so the caret's
/// line is whole in view.
pub(super) fn reveal(
    mut fields: Query<(&mut RichField, &MaxLines)>,
    texts: Query<(&DrawnLayout, &ComputedNode, &TextFont, &ChildOf)>,
    mut bodies: Query<(
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
        &Children,
    )>,
    mut drawn: Query<(&mut UiGlobalTransform, Option<&Children>)>,
) {
    for (mut field, MaxLines(lines)) in &mut fields {
        let field = field.bypass_change_detection();
        let Ok((block, text, font, body)) = texts.get(field.text)
        else {
            continue;
        };
        let layout = &block.0;
        if field.is_stale(layout) || is_hidden(text) {
            continue;
        }
        let Ok((area, mut position, mut goal, kids)) =
            bodies.get_mut(body.parent())
        else {
            continue;
        };
        let scale = text.inverse_scale_factor();
        let rooms = rooms(layout);
        let caret =
            field.selection(layout).focus().geometry(layout, 1.0);
        let line =
            line_at(&rooms, ((caret.y0 + caret.y1) / 2.0) as f32);
        let view = view_height(font, *lines) / scale;
        let seen = Seen {
            line,
            count: rooms.len(),
            view,
        };
        if field.seen == Some(seen) {
            continue;
        }
        field.seen = Some(seen);
        let Some(&room) = rooms.get(line) else {
            continue;
        };
        let content = rooms.last().map_or(0.0, |(_, end)| *end);
        let to = scrolled_to(room, position.y / scale, view, content)
            * scale;
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
            // This frame's layout has run, so what it draws scrolls
            // now as the next layout will, clamped and floored as it
            // is, or the new lines would show a frame late.
            let physical = scale.recip();
            let room = (area.content_size.y - area.size.y).max(0.0);
            let next =
                (position.y * physical).clamp(0.0, room).floor();
            let moved = next - area.scroll_position.y;
            if moved != 0.0 {
                shift_under(kids, Vec2::new(0.0, -moved), &mut drawn);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_in_view_leaves_the_view_where_it_is() {
        assert_eq!(
            scrolled_to((20.0, 37.0), 10.0, 68.0, 200.0),
            10.0
        );
    }

    #[test]
    fn a_line_out_of_view_is_brought_to_the_nearest_edge() {
        assert_eq!(scrolled_to((0.0, 17.0), 10.0, 68.0, 200.0), 0.0);
        assert_eq!(
            scrolled_to((100.0, 120.0), 10.0, 68.0, 200.0),
            52.0
        );
    }

    #[test]
    fn a_view_never_scrolls_past_its_text() {
        assert_eq!(
            scrolled_to((190.0, 210.0), 0.0, 68.0, 200.0),
            132.0
        );
        assert_eq!(scrolled_to((0.0, 17.0), 0.0, 68.0, 40.0), 0.0);
    }
}
