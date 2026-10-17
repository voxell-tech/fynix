//! Wheel scrolling that eases.
//!
//! A wheel turns in notches, each a jump of many pixels, so a notch
//! sets where the area is headed and it eases there. A trackpad
//! already reports small steps with momentum of its own, and those
//! are followed as they come.
//!
//! An area's bars are styled by [`ScrollbarStyle`].

mod bar;

use core::time::Duration;

pub use bar::{OwnScrollbar, ScrollbarStyle, ScrollbarVisibility};
use bevy::app::{App, Update};
use bevy::ecs::component::Component;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::input::mouse::{MouseScrollPixelsPerLine, MouseScrollUnit};
use bevy::math::Vec2;
use bevy::picking::events::PointerScroll;
use bevy::time::Time;
use bevy::ui::{ComputedNode, Node, OverflowAxis, ScrollPosition};

use crate::transition::ReducedMotion;

pub(crate) fn plugin(app: &mut App) {
    // Kept as the input plugin set it, if it is there first.
    app.add_plugins(bar::plugin)
        .init_resource::<ScrollMotion>()
        .init_resource::<MouseScrollPixelsPerLine>()
        .add_observer(on_scroll)
        .add_systems(Update, ease);
}

/// How fast a scroll area closes on where a wheel sent it.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ScrollMotion {
    /// The time it takes to close most of the way.
    pub follow: Duration,
}

impl Default for ScrollMotion {
    fn default() -> Self {
        Self {
            follow: Duration::from_millis(60),
        }
    }
}

/// How far a wheel event turned, in pixels, and whether it came in
/// notches, each `per_line` pixels.
pub fn wheel(
    scroll: &PointerScroll,
    per_line: MouseScrollPixelsPerLine,
) -> (Vec2, bool) {
    let turned = Vec2::new(scroll.x, scroll.y);
    match scroll.unit {
        MouseScrollUnit::Line => (turned * per_line, true),
        MouseScrollUnit::Pixel => (turned, false),
    }
}

/// A node the wheel scrolls, and where it is easing to, if anywhere.
#[derive(Component, Clone, Copy, Debug, Default)]
#[require(ScrollPosition)]
pub struct ScrollGoal(Option<Vec2>);

impl ScrollGoal {
    /// Scrolls on by `delta`, no further than `max`: eased from
    /// where it is already headed when `notched`, so quick notches
    /// add up, and at once otherwise.
    pub fn scroll_by(
        &mut self,
        position: &mut ScrollPosition,
        delta: Vec2,
        max: Vec2,
        notched: bool,
    ) {
        let from = self.0.unwrap_or(**position);
        let to = (from + delta).clamp(Vec2::ZERO, max);
        if notched {
            self.0 = Some(to);
        } else {
            self.0 = None;
            **position = to;
        }
    }
}

/// How far the content of a node overflows what it has to itself,
/// which leaves out any room kept for bars.
fn overflow(computed: &ComputedNode) -> Vec2 {
    let visible = computed.size() - computed.scrollbar_size;
    ((computed.content_size() - visible)
        * computed.inverse_scale_factor())
    .max(Vec2::ZERO)
}

fn on_scroll(
    mut scroll: On<PointerScroll>,
    mut areas: Query<(
        &Node,
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
    )>,
    per_line: Res<MouseScrollPixelsPerLine>,
) {
    let Ok((node, computed, mut position, mut goal)) =
        areas.get_mut(scroll.entity)
    else {
        return;
    };
    let (turned, notched) = wheel(&scroll, *per_line);
    let scrolls = |axis| axis == OverflowAxis::Scroll;
    let delta = Vec2::new(
        if scrolls(node.overflow.x) {
            -turned.x
        } else {
            0.0
        },
        if scrolls(node.overflow.y) {
            -turned.y
        } else {
            0.0
        },
    );
    // A wheel this area has no use for, turned the other way or
    // past its end, is left to an area around it.
    let before = (goal.0, **position);
    goal.scroll_by(&mut position, delta, overflow(computed), notched);
    let settled = goal.0.unwrap_or(**position);
    if settled != before.0.unwrap_or(before.1) {
        scroll.propagate(false);
    }
}

fn ease(
    time: Res<Time>,
    motion: Res<ScrollMotion>,
    reduced: Res<ReducedMotion>,
    mut areas: Query<(
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
    )>,
) {
    let follow = motion.follow.as_secs_f32();
    let share = if reduced.0 || follow <= 0.0 {
        1.0
    } else {
        1.0 - (-time.delta_secs() / follow).exp()
    };
    for (computed, mut position, mut goal) in &mut areas {
        let Some(to) = goal.0 else {
            continue;
        };
        // The content may have shrunk since the wheel turned.
        let to = to.min(overflow(computed));
        let left = to - **position;
        if left.abs().max_element() < 0.5 || share >= 1.0 {
            **position = to;
            goal.0 = None;
        } else {
            **position += left * share;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::camera::NormalizedRenderTarget;
    use bevy::ecs::entity::Entity;
    use bevy::input::touch::TouchPhase;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::Pointer;
    use bevy::picking::pointer::{Location, PointerId};

    use super::*;
    use crate::tests;

    #[test]
    fn a_wheel_turns_without_the_input_plugin() {
        let mut app = tests::app();
        let node = app.world_mut().spawn_empty().id();
        let location = Location {
            target: NormalizedRenderTarget::None {
                width: 100,
                height: 100,
            },
            position: Vec2::ZERO,
        };
        app.world_mut().trigger(PointerScroll {
            entity: node,
            pointer: Pointer::new(PointerId::Mouse, location),
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            phase: TouchPhase::Moved,
        });
        app.update();
    }
}
