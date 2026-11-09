//! Views laid out in the line of a text, the way a word is.
//!
//! An atom is an inline box in the text, which takes the size of its
//! view, and a slot over the text node's host where its view is
//! built. The slot is moved to the box each frame, as a text node
//! cannot hold nodes of its own.

use core::fmt;
use std::sync::Arc;

use bevy::app::{App, PostUpdate, Update};
use bevy::camera::visibility::Visibility;
use bevy::ecs::change_detection::DetectChangesMut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::lifecycle::Remove;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};
use bevy::ecs::world::World;
use bevy::math::{Rect, Vec2};
use bevy::text::{
    ComputedTextBlock, InlineBox, InlineBoxKind, TextLayoutInfo,
};
use bevy::ui::widget::text_system;
use bevy::ui::{ComputedNode, Node, PositionType, px};
use parley::{Brush, Layout};

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
                        Node {
                            position_type: PositionType::Absolute,
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
}

impl AtomBox {
    pub fn new(atom: Atom) -> Self {
        Self {
            atom,
            slot: None,
            chip: Rect::default(),
        }
    }
}

/// How far the line holding the layout-space point `y` reaches under
/// its baseline.
fn below_baseline<B: Brush>(layout: &Layout<B>, y: f32) -> f32 {
    layout
        .lines()
        .map(|line| *line.metrics())
        .find(|line| {
            line.block_min_coord <= y && y < line.block_max_coord
        })
        .map_or(0.0, |line| line.block_max_coord - line.baseline)
}

/// The node an atom's view is built under.
#[derive(Component)]
struct Slot;

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(release)
        .add_systems(Update, build.in_set(FynixSystems::Settle))
        .add_systems(PostUpdate, place.after(text_system));
}

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
/// each view the place of its box. A box sized now is laid out in the
/// next frame.
///
/// A line puts the bottom of a box on its baseline, so the text would
/// sit on the bottom of its view. The view hangs below the box
/// instead, down to the bottom of the line, which makes the line as
/// high as the view and the text about as high as its middle.
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
        &ComputedTextBlock,
    )>,
    mut slots: Query<
        (&ComputedNode, &mut Node, &mut Visibility),
        With<Slot>,
    >,
) {
    for (entity, mut held, mut inline, parent) in &mut boxes {
        let Some(slot) = held.slot else {
            continue;
        };
        let Ok((layout, text, block)) = texts.get(parent.parent())
        else {
            continue;
        };
        let Ok((measured, mut node, mut visibility)) =
            slots.get_mut(slot)
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
        let below = placed.map_or(0.0, |rect| {
            below_baseline(block.buffer(), rect.center().y)
        }) * scale;
        if size.x > 0.0 && size.y > 0.0 {
            inline.set_if_neq(InlineBox {
                kind: InlineBoxKind::InFlow,
                size: Vec2::new(
                    size.x + 2.0 * GAP,
                    (size.y - below).max(f32::EPSILON),
                ),
            });
        }
        let Some(rect) = placed else {
            continue;
        };
        let gap = GAP / scale;
        let chip = Rect::new(
            rect.min.x + gap,
            rect.min.y,
            rect.max.x - gap,
            rect.max.y + below / scale,
        );
        if held.chip != chip {
            held.chip = chip;
        }
        let origin = chip.min * scale;
        let (left, top) = (px(origin.x), px(origin.y));
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
        visibility.set_if_neq(Visibility::Inherited);
    }
}
