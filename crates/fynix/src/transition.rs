//! How an element's written values travel to new ones, on a spring
//! or an ease curve. Either way, a travel cut short turns around
//! without a jump: a spring keeps its speed, and curves are laid over
//! each other.

use alloc::vec::Vec;
use core::f32::consts::TAU;
use core::time::Duration;

use motiongfx_interp::ease::EaseFn;

/// A kind of movement, resolved to a [`Timing`] by the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// Answering the pointer: hover, press.
    Interact,
    /// Something opening or growing.
    Expand,
}

/// How a value travels to its target.
#[derive(Clone, Copy, Debug)]
pub enum Timing {
    Spring(Spring),
    Ease(Curve),
}

impl Timing {
    /// Arrives at once.
    pub const INSTANT: Self = Self::Spring(Spring::INSTANT);

    /// About how long a travel takes, exactly so for a curve.
    pub fn duration(&self) -> Duration {
        match self {
            Self::Spring(spring) => spring.duration,
            Self::Ease(curve) => curve.duration,
        }
    }

    /// Whether it arrives at once.
    pub fn is_instant(&self) -> bool {
        self.duration().is_zero()
    }
}

impl From<Spring> for Timing {
    fn from(spring: Spring) -> Self {
        Self::Spring(spring)
    }
}

impl From<Curve> for Timing {
    fn from(curve: Curve) -> Self {
        Self::Ease(curve)
    }
}

/// A value pulled toward its target and slowed, so when the target
/// changes on the way it keeps its speed and turns around. In
/// SwiftUI's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// About how long it takes to arrive. Zero arrives at once.
    pub duration: Duration,
    /// How far it swings past the target, between -1 and 1: 0
    /// arrives without passing it, above 0 swings past and back,
    /// below 0 arrives more slowly still.
    pub bounce: f32,
}

impl Spring {
    /// Arrives at once.
    pub const INSTANT: Self = Self::new(Duration::ZERO, 0.0);

    pub const fn new(duration: Duration, bounce: f32) -> Self {
        Self { duration, bounce }
    }

    /// Its angular frequency and damping ratio. Without
    /// `can_overshoot` the ratio is held at 1 or above, so the value
    /// never passes its target.
    fn coefficients(self, can_overshoot: bool) -> (f32, f32) {
        let omega = TAU / self.duration.as_secs_f32();
        // Past 1 it would never settle, and at -1 never arrive.
        let bounce = self.bounce.clamp(-0.95, 0.95);
        let bounce = if can_overshoot {
            bounce
        } else {
            bounce.min(0.0)
        };
        let zeta = if bounce >= 0.0 {
            1.0 - bounce
        } else {
            1.0 / (1.0 + bounce)
        };
        (omega, zeta)
    }
}

/// A travel of a fixed length, its progress eased.
#[derive(Clone, Copy, Debug)]
pub struct Curve {
    pub duration: Duration,
    pub ease: EaseFn,
}

/// What a theme answers when asked how something moves.
pub trait MotionTokens {
    fn motion(&self, motion: Motion) -> Timing;
}

/// How the values of an element under a transition rule travel: over
/// `timing`, which the theme gave for `motion` when the rule named
/// one.
#[derive(Clone, Copy, Debug)]
pub struct Travel {
    pub timing: Timing,
    pub motion: Option<Motion>,
}

/// How many numbers a value can be split into to travel.
pub const AXES: usize = 4;

/// How a prop's value `S` travels: split into up to [`AXES`] numbers
/// that blend linearly, each travelling on its own, and joined back.
/// A colour splits into Oklab's lightness, a, b and alpha.
pub struct Axes<S> {
    pub split: fn(&S) -> [f32; AXES],
    pub join: fn([f32; AXES]) -> S,
    /// Whether the value may swing past its target when a spring
    /// bounces. A colour or an opacity should not.
    pub can_overshoot: bool,
}

impl<S> Clone for Axes<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Axes<S> {}

/// An `f32` that may swing past its target: a size, a scale, an
/// angle.
pub const F32_AXES: Axes<f32> = Axes {
    split: |value| [*value, 0.0, 0.0, 0.0],
    join: |axes| axes[0],
    can_overshoot: true,
};

/// An `f32` that never passes its target: an opacity.
pub const OPACITY_AXES: Axes<f32> = Axes {
    can_overshoot: false,
    ..F32_AXES
};

/// How a prop's value `S` travels: over `timing`, given for `motion`
/// when one was named, along `axes`.
pub struct Tween<S> {
    pub timing: Timing,
    pub motion: Option<Motion>,
    pub axes: Axes<S>,
}

impl<S> Clone for Tween<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Tween<S> {}

type Point = [f32; AXES];

/// Closer than this to its target, and slower than this per radian
/// of the spring, a number on a spring has arrived. Too small to see
/// in pixels, in a fraction of a whole, or in an Oklab channel.
const REST: f32 = 1e-3;

/// Numbers travelling to a target.
pub(crate) struct Glide {
    timing: Timing,
    can_overshoot: bool,
    /// The target the last step headed to.
    target: Point,
    state: State,
}

enum State {
    /// Each axis where it is and how fast it moves, per second.
    Spring { at: Point, speed: Point },
    /// Weakest first. The value is the target, plus what is left of
    /// each layer's offset.
    Ease(Vec<Layer>),
}

/// One ease curve's travel: from `offset` away from the target it
/// was laid down for, to no offset.
struct Layer {
    offset: Point,
    elapsed: Duration,
    curve: Curve,
}

impl Layer {
    /// How much of its offset is left.
    fn left(&self) -> f32 {
        let duration = self.curve.duration.as_secs_f32();
        let progress = self.elapsed.as_secs_f32() / duration;
        1.0 - (self.curve.ease)(progress.min(1.0))
    }
}

impl Glide {
    /// A travel from `from`, at rest.
    pub(crate) fn new(
        from: Point,
        timing: Timing,
        can_overshoot: bool,
    ) -> Self {
        Self {
            timing,
            can_overshoot,
            target: from,
            state: State::at_rest(from, timing),
        }
    }

    /// Takes `timing` for the travels from here on. From a spring to
    /// curves or back, it starts again at rest from where it is.
    pub(crate) fn retime(&mut self, timing: Timing) {
        let same_kind = matches!(
            (&self.state, timing),
            (State::Spring { .. }, Timing::Spring(_))
                | (State::Ease(_), Timing::Ease(_))
        );
        if !same_kind {
            let value = self.value();
            self.state = State::at_rest(value, timing);
            self.target = value;
        }
        self.timing = timing;
    }

    /// Where it is now.
    pub(crate) fn value(&self) -> Point {
        match &self.state {
            State::Spring { at, .. } => *at,
            State::Ease(layers) => {
                let mut value = self.target;
                for layer in layers {
                    let left = layer.left();
                    for (axis, offset) in
                        value.iter_mut().zip(layer.offset)
                    {
                        *axis += offset * left;
                    }
                }
                value
            }
        }
    }

    /// Moves on by `delta` toward `target`. Whether it has arrived,
    /// in which case it is on the target, at rest.
    pub(crate) fn advance(
        &mut self,
        delta: Duration,
        target: Point,
    ) -> bool {
        let last = core::mem::replace(&mut self.target, target);
        if self.timing.is_instant() {
            self.state = State::at_rest(target, self.timing);
            return true;
        }
        match (&mut self.state, self.timing) {
            (State::Spring { at, speed }, Timing::Spring(spring)) => {
                let (omega, zeta) =
                    spring.coefficients(self.can_overshoot);
                let t = delta.as_secs_f32();
                let mut arrived = true;
                for i in 0..AXES {
                    let (offset, v) = spring_step(
                        at[i] - target[i],
                        speed[i],
                        t,
                        omega,
                        zeta,
                    );
                    let resting = offset.abs() <= REST
                        && v.abs() <= REST * omega;
                    arrived &= resting;
                    (at[i], speed[i]) = if resting {
                        (target[i], 0.0)
                    } else {
                        (target[i] + offset, v)
                    };
                }
                arrived
            }
            (State::Ease(layers), Timing::Ease(curve)) => {
                if last != target {
                    let mut offset = last;
                    for (axis, target) in
                        offset.iter_mut().zip(target)
                    {
                        *axis -= target;
                    }
                    layers.push(Layer {
                        offset,
                        elapsed: Duration::ZERO,
                        curve,
                    });
                }
                for layer in layers.iter_mut() {
                    layer.elapsed += delta;
                }
                layers.retain(|layer| {
                    layer.elapsed < layer.curve.duration
                });
                layers.is_empty()
            }
            _ => unreachable!("retime keeps the state's kind"),
        }
    }
}

impl State {
    fn at_rest(at: Point, timing: Timing) -> Self {
        match timing {
            Timing::Spring(_) => Self::Spring {
                at,
                speed: [0.0; AXES],
            },
            Timing::Ease(_) => Self::Ease(Vec::new()),
        }
    }
}

/// How far from its target, and how fast, a damped spring is `t`
/// seconds after it was `offset` away moving at `speed`. Exact for
/// any `t`, so a late frame lands where an on-time one would have.
fn spring_step(
    offset: f32,
    speed: f32,
    t: f32,
    omega: f32,
    zeta: f32,
) -> (f32, f32) {
    if (zeta - 1.0).abs() < 1e-4 {
        let decay = libm::expf(-omega * t);
        let b = speed + omega * offset;
        return (
            decay * (offset + b * t),
            decay * (speed - omega * b * t),
        );
    }
    if zeta < 1.0 {
        let damped = omega * libm::sqrtf(1.0 - zeta * zeta);
        let decay = libm::expf(-zeta * omega * t);
        let (sin, cos) = libm::sincosf(damped * t);
        let b = (speed + zeta * omega * offset) / damped;
        let db =
            (zeta * omega * speed + omega * omega * offset) / damped;
        return (
            decay * (offset * cos + b * sin),
            decay * (speed * cos - db * sin),
        );
    }
    let root = libm::sqrtf(zeta * zeta - 1.0);
    let slow = -omega * (zeta - root);
    let fast = -omega * (zeta + root);
    let c_fast = (speed - slow * offset) / (fast - slow);
    let c_slow = offset - c_fast;
    let (e_slow, e_fast) =
        (libm::expf(slow * t), libm::expf(fast * t));
    (
        c_slow * e_slow + c_fast * e_fast,
        slow * c_slow * e_slow + fast * c_fast * e_fast,
    )
}

/// One prop's travel in progress.
pub(crate) struct Run<S> {
    glide: Glide,
    tween: Tween<S>,
}

impl<S> Run<S> {
    /// A travel from `from`, at rest.
    pub(crate) fn new(from: &S, tween: Tween<S>) -> Self {
        Self {
            glide: Glide::new(
                (tween.axes.split)(from),
                tween.timing,
                tween.axes.can_overshoot,
            ),
            tween,
        }
    }

    /// Takes `tween` from here on, keeping where the value is and how
    /// it moves.
    pub(crate) fn retarget(&mut self, tween: Tween<S>) {
        self.glide.retime(tween.timing);
        self.tween = tween;
    }

    /// The motion it was given for, if one was named.
    pub(crate) fn motion(&self) -> Option<Motion> {
        self.tween.motion
    }

    /// Where the value is `delta` later, heading to `to`. `None` once
    /// it has arrived.
    pub(crate) fn advance(
        &mut self,
        delta: Duration,
        to: &S,
    ) -> Option<S> {
        let arrived =
            self.glide.advance(delta, (self.tween.axes.split)(to));
        (!arrived).then(|| (self.tween.axes.join)(self.glide.value()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPRINGS: [(f32, f32); 3] =
        [(20.0, 0.4), (20.0, 1.0), (20.0, 2.5)];

    #[test]
    fn two_spring_steps_land_where_one_of_their_sum_does() {
        for (omega, zeta) in SPRINGS {
            let (o, s) = spring_step(1.0, -3.0, 0.03, omega, zeta);
            let (o, s) = spring_step(o, s, 0.05, omega, zeta);
            let (one_o, one_s) =
                spring_step(1.0, -3.0, 0.08, omega, zeta);
            assert!((o - one_o).abs() < 1e-5, "{zeta}: {o} {one_o}");
            assert!((s - one_s).abs() < 1e-4, "{zeta}: {s} {one_s}");
        }
    }

    #[test]
    fn a_spring_step_keeps_the_speed_it_was_given() {
        // Over a tiny step, the offset moves by the speed.
        for (omega, zeta) in SPRINGS {
            let (o, _) = spring_step(0.5, 4.0, 1e-4, omega, zeta);
            assert!((o - (0.5 + 4.0 * 1e-4)).abs() < 1e-5, "{zeta}");
        }
    }

    fn one(value: f32) -> Point {
        [value, 0.0, 0.0, 0.0]
    }

    #[test]
    fn a_spring_that_cannot_overshoot_never_passes_its_target() {
        let spring = Spring::new(Duration::from_millis(200), 0.8);
        let mut glide = Glide::new(one(0.0), spring.into(), false);
        let tick = Duration::from_millis(4);
        while !glide.advance(tick, one(1.0)) {
            assert!(glide.value()[0] <= 1.0, "{:?}", glide.value());
        }
        assert_eq!(glide.value(), one(1.0));
    }

    #[test]
    fn a_curve_cut_short_carries_on_under_the_new_one() {
        let curve = Curve {
            duration: Duration::from_secs(1),
            ease: |t| t,
        };
        let mut glide = Glide::new(one(0.0), curve.into(), true);
        let half = Duration::from_millis(500);
        glide.advance(Duration::ZERO, one(100.0));
        glide.advance(half, one(100.0));
        assert_eq!(glide.value(), one(50.0));

        // The rise left and the fall added cancel out while both run.
        glide.advance(Duration::ZERO, one(0.0));
        assert_eq!(glide.value(), one(50.0), "no jump");
        glide.advance(Duration::from_millis(250), one(0.0));
        assert_eq!(glide.value(), one(50.0));
        assert!(!glide.advance(half, one(0.0)));
        assert_eq!(glide.value(), one(25.0));
        assert!(glide.advance(half, one(0.0)), "a curve after it");
        assert_eq!(glide.value(), one(0.0));
    }
}
