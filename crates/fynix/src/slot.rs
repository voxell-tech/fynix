//! One prop of a mounted element: what is written on the node, where
//! it is heading, and how the backend writes it.

use crate::backend::Backend;
use crate::mounted::Tick;
use crate::transition::{Motion, Run, Tween};

/// How one prop of an element is written onto its node. A type with
/// no value, named by the prop's `#[lenz(tag = ..)]`, so anything
/// that names the prop by its path knows how to write it.
pub trait Patch<B: Backend, P> {
    fn patch(world: &mut B::World, node: B::Node, value: &P);

    /// How a backend that [animates](Backend::ANIMATES) writes
    /// `value`: moving to it over the theme's curve for `motion`, or
    /// at once when there is none. At once by default.
    fn patch_over(
        world: &mut B::World,
        node: B::Node,
        value: &P,
        motion: Option<Motion>,
    ) {
        let _ = motion;
        Self::patch(world, node, value);
    }
}

/// What `#[elem(with = ..)]` runs: `make(value, theme)`. A function
/// so a closure written in the attribute learns its argument types.
pub fn with_theme<P, T, Q>(
    make: impl FnOnce(P, &T) -> Q,
    value: P,
    theme: &T,
) -> Q {
    make(value, theme)
}

/// One prop's value as written, as last asked for, and its travel
/// between the two.
pub struct Slot<P> {
    shown: Option<P>,
    target: Option<P>,
    run: Option<Run<P>>,
}

impl<P> Default for Slot<P> {
    fn default() -> Self {
        Self {
            shown: None,
            target: None,
            run: None,
        }
    }
}

impl<P: Clone + PartialEq> Slot<P> {
    /// What is written on the node. `None` before the first write.
    pub fn shown(&self) -> Option<&P> {
        self.shown.as_ref()
    }

    /// Heads for `target`: over `tween` from what is written now, or
    /// at once when there is no tween, nothing written yet, or motion
    /// is reduced.
    pub fn aim(
        &mut self,
        target: P,
        tween: Option<Tween<P>>,
        tick: Tick,
    ) {
        if self.target.as_ref() == Some(&target) {
            return;
        }
        self.run = match (&self.shown, tween) {
            (Some(shown), Some(tween))
                if !tick.reduced_motion
                    && !tween.curve.duration.is_zero() =>
            {
                Some(Run::new(shown.clone(), tween))
            }
            _ => None,
        };
        self.target = Some(target);
    }

    /// Moves on by `tick`, and calls `write` with the value to show
    /// if it differs from what is written. Returns whether it is
    /// still travelling.
    pub fn step(
        &mut self,
        tick: Tick,
        write: impl FnOnce(&P),
    ) -> bool {
        let Some(target) = &self.target else {
            return false;
        };
        if tick.reduced_motion {
            self.run = None;
        }
        let next = match &mut self.run {
            Some(run) => match run.advance(tick.delta, target) {
                Some(next) => next,
                None => {
                    self.run = None;
                    target.clone()
                }
            },
            None => target.clone(),
        };
        if self.shown.as_ref() != Some(&next) {
            write(&next);
            self.shown = Some(next);
        }
        self.run.is_some()
    }

    /// As [`step`](Self::step), for a backend that
    /// [animates](Backend::ANIMATES): a travel over a motion is not
    /// stepped, its target is written at once with the motion, for
    /// the backend to move to. `write` is given no motion otherwise.
    pub fn hand_over(
        &mut self,
        tick: Tick,
        write: impl FnOnce(&P, Option<Motion>),
    ) -> bool {
        let motion = self
            .run
            .as_ref()
            .and_then(Run::motion)
            .filter(|_| !tick.reduced_motion);
        if motion.is_some() {
            self.run = None;
        }
        self.step(tick, |value| write(value, motion))
    }
}
