//! The views this crate ships.

mod behavior;
mod button;
mod checkbox;
mod divider;
mod field;
mod field_row;
mod foldable;
mod frame;
mod icon;
mod label;
mod number_field;
mod segmented;
mod stack;
mod text_input;

pub use behavior::{BehaviorExt, OnActivate, Tagged, Toned};
pub use button::{Button, button, ghost, menu_bar, segment, tint};
pub(crate) use checkbox::sync_checked;
pub use checkbox::{Checkbox, checkbox};
pub use divider::{Axis, Divider, divider};
pub use field::{TextField, text_field};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameProps, frame};
pub use icon::{Icon, IconProps, icon};
pub use label::{Label, LabelProps, label};
pub use number_field::{NumberField, number_field};
pub(crate) use segmented::sync_segments;
pub use segmented::{Segmented, segmented};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
pub(crate) use text_input::sync_text_inputs;
pub use text_input::{TextInput, TextInputProps};
