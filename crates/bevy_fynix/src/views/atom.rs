//! Views laid out in the line of a text, the way a word is.
//!
//! An atom is an inline box in the text, which takes the size of its
//! view, and a slot over the text node's host where its view is
//! built. The slot is moved to the box each frame, as a text node
//! cannot hold nodes of its own.

use core::fmt;
use std::sync::Arc;

use bevy::app::{App, PostUpdate, Update};
use bevy::camera::visibility::{InheritedVisibility, Visibility};
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::lifecycle::Remove;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};
use bevy::ecs::world::World;
use bevy::math::{Rect, Vec2};
use bevy::platform::collections::{HashMap, HashSet};
use bevy::text::{InlineBox, InlineBoxKind, TextLayoutInfo};
use bevy::ui::widget::text_system;
use bevy::ui::{
    ComputedNode, Node, PositionType, UiGlobalTransform, px,
};
use parley::{Brush, Layout};

use crate::views::overlay;
use crate::{AnyView, Bevy, FynixSystems, mount_under};

type Spawn = dyn Fn(&mut World, Entity) -> Entity + Send + Sync;

/// A view in the middle of a text, such as a chip showing a file.
///
/// Its key tells it from the other atoms of the same text, so it
/// is unique there.
#[derive(Clone)]
pub struct Atom {
    key: String,
    text: String,
    spawn: Arc<Spawn>,
}

impl Atom {
    /// An atom showing the view `view` builds, which copies as
    /// `text`. It builds the view again each time the atom is put in
    /// a text.
    pub fn new<T: Send + Sync + 'static>(
        key: impl Into<String>,
        text: impl Into<String>,
        view: impl Fn() -> AnyView<Bevy, T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            text: text.into(),
            spawn: Arc::new(move |world, host| {
                let slot = world
                    .spawn((
                        // Put somewhere, for the first move to be
                        // from there.
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            top: px(0.0),
                            ..Node::default()
                        },
                        // Until it has a place in the line.
                        Visibility::Hidden,
                        Slot,
                        ChildOf(host),
                    ))
                    .id();
                mount_under::<T>(world, slot, view());
                slot
            }),
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    /// What the atom is where text is copied.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.text == other.text
            && Arc::ptr_eq(&self.spawn, &other.spawn)
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Atom")
            .field("key", &self.key)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

/// On a text node: the node its atoms' slots are children of, which
/// has the text node at its origin.
#[derive(Component)]
pub(crate) struct AtomHost(pub Entity);

/// How much room an atom leaves on each side of itself, in logical
/// pixels.
const GAP: f32 = 3.0;

/// On an inline box under a text node: the atom it holds a place
/// for, and the slot its view is in once there is one.
#[derive(Component)]
pub(crate) struct AtomBox {
    pub atom: Atom,
    slot: Option<Entity>,
    /// Where the view is, in the pixels of the text layout.
    pub chip: Rect,
    /// Whether the view has shown, once its line had room for it.
    shown: bool,
}

impl AtomBox {
    pub fn new(atom: Atom) -> Self {
        Self {
            atom,
            slot: None,
            chip: Rect::default(),
            shown: false,
        }
    }
}

/// The room the line of `layout` whose baseline is at `baseline`
/// takes, as where it starts and how high it is, in the layout's
/// pixels.
///
/// A box as high as its view makes its line that high, and sits on
/// the baseline, which can be above the bottom of the line. The view
/// goes in the middle of the room instead, so a line of views never
/// reaches into the next.
fn room<B: Brush>(
    layout: &Layout<B>,
    baseline: f32,
) -> Option<(f32, f32)> {
    let tops = overlay::line_tops(layout);
    layout
        .lines()
        .zip(tops)
        .min_by(|(a, _), (b, _)| {
            let off = |line: &parley::Line<'_, B>| {
                (line.metrics().baseline - baseline).abs()
            };
            off(a).total_cmp(&off(b))
        })
        .map(|(line, top)| (top, line.metrics().line_height))
}

/// The node an atom's view is built under.
#[derive(Component)]
struct Slot;

/// What placing a slot reads and writes of it.
type Placed = (
    &'static ComputedNode,
    &'static mut Node,
    &'static mut Visibility,
    &'static mut InheritedVisibility,
    &'static mut UiGlobalTransform,
    Option<&'static Children>,
);

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(release)
        .add_systems(Update, build.in_set(FynixSystems::Settle))
        .add_systems(
            PostUpdate,
            (overlay::keep_drawn, place).chain().after(text_system),
        );
}

/// On a text whose atoms have all had room in its lines: it has been
/// shown whole.
#[derive(Component)]
struct Settled;

/// Builds the view of each atom that has a host to be in.
fn build(
    boxes: Query<(Entity, &AtomBox, &ChildOf)>,
    hosts: Query<&AtomHost>,
    mut commands: Commands,
) {
    for (entity, held, parent) in &boxes {
        let Ok(host) = hosts.get(parent.parent()) else {
            continue;
        };
        if held.slot.is_some() {
            continue;
        }
        let (atom, host) = (held.atom.clone(), host.0);
        commands.queue(move |world: &mut World| {
            if world.get_entity(entity).is_err() {
                return;
            }
            let slot = (atom.spawn)(world, host);
            match world.get_mut::<AtomBox>(entity) {
                Some(mut held) => held.slot = Some(slot),
                None => {
                    world.entity_mut(slot).despawn();
                }
            }
        });
    }
}

/// Takes the view of an atom away with its box.
fn release(
    removed: On<Remove<AtomBox>>,
    boxes: Query<&AtomBox>,
    mut commands: Commands,
) {
    if let Ok(AtomBox {
        slot: Some(slot), ..
    }) = boxes.get(removed.entity)
    {
        commands.entity(*slot).try_despawn();
    }
}

/// Gives each box the size of its view and a gap on both sides, and
/// each view the place of its box, in the middle of its line's room.
///
/// A box sized now is laid out in the next frame. A text shown for
/// the first time is hidden until its lines have room for all its
/// atoms, so it never shows without them and then moves.
fn place(
    mut boxes: Query<(
        Entity,
        &mut AtomBox,
        &mut InlineBox,
        &ChildOf,
    )>,
    texts: Query<(
        &TextLayoutInfo,
        &ComputedNode,
        &overlay::DrawnLayout,
        &ChildOf,
        Has<Settled>,
    )>,
    mut slots: Query<Placed, With<Slot>>,
    // The texts, and the nodes of the views under the slots.
    mut under: Query<overlay::Under, Without<Slot>>,
    mut commands: Commands,
) {
    // Each box sized as its view, and the texts laid out without the
    // room one of them needs.
    let mut waiting = HashSet::<Entity>::new();
    for (_, held, mut inline, parent) in &mut boxes {
        let size = held
            .slot
            .and_then(|slot| slots.get(slot).ok())
            .map_or(Vec2::ZERO, |(measured, ..)| {
                measured.size() * measured.inverse_scale_factor()
            });
        let resized = size.x > 0.0
            && size.y > 0.0
            && inline.set_if_neq(InlineBox {
                kind: InlineBoxKind::InFlow,
                size: Vec2::new(size.x + 2.0 * GAP, size.y),
            });
        if resized || held.slot.is_none() {
            waiting.insert(parent.parent());
        }
    }
    // Whether each text with atoms is seen: one never shown whole
    // waits for its atoms' room.
    let mut shown = HashMap::<Entity, bool>::new();
    for (_, _, _, parent) in &boxes {
        let text = parent.parent();
        if shown.contains_key(&text) {
            continue;
        }
        let Ok((_, node, _, body, settled)) = texts.get(text) else {
            continue;
        };
        // Laid out nowhere, as in a tab not shown, its atoms have no
        // size to make room for yet.
        if node.size().x <= 0.0 {
            continue;
        }
        let whole = !waiting.contains(&text);
        let body_seen = under
            .get(body.parent())
            .is_ok_and(|(_, seen, ..)| seen.get());
        if let Ok((_, mut seen, ..)) = under.get_mut(text) {
            if !settled {
                seen.set_if_neq(if body_seen && whole {
                    InheritedVisibility::VISIBLE
                } else {
                    InheritedVisibility::HIDDEN
                });
            }
            shown.insert(text, seen.get());
        }
        if whole && !settled {
            commands.entity(text).insert(Settled);
        }
    }
    for (entity, mut held, _, parent) in &mut boxes {
        let Some(slot) = held.slot else {
            continue;
        };
        let Ok((layout, text, block, ..)) =
            texts.get(parent.parent())
        else {
            continue;
        };
        let text_seen = match shown.get(&parent.parent()) {
            Some(true) => InheritedVisibility::VISIBLE,
            _ => InheritedVisibility::HIDDEN,
        };
        let Ok((
            measured,
            mut node,
            mut visibility,
            mut seen,
            mut transform,
            kids,
        )) = slots.get_mut(slot)
        else {
            continue;
        };
        let scale = text.inverse_scale_factor();
        let size = measured.size() * measured.inverse_scale_factor();
        let placed = layout
            .inline_boxes
            .iter()
            .find(|(placed, ..)| *placed == entity)
            .map(|(_, _, rect)| *rect);
        // Whether the text was laid out with the room the view needs.
        let fits = !waiting.contains(&parent.parent());
        let Some(rect) = placed else {
            continue;
        };
        let gap = GAP / scale;
        let high = size.y / scale;
        let top = room(&block.0, rect.max.y)
            .map_or(rect.min.y, |(top, line_height)| {
                top + (line_height - high) / 2.0
            });
        let chip = Rect::new(
            rect.min.x + gap,
            top,
            rect.max.x - gap,
            top + high,
        );
        if held.chip != chip {
            held.chip = chip;
        }
        let moved = overlay::put_corner(
            &mut node,
            measured,
            &mut transform,
            chip.min * scale,
        );
        // A new view shows once the line has room for it, and stays.
        held.shown |= fits;
        let was = *seen;
        overlay::set_shown(
            &mut visibility,
            &mut seen,
            held.shown,
            text_seen,
        );
        if let Some(kids) = kids
            && (moved.is_some() || was != *seen)
        {
            overlay::follow(
                kids,
                moved.unwrap_or(Vec2::ZERO),
                seen.get(),
                &mut under,
            );
        }
    }
}
