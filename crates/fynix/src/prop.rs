//! A value a view is handed.

use alloc::boxed::Box;
use alloc::string::{String, ToString};

/// A prop: set at the call site, bound to the world `W`, or left for
/// set rules and then the theme to decide.
#[derive(Default)]
pub enum Prop<W, T> {
    #[default]
    Unset,
    Value(T),
    Bound(Signal<W, T>),
}

/// A value read from the world `W`, and a check for whether it may
/// have changed since the check last ran.
pub struct Signal<W, T> {
    read: Box<dyn Fn(&W) -> T + Send + Sync>,
    changed: Box<dyn FnMut(&W) -> bool + Send + Sync>,
}

impl<W, T> Signal<W, T> {
    /// A signal reading with `read`, whose `changed` says whether that
    /// read may differ from the one before.
    pub fn new(
        read: impl Fn(&W) -> T + Send + Sync + 'static,
        changed: impl FnMut(&W) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            read: Box::new(read),
            changed: Box::new(changed),
        }
    }

    /// What the world holds now.
    pub fn get(&self, world: &W) -> T {
        (self.read)(world)
    }

    /// Whether what this reads may have changed since the last call.
    pub fn changed(&mut self, world: &W) -> bool {
        (self.changed)(world)
    }
}

/// A read from the world `W` still waiting for its change check.
pub struct Derived<W, T> {
    read: Box<dyn Fn(&W) -> T + Send + Sync>,
}

impl<W, T> Derived<W, T> {
    /// The [`Signal`] of this read, re-read when `changed` says so.
    pub fn when(
        self,
        changed: impl FnMut(&W) -> bool + Send + Sync + 'static,
    ) -> Signal<W, T> {
        Signal {
            read: self.read,
            changed: Box::new(changed),
        }
    }
}

/// A read of whatever `read` returns, to be given its change check.
pub fn derived<W, T>(
    read: impl Fn(&W) -> T + Send + Sync + 'static,
) -> Derived<W, T> {
    Derived {
        read: Box::new(read),
    }
}

impl<W, T> Prop<W, T> {
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    pub fn is_bound(&self) -> bool {
        matches!(self, Self::Bound(_))
    }

    /// This, or `below` when this was left unset.
    pub fn or(self, below: Self) -> Self {
        match self {
            Self::Unset => below,
            set => set,
        }
    }

    /// What this holds now. `None` when unset.
    pub fn get(&self, world: &W) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Self::Unset => None,
            Self::Value(value) => Some(value.clone()),
            Self::Bound(signal) => Some(signal.get(world)),
        }
    }

    /// Whether what this holds may have changed since the last call.
    /// Only a bound prop can.
    pub fn changed(&mut self, world: &W) -> bool {
        match self {
            Self::Bound(signal) => signal.changed(world),
            Self::Unset | Self::Value(_) => false,
        }
    }
}

impl<W, T> From<T> for Prop<W, T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<W, T> From<Signal<W, T>> for Prop<W, T> {
    fn from(signal: Signal<W, T>) -> Self {
        Self::Bound(signal)
    }
}

impl<W> From<&str> for Prop<W, String> {
    fn from(text: &str) -> Self {
        Self::Value(text.to_string())
    }
}
