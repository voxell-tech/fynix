pub use fynix_macros::Init;

/// Constructs a "blank" instance of a type before styles are applied.
///
/// Implement this (or derive it with `#[derive(Init)]`) alongside
/// `ElementBuild` and `ElementChildren` to satisfy the `ElementTemplate`
/// bound.
///
/// Unlike `Default`, `init` is the designated hook for per-field
/// initialization values declared via `#[init = expr]`.
pub trait Init {
    fn init() -> Self
    where
        Self: Sized;
}
