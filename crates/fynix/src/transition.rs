//! How an element's written values travel to new ones.

use core::time::Duration;

use motiongfx_interp::ease::EaseFn;
use motiongfx_interp::interpolation::InterpFn;

/// A kind of movement, resolved to a [`Curve`] by the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// Answering the pointer: hover, press.
    Interact,
    /// Something opening or growing.
    Expand,
}

/// How long a value travels, and how its progress eases.
#[derive(Clone, Copy, Debug)]
pub struct Curve {
    pub duration: Duration,
    pub ease: EaseFn,
}

/// What a theme answers when asked how something moves.
pub trait MotionTokens {
    fn motion(&self, motion: Motion) -> Curve;
}

/// How the values of an element under a transition rule travel: over
/// `curve`, which the theme gave for `motion` when the rule named
/// one.
#[derive(Clone, Copy, Debug)]
pub struct Travel {
    pub curve: Curve,
    pub motion: Option<Motion>,
}

/// How a prop's value `S` travels: over `curve`, given for `motion`
/// when one was named, blended by `interp`.
pub struct Tween<S> {
    pub curve: Curve,
    pub motion: Option<Motion>,
    pub interp: InterpFn<S>,
}

impl<S> Clone for Tween<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for Tween<S> {}

/// One travel in progress, from where the value was when it started.
pub(crate) struct Run<S> {
    from: S,
    elapsed: Duration,
    tween: Tween<S>,
}

impl<S> Run<S> {
    pub(crate) fn new(from: S, tween: Tween<S>) -> Self {
        Self {
            from,
            elapsed: Duration::ZERO,
            tween,
        }
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
        self.elapsed += delta;
        let duration = self.tween.curve.duration;
        if self.elapsed >= duration {
            return None;
        }
        let progress =
            self.elapsed.as_secs_f32() / duration.as_secs_f32();
        let eased = (self.tween.curve.ease)(progress);
        Some((self.tween.interp)(&self.from, to, eased))
    }
}
