//! The views this crate ships.

mod behavior;
mod button;
mod context_menu;
mod divider;
mod dropdown;
mod field_row;
mod foldable;
mod frame;
mod icon;
mod label;
mod menu;
mod stack;
mod tooltip;

pub use behavior::{BehaviorExt, OnActivate, Tagged, Toned};
pub use button::{Button, button};
pub(crate) use context_menu::dismiss as dismiss_context_menu;
pub use context_menu::{ContextMenu, ContextMenuExt};
pub use divider::{Axis, Divider, divider};
pub(crate) use dropdown::on_menu_event as toggle_dropdown;
pub use dropdown::{Dropdown, dropdown};
pub use field_row::{AnimatedField, FieldRow, HasAction, field_row};
pub use foldable::{Foldable, Open, foldable};
pub use frame::{Frame, FrameProps, frame};
pub use icon::{Icon, IconProps, icon};
pub use label::{Label, LabelProps, label};
pub use menu::{
    MENU_Z, MenuItem, TOOLTIP_Z, menu_item, menu_surface,
};
pub(crate) use menu::{despawn_orphans, focus_first};
pub use stack::{Extra, Stack, column, overlay, row, scroll};
pub(crate) use tooltip::tick as tick_tooltips;
pub use tooltip::{Tooltip, TooltipExt, TooltipTiming};
