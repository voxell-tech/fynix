/// A pointer's full identity: which device it came from, and which
/// pointer on that device.
///
/// Backends keep their own id types; this crate stays generic over
/// them. Bundling both into one value lets each interaction type take
/// a single `Id` parameter, and lets the recognizer key per-pointer
/// state by the whole id so two devices (or two fingers) never
/// collide. A mouse-only backend uses `PointerId<DeviceId, ()>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerId<Device, Pointer> {
    /// The device the pointer belongs to.
    pub device: Device,
    /// The pointer within that device.
    pub pointer: Pointer,
}

/// Semantic role of a pressed button, classified once by the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRole {
    /// The primary button (left mouse, primary tap).
    Primary,
    /// The secondary button (right mouse, context).
    Secondary,
    /// The middle button.
    Middle,
    /// Any other button.
    Other,
}

/// Maps a backend's native button type to a [`ButtonRole`].
///
/// The backend supplies one of these to the [`Interactor`] so the
/// crate never hardcodes a button mapping. A plain function pointer is
/// enough; no trait needed.
///
/// [`Interactor`]: crate::pointer::interactor::Interactor
pub type ClassifyButton<Btn> = fn(Btn) -> ButtonRole;
