//! The views this crate ships.

mod behavior;
mod button;
mod divider;
mod field_row;
mod foldable;
mod frame;
mod icon;
mod label;
mod stack;

pub use behavior::{BehaviorExt, OnActivate, Tagged, Toned};
pub use button::{Button, button};
pub use divider::{Axis, Divider, divider};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameSnapshot, frame};
pub use icon::{Icon, IconSnapshot, icon};
pub use label::{Label, LabelSnapshot, label};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
