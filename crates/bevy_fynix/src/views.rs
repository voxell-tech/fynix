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
mod popup;
mod segmented;
mod stack;
mod text_input;
mod tooltip;

pub use behavior::{BehaviorExt, OnActivate, Seeded, Tagged, Toned};
pub use button::{
    Button, TintedIcon, button, danger, ghost, icon_button, menu_bar,
    primary, segment, tint, tint_to, tinted_icon,
};
pub use checkbox::{Checkbox, checkbox};
pub(crate) use context_menu::dismiss as dismiss_context_menu;
pub use context_menu::{ContextMenu, ContextMenuExt};
pub use divider::{Axis, Divider, divider, revealed};
pub use dropdown::{
    Dropdown, MenuEntry, MenuTitle, dropdown, menu_button,
};
pub(crate) use dropdown::{
    ListWidth, Parts, list, on_menu_event as toggle_dropdown, shut,
};
pub use field::{TextField, text_field};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameProps, frame};
pub use icon::{Icon, IconProps, icon};
pub use label::{Label, LabelProps, label};
pub use menu::{
    MENU_Z, MenuItem, Submenu, TOOLTIP_Z, menu_item, menu_surface,
    submenu,
};
pub(crate) use menu::{
    close_on_escape, close_on_outside_press, despawn_orphans,
    focus_first,
};
pub use number_field::{Number, NumberField, number_field};
pub use popup::{Popup, corners, popup};
pub use segmented::{Segmented, segmented};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
pub(crate) use text_input::sync_text_inputs;
pub use text_input::{TextInput, TextInputProps};
pub(crate) use tooltip::tick as tick_tooltips;
pub use tooltip::{Tooltip, TooltipExt, TooltipTiming};
