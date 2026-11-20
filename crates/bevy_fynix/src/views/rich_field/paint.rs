//! The caret and the selection of a rich field, drawn as nodes over
//! its text, and where the input method puts its candidates.

use bevy::camera::visibility::{InheritedVisibility, Visibility};
use bevy::color::{Alpha, Color};
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::system::{Commands, Local, Query, Res};
use bevy::ecs::world::World;
use bevy::input_focus::InputFocus;
use bevy::math::{Rect, Vec2};
use bevy::picking::Pickable;
use bevy::text::{FontSize, TextFont, TextLayoutInfo};
use bevy::time::{Real, Time};
use bevy::ui::{
    BackgroundColor, ComputedNode, ComputedUiRenderTargetInfo, Node,
    PositionType, UiGlobalTransform, UiScale, ZIndex, px,
};
use bevy::window::{PrimaryWindow, Window};
use parley::editing::Cursor;
use parley::{
    BoundingBox, Brush, Layout, PositionedInlineBox,
    PositionedLayoutItem,
};

use super::RichField;
use crate::Focused;
use crate::views::atom::AtomBox;
use crate::views::overlay::{DrawnLayout, put, set_shown};

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
        // Put somewhere, for the first move to be from there.
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: px(0.0),
            height: px(0.0),
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

/// How many shapes a field has ready when it is built. A shape made
/// while painting is drawn from the next frame, so the shapes a field
/// shows at first, such as a quote's bars, are made with it.
const READY: usize = 8;

/// The shapes a field over `body` has ready when it is built.
pub(super) fn spawn_shapes(
    world: &mut World,
    body: Entity,
) -> Vec<Entity> {
    (0..READY)
        .map(|_| world.spawn(overlay(BEHIND, body)).id())
        .collect()
}

/// The box of `cursor`, as high as the text beside it: the line is
/// taller where an atom is, and the caret stays the height of its
/// text. `markers` are the indices of the atoms' markers, in order.
pub(super) fn caret_box<B: Brush>(
    cursor: &Cursor,
    layout: &Layout<B>,
    width: f32,
    markers: impl IntoIterator<Item = usize>,
) -> BoundingBox {
    let mut area = cursor.geometry(layout, width);
    // With nothing before it on its line, the caret is put by the
    // cluster after it, which for an atom is its marker, after its
    // box. It goes before the box.
    let [before, _] = cursor.logical_clusters(layout);
    let atom = markers
        .into_iter()
        .position(|marker| marker == cursor.index());
    if let Some(atom) = atom
        && before.is_none_or(|before| before.is_end_of_line())
        && let Some(x) = inline_boxes(layout).nth(atom).map(|b| b.x)
    {
        area.x1 = f64::from(x) + area.width();
        area.x0 = f64::from(x);
    }
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

/// The inline boxes of `layout`, in order.
fn inline_boxes<B: Brush>(
    layout: &Layout<B>,
) -> impl Iterator<Item = PositionedInlineBox> + '_ {
    layout.lines().flat_map(|line| {
        line.items().filter_map(|item| match item {
            PositionedLayoutItem::InlineBox(placed) => Some(placed),
            PositionedLayoutItem::GlyphRun(_) => None,
        })
    })
}

fn rect_of(bounds: BoundingBox) -> Rect {
    Rect::new(
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    )
}

/// What painting a caret or a shape reads and writes of its node.
type Shape = (
    &'static mut Node,
    &'static mut Visibility,
    &'static mut InheritedVisibility,
    &'static mut BackgroundColor,
    &'static mut ZIndex,
    &'static mut ComputedNode,
    &'static mut UiGlobalTransform,
);

pub(super) fn paint(
    mut fields: Query<(&mut RichField, Has<Focused>)>,
    texts: Query<(
        &DrawnLayout,
        &TextLayoutInfo,
        &ComputedNode,
        &TextFont,
        &ChildOf,
        &InheritedVisibility,
    )>,
    mut nodes: Query<Shape, Without<DrawnLayout>>,
    chips: Query<&AtomBox>,
    time: Option<Res<Time<Real>>>,
    mut commands: Commands,
) {
    for (mut field, focused) in &mut fields {
        let field = field.bypass_change_detection();
        let Ok((block, info, text, font, parent, text_seen)) =
            texts.get(field.text)
        else {
            continue;
        };
        let layout = &block.0;
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
                    let mut rect = rect_of(caret_box(
                        &selection.focus(),
                        layout,
                        width,
                        field.content.markers(),
                    ));
                    // After spaces at the end of a line, the caret is
                    // put past the text's box: it stays at its edge.
                    let room =
                        (text.size().x - rect.width()).max(0.0);
                    let x = rect.min.x.clamp(0.0, room);
                    rect = Rect::new(
                        x,
                        rect.min.y,
                        x + rect.width(),
                        rect.max.y,
                    );
                    caret = Some(rect);
                }
            } else {
                let fill = field.selection_color;
                // A line selected to its end reaches as far as the
                // layout could, past the text's box, which clips only
                // what it scrolls.
                let wide = text.size().x;
                selection.geometry_with(layout, |bounds, _| {
                    let mut rect = rect_of(bounds);
                    rect.min.x = rect.min.x.clamp(0.0, wide);
                    rect.max.x = rect.max.x.clamp(rect.min.x, wide);
                    shapes.push((rect, fill, BEHIND));
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
            let Ok((
                mut node,
                mut visibility,
                mut seen,
                mut fill,
                mut z,
                mut computed,
                mut transform,
            )) = nodes.get_mut(entity)
            else {
                continue;
            };
            let shape = shapes.get(index);
            if let Some(&(rect, color, front)) = shape {
                put(
                    &mut node,
                    &mut computed,
                    &mut transform,
                    Rect {
                        min: rect.min * scale,
                        max: rect.max * scale,
                    },
                );
                fill.set_if_neq(BackgroundColor(color));
                z.set_if_neq(ZIndex(front));
            }
            set_shown(
                &mut visibility,
                &mut seen,
                shape.is_some(),
                *text_seen,
            );
        }
        if let Ok((
            mut node,
            mut visibility,
            mut seen,
            mut fill,
            _,
            mut computed,
            mut transform,
        )) = nodes.get_mut(field.caret)
        {
            if let Some(rect) = caret {
                put(
                    &mut node,
                    &mut computed,
                    &mut transform,
                    Rect {
                        min: rect.min * scale,
                        max: rect.max * scale,
                    },
                );
                fill.set_if_neq(BackgroundColor(field.caret_color));
            }
            set_shown(
                &mut visibility,
                &mut seen,
                caret.is_some(),
                *text_seen,
            );
        }
    }
}

/// Keeps the input method on while a field has the focus, with its
/// candidates under the caret.
pub(super) fn ime_window(
    fields: Query<&RichField>,
    focus: Res<InputFocus>,
    texts: Query<(
        &DrawnLayout,
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
    let layout = &block.0;
    if field.is_stale(layout) {
        return;
    }
    let area = caret_box(
        &field.selection(layout).focus(),
        layout,
        1.0,
        field.content.markers(),
    );
    let local = Vec2::new(area.x0 as f32, area.y1 as f32)
        + node.content_box().min;
    let position = transform.affine().transform_point2(local)
        * ui_scale.map_or(1.0, |scale| scale.0)
        / target.scale_factor();
    if window.ime_position != position {
        window.ime_position = position;
    }
}
