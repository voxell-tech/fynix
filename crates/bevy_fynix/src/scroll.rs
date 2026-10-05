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
use bevy::input::mouse::MouseScrollUnit;
use bevy::math::Vec2;
use bevy::picking::events::{Pointer, Scroll};
use bevy::time::Time;
use bevy::ui::{ComputedNode, Node, OverflowAxis, ScrollPosition};

use crate::transition::ReducedMotion;

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(bar::plugin)
        .init_resource::<ScrollMotion>()
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
/// notches.
pub fn wheel(scroll: &Pointer<Scroll>) -> (Vec2, bool) {
    let turned = Vec2::new(scroll.x, scroll.y);
    match scroll.unit {
        MouseScrollUnit::Line => (
            turned * MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
            true,
        ),
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
    mut scroll: On<Pointer<Scroll>>,
    mut areas: Query<(
        &Node,
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
    )>,
) {
    let Ok((node, computed, mut position, mut goal)) =
        areas.get_mut(scroll.entity)
    else {
        return;
    };
    scroll.propagate(false);
    let (turned, notched) = wheel(&scroll);
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
    goal.scroll_by(&mut position, delta, overflow(computed), notched);
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
