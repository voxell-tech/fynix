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
use bevy::math::{BVec2, Vec2};
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
pub struct ScrollGoal(Option<Heading>);

/// Where a notched wheel is easing to. An axis the wheel sent to
/// the end stays headed for the end as the content grows, so a
/// wheel flung to the bottom of a list that is still filling lands
/// at the bottom and not where the bottom was.
#[derive(Clone, Copy, Debug)]
struct Heading {
    to: Vec2,
    to_end: BVec2,
}

impl Heading {
    /// Where the ease measures what is left from, and where it is
    /// headed now that the content overflows by `overflow`. An axis
    /// headed for its end measures from the end as it was and heads
    /// for the end as it is, so content that grows on the way never
    /// widens the gap.
    fn ends_for(&self, overflow: Vec2) -> (Vec2, Vec2) {
        let from =
            Vec2::select(self.to_end, self.to, self.to.min(overflow));
        (from, Vec2::select(self.to_end, overflow, from))
    }
}

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
        let to =
            (self.settled(position) + delta).clamp(Vec2::ZERO, max);
        if notched {
            let was = self.0.map_or(BVec2::FALSE, |goal| goal.to_end);
            let reaches = delta.cmpgt(Vec2::ZERO) & to.cmpge(max);
            self.0 = Some(Heading {
                to,
                to_end: reaches | (was & delta.cmpeq(Vec2::ZERO)),
            });
        } else {
            self.0 = None;
            **position = to;
        }
    }

    /// Scrolls by `delta` at once, and moves where an ease in flight
    /// is headed by as much, so content that shifted under the view
    /// is still eased to the place the wheel sent it. A view that
    /// moves its content to keep a row still calls this instead of
    /// setting the position.
    pub(crate) fn shift(
        &mut self,
        position: &mut ScrollPosition,
        delta: Vec2,
    ) {
        **position += delta;
        if let Some(goal) = &mut self.0 {
            goal.to += delta;
        }
    }

    /// Where the area ends up: where it is headed, or where it is.
    fn settled(&self, position: &ScrollPosition) -> Vec2 {
        self.0.map_or(**position, |goal| goal.to)
    }
}

/// How far the content of a node overflows what it has to itself,
/// which leaves out any room kept for bars.
pub(crate) fn overflow(computed: &ComputedNode) -> Vec2 {
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
    let before = goal.settled(&position);
    goal.scroll_by(&mut position, delta, overflow(computed), notched);
    if goal.settled(&position) != before {
        scroll.propagate(false);
    }
}

pub(crate) fn ease(
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
        let Some(heading) = &mut goal.0 else {
            continue;
        };
        let (from, to) = heading.ends_for(overflow(computed));
        let left = from - **position;
        if left.abs().max_element() < 0.5 || share >= 1.0 {
            **position = to;
            goal.0 = None;
        } else {
            **position += to - from + left * share;
            heading.to = Vec2::select(heading.to_end, to, heading.to);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::entity::Entity;

    use super::*;
    use crate::tests;

    /// A wheel turned `by` in notches over content 1,000 px tall in
    /// an area 100 px square.
    fn flung(by: f32) -> (ScrollPosition, ScrollGoal) {
        let mut position = ScrollPosition::default();
        let mut goal = ScrollGoal::default();
        goal.scroll_by(
            &mut position,
            Vec2::Y * by,
            Vec2::Y * 900.0,
            true,
        );
        (position, goal)
    }

    fn area(
        app: &mut App,
        (position, goal): (ScrollPosition, ScrollGoal),
    ) -> Entity {
        let computed = ComputedNode {
            size: Vec2::splat(100.0),
            content_size: Vec2::new(100.0, 1_000.0),
            ..ComputedNode::default()
        };
        app.world_mut()
            .spawn((Node::default(), computed, position, goal))
            .id()
    }

    fn grow(app: &mut App, area: Entity, by: f32) {
        app.world_mut()
            .get_mut::<ComputedNode>(area)
            .unwrap()
            .content_size
            .y += by;
    }

    fn at(app: &App, area: Entity) -> f32 {
        app.world().get::<ScrollPosition>(area).unwrap().y
    }

    #[test]
    fn an_ease_lands_as_far_on_as_its_content_shifted() {
        let mut app = tests::app();
        let (mut position, mut goal) = flung(100.0);
        goal.shift(&mut position, Vec2::Y * 40.0);
        let area = area(&mut app, (position, goal));

        for _ in 0..60 {
            app.update();
        }
        assert_eq!(at(&app, area), 140.0);
    }

    #[test]
    fn an_ease_short_of_the_end_lands_where_the_wheel_sent_it() {
        let mut app = tests::app();
        let still = area(&mut app, flung(100.0));
        let growing = area(&mut app, flung(100.0));

        for frame in 1..=60 {
            grow(&mut app, growing, 50.0);
            app.update();
            assert_eq!(
                at(&app, growing),
                at(&app, still),
                "frame {frame}"
            );
        }
        assert_eq!(at(&app, growing), 100.0);
    }

    #[test]
    fn a_fling_to_the_end_reaches_the_end_of_content_that_grows() {
        let mut app = tests::app();
        let area = area(&mut app, flung(2_000.0));

        for _ in 0..5 {
            grow(&mut app, area, 50.0);
            app.update();
        }
        for _ in 0..60 {
            app.update();
        }
        assert_eq!(at(&app, area), 900.0 + 5.0 * 50.0);
    }

    #[test]
    fn a_wheel_turns_without_the_input_plugin() {
        let mut app = tests::app();
        let node = app.world_mut().spawn_empty().id();
        tests::wheel(&mut app, node, 1.0);
        app.update();
    }
}
