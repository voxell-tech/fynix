//! Nodes put over a text where its layout says, such as a caret or an
//! atom's view.
//!
//! Bevy lays a text out after the rest of the UI, so a node put by
//! its layout is moved after the frame's layout has run, and would be
//! drawn where it was until the next frame. What the frame draws of
//! it is moved too.

use bevy::camera::visibility::{InheritedVisibility, Visibility};
use bevy::ecs::change_detection::{
    DetectChanges, DetectChangesMut, Mut, Ref,
};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::QueryFilter;
use bevy::ecs::system::Query;
use bevy::math::Vec2;
use bevy::text::{
    ComputedTextBlock, LineBreak, TextBrush, TextLayout,
};
use bevy::ui::{ComputedNode, Node, UiGlobalTransform, Val, px};
use parley::{AlignmentOptions, Brush, Layout};

/// On a text that has things put over it: its layout, its lines
/// broken as Bevy drew them.
///
/// Bevy breaks the lines of the layout it keeps at other widths as it
/// measures the text, and breaks them at the node's width again only
/// when the text or the node changes. What it keeps can then be
/// broken otherwise than what it drew, and what is put over it would
/// be lines off.
#[derive(Component, Default)]
pub(crate) struct DrawnLayout(pub Layout<TextBrush>);

/// Breaks the lines of each kept layout as Bevy drew them, once Bevy
/// has laid its text out.
pub(crate) fn keep_drawn(
    mut texts: Query<(
        Ref<ComputedTextBlock>,
        Ref<ComputedNode>,
        &TextLayout,
        &mut DrawnLayout,
    )>,
) {
    for (block, node, text, mut drawn) in &mut texts {
        if !block.is_changed()
            && !node.is_changed()
            && !drawn.is_added()
        {
            continue;
        }
        let mut layout = block.buffer().clone();
        let width = match text.linebreak {
            LineBreak::NoWrap => None,
            _ => Some(node.content_box().size().x),
        };
        layout.break_all_lines(width);
        layout
            .align(text.justify.into(), AlignmentOptions::default());
        drawn.0 = layout;
    }
}

/// Where each line of `layout` starts, in its pixels: the top of the
/// room the line takes, which the line after starts below. Its glyphs
/// and boxes can reach past it, so this is not its box.
pub(crate) fn line_tops<B: Brush>(layout: &Layout<B>) -> Vec<f32> {
    let mut tops = Vec::with_capacity(layout.len());
    let mut y = 0.0_f64;
    for line in layout.lines() {
        // As the layout puts its lines, at whole pixels.
        tops.push(y.round() as f32);
        y += f64::from(line.metrics().line_height);
    }
    tops
}

/// Puts the corner of `node` at `at`, in logical pixels from where
/// its position starts, keeping its size. It gives how far the frame
/// draws it from where it was, in physical pixels of the window, for
/// the nodes under it to move as far, or `None` when it did not move.
pub(crate) fn put_corner(
    node: &mut Node,
    computed: &ComputedNode,
    transform: &mut UiGlobalTransform,
    at: Vec2,
) -> Option<Vec2> {
    let (left, top) = (px(at.x), px(at.y));
    if node.left == left && node.top == top {
        return None;
    }
    let moved = match (node.left, node.top) {
        (Val::Px(x), Val::Px(y)) => {
            let scale = computed.inverse_scale_factor.recip();
            let before = transform.affine().translation;
            shift(transform, (at - Vec2::new(x, y)) * scale);
            Some(transform.affine().translation - before)
        }
        // Never laid out where it was put, so it has no place to move
        // from.
        _ => None,
    };
    node.left = left;
    node.top = top;
    moved
}

/// Moves what the frame draws of `transform`'s node by `by`, physical
/// pixels in the node's own space.
fn shift(transform: &mut UiGlobalTransform, by: Vec2) {
    let mut affine = transform.affine();
    affine.translation += affine.matrix2 * by;
    *transform = affine.into();
}

/// Shows `node` or hides it, under a parent seen as `parent` is. Bevy
/// works out what is seen before a text is laid out, so what this
/// frame draws is told too.
pub(crate) fn set_shown(
    visibility: &mut Mut<Visibility>,
    seen: &mut Mut<InheritedVisibility>,
    shown: bool,
    parent: InheritedVisibility,
) {
    visibility.set_if_neq(if shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    seen.set_if_neq(if shown && parent.get() {
        InheritedVisibility::VISIBLE
    } else {
        InheritedVisibility::HIDDEN
    });
}

/// What moving and showing the nodes under a node reads and writes of
/// each.
pub(crate) type Under = (
    &'static mut UiGlobalTransform,
    &'static mut InheritedVisibility,
    &'static Visibility,
    Option<&'static Children>,
);

/// Moves what the frame draws of each node under `kids`, and of the
/// nodes under them, by `by`, physical pixels of the window, and has
/// it seen as their parent is, `seen`.
pub(crate) fn follow<F: QueryFilter>(
    kids: &Children,
    by: Vec2,
    seen: bool,
    under: &mut Query<Under, F>,
) {
    let mut stack =
        kids.iter().map(|kid| (*kid, seen)).collect::<Vec<_>>();
    while let Some((kid, parent)) = stack.pop() {
        let Ok((mut transform, mut inherited, visibility, children)) =
            under.get_mut(kid)
        else {
            continue;
        };
        if by != Vec2::ZERO {
            let mut affine = transform.affine();
            affine.translation += by;
            *transform = affine.into();
        }
        let seen = match visibility {
            Visibility::Inherited => parent,
            Visibility::Visible => true,
            Visibility::Hidden => false,
        };
        inherited.set_if_neq(if seen {
            InheritedVisibility::VISIBLE
        } else {
            InheritedVisibility::HIDDEN
        });
        if let Some(children) = children {
            stack.extend(children.iter().map(|kid| (*kid, seen)));
        }
    }
}
