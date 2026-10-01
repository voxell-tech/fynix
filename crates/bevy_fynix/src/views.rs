//! The views this crate ships.

mod behavior;
mod button;
mod checkbox;
mod divider;
mod field_row;
mod foldable;
mod frame;
mod icon;
mod label;
mod segmented;
mod stack;

pub use behavior::{BehaviorExt, OnActivate, Tagged, Toned};
pub use button::{Button, button, ghost, menu_bar, segment, tint};
pub(crate) use checkbox::sync_checked;
pub use checkbox::{Checkbox, checkbox};
pub use divider::{Axis, Divider, divider};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameProps, frame};
pub use icon::{Icon, IconProps, icon};
pub use label::{Label, LabelProps, label};
pub(crate) use segmented::sync_segments;
pub use segmented::{Segmented, segmented};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
