//! The caret and the selection of a rich field, drawn as nodes over
//! its text, and where the input method puts its candidates.

use bevy::camera::visibility::Visibility;
use bevy::color::{Alpha, Color};
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Local, Query, Res};
use bevy::ecs::world::World;
use bevy::input_focus::InputFocus;
use bevy::math::{Rect, Vec2};
use bevy::picking::Pickable;
use bevy::text::{
    ComputedTextBlock, FontSize, TextFont, TextLayoutInfo,
};
use bevy::time::{Real, Time};
use bevy::ui::{
    BackgroundColor, ComputedNode, ComputedUiRenderTargetInfo, Node,
    PositionType, UiGlobalTransform, UiScale, ZIndex, px,
};
use bevy::window::{PrimaryWindow, Window};
use parley::editing::Cursor;
use parley::{BoundingBox, Brush, Layout};

use super::RichField;
use crate::Focused;
use crate::views::atom::AtomBox;

/// The caret is on for this long and off for as long.
const BLINK: f32 = 0.5;

/// How wide the caret is, in the font's size.
const CARET_WIDTH: f32 = 0.2;

/// How opaque a selected atom's veil is, over the atom's own fill.
const VEIL: f32 = 0.4;

/// Behind the text, for the selection of text.
const BEHIND: i32 = -1;

/// In front of the atoms, for the caret and the veils.
const FRONT: i32 = 1;

fn overlay(z: i32, parent: Entity) -> impl bevy::ecs::bundle::Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            ..Node::default()
        },
        BackgroundColor(Color::NONE),
        Visibility::Hidden,
        ZIndex(z),
        Pickable::IGNORE,
        ChildOf(parent),
    )
}

pub(super) fn spawn_caret(world: &mut World, body: Entity) -> Entity {
    world.spawn(overlay(FRONT, body)).id()
}

/// The box of `cursor`, as high as the text beside it: the line is
/// taller where an atom is, and the caret stays the height of its
/// text.
pub(super) fn caret_box<B: Brush>(
    cursor: &Cursor,
    layout: &Layout<B>,
    width: f32,
) -> BoundingBox {
    let mut area = cursor.geometry(layout, width);
    let middle = ((area.y0 + area.y1) / 2.0) as f32;
    let line = layout.lines().find(|line| {
        let line = line.metrics();
        line.block_min_coord <= middle
            && middle < line.block_max_coord
    });
    if let Some(line) = line
        && let Some(run) = line.runs().next()
    {
        let (line, run) = (line.metrics(), run.metrics());
        area.y0 = f64::from(line.baseline - run.ascent);
        area.y1 = f64::from(line.baseline + run.descent);
    }
    area
}

fn rect_of(bounds: BoundingBox) -> Rect {
    Rect::new(
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    )
}

/// Puts `node` over `rect`, in logical pixels.
fn place(node: &mut Node, rect: Rect) {
    let (left, top) = (px(rect.min.x), px(rect.min.y));
    let (width, height) = (px(rect.width()), px(rect.height()));
    if node.left != left
        || node.top != top
        || node.width != width
        || node.height != height
    {
        node.left = left;
        node.top = top;
        node.width = width;
        node.height = height;
    }
}

pub(super) fn paint(
    mut fields: Query<(&mut RichField, Has<Focused>)>,
    texts: Query<(
        &ComputedTextBlock,
        &TextLayoutInfo,
        &ComputedNode,
        &TextFont,
        &ChildOf,
    )>,
    mut nodes: Query<(
        &mut Node,
        &mut Visibility,
        &mut BackgroundColor,
        &mut ZIndex,
    )>,
    chips: Query<&AtomBox>,
    time: Option<Res<Time<Real>>>,
    mut commands: Commands,
) {
    for (mut field, focused) in &mut fields {
        let field = field.bypass_change_detection();
        let Ok((block, info, text, font, parent)) =
            texts.get(field.text)
        else {
            continue;
        };
        let layout = block.buffer();
        if field.is_stale(layout) {
            continue;
        }
        let scale = text.inverse_scale_factor();
        let selection = field.selection(layout);
        let mut shapes = Vec::<(Rect, Color, i32)>::new();
        let mut caret = None;
        if focused && field.preedit.is_none() {
            if selection.is_collapsed() {
                let size = match font.font_size {
                    FontSize::Px(size) => size,
                    _ => 0.0,
                };
                let width = CARET_WIDTH * size / scale;
                let since = time.as_ref().map_or(0.0, |time| {
                    time.elapsed()
                        .saturating_sub(field.active)
                        .as_secs_f32()
                });
                if since % (2.0 * BLINK) < BLINK {
                    caret = Some(rect_of(caret_box(
                        &selection.focus(),
                        layout,
                        width,
                    )));
                }
            } else {
                let fill = field.selection_color;
                selection.geometry_with(layout, |bounds, _| {
                    shapes.push((rect_of(bounds), fill, BEHIND));
                });
                let range = selection.text_range();
                let veil = fill.with_alpha(VEIL);
                let held = field.content.atoms().iter();
                for ((at, _), (entity, _, rect)) in
                    held.zip(&info.inline_boxes)
                {
                    if range.contains(at) {
                        let chip = chips
                            .get(*entity)
                            .map_or(*rect, |held| held.chip);
                        shapes.push((chip, veil, FRONT));
                    }
                }
            }
        }
        while field.selected.len() < shapes.len() {
            let entity =
                commands.spawn(overlay(BEHIND, parent.parent())).id();
            field.selected.push(entity);
        }
        for (index, &entity) in field.selected.iter().enumerate() {
            let Ok((mut node, mut visibility, mut fill, mut z)) =
                nodes.get_mut(entity)
            else {
                continue;
            };
            match shapes.get(index) {
                Some(&(rect, color, front)) => {
                    place(
                        &mut node,
                        Rect {
                            min: rect.min * scale,
                            max: rect.max * scale,
                        },
                    );
                    fill.set_if_neq(BackgroundColor(color));
                    z.set_if_neq(ZIndex(front));
                    visibility.set_if_neq(Visibility::Inherited);
                }
                None => {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
        }
        if let Ok((mut node, mut visibility, mut fill, _)) =
            nodes.get_mut(field.caret)
        {
            match caret {
                Some(rect) => {
                    place(
                        &mut node,
                        Rect {
                            min: rect.min * scale,
                            max: rect.max * scale,
                        },
                    );
                    fill.set_if_neq(BackgroundColor(
                        field.caret_color,
                    ));
                    visibility.set_if_neq(Visibility::Inherited);
                }
                None => {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}

/// Keeps the input method on while a field has the focus, with its
/// candidates under the caret.
pub(super) fn ime_window(
    fields: Query<&RichField>,
    focus: Res<InputFocus>,
    texts: Query<(
        &ComputedTextBlock,
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiRenderTargetInfo,
    )>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    ui_scale: Option<Res<UiScale>>,
    mut enabled: Local<bool>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Some(field) =
        focus.get().and_then(|node| fields.get(node).ok())
    else {
        if *enabled {
            window.ime_enabled = false;
            *enabled = false;
        }
        return;
    };
    *enabled = true;
    if !window.ime_enabled {
        window.ime_enabled = true;
    }
    let Ok((block, node, transform, target)) = texts.get(field.text)
    else {
        return;
    };
    let layout = block.buffer();
    if field.is_stale(layout) {
        return;
    }
    let area =
        caret_box(&field.selection(layout).focus(), layout, 1.0);
    let local = Vec2::new(area.x0 as f32, area.y1 as f32)
        + node.content_box().min;
    let position = transform.affine().transform_point2(local)
        * ui_scale.map_or(1.0, |scale| scale.0)
        / target.scale_factor();
    if window.ime_position != position {
        window.ime_position = position;
    }
}
