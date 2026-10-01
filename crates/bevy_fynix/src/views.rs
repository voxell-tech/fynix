//! The views this crate ships.

mod behavior;
mod button;
mod checkbox;
mod context_menu;
mod divider;
mod dropdown;
mod field;
mod field_row;
mod foldable;
mod frame;
mod icon;
mod label;
mod menu;
mod number_field;
mod segmented;
mod stack;
mod text_input;
mod tooltip;

pub use behavior::{BehaviorExt, OnActivate, Seeded, Tagged, Toned};
pub use button::{
    Button, button, danger, ghost, icon_button, menu_bar, primary,
    segment, tint,
};
pub use checkbox::{Checkbox, checkbox};
pub(crate) use context_menu::dismiss as dismiss_context_menu;
pub use context_menu::{ContextMenu, ContextMenuExt};
pub use divider::{Axis, Divider, divider};
pub(crate) use dropdown::on_menu_event as toggle_dropdown;
pub use dropdown::{Dropdown, dropdown};
pub use field::{TextField, text_field};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameProps, frame};
pub use icon::{Icon, IconProps, icon};
pub use label::{Label, LabelProps, label};
pub use menu::{
    MENU_Z, MenuItem, TOOLTIP_Z, menu_item, menu_surface,
};
pub(crate) use menu::{despawn_orphans, focus_first};
pub use number_field::{Number, NumberField, number_field};
pub use segmented::{Segmented, segmented};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
pub(crate) use text_input::sync_text_inputs;
pub use text_input::{TextInput, TextInputProps};
pub(crate) use tooltip::tick as tick_tooltips;
pub use tooltip::{Tooltip, TooltipExt, TooltipTiming};
