//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;

#[path = "command_palette.rs"]
mod command_palette;
#[path = "context_menu.rs"]
mod context_menu;
#[path = "drawer_deep.rs"]
mod drawer_deep;
#[path = "dropdown_anatomy_deep.rs"]
mod dropdown_anatomy_deep;
#[path = "dropdown_close.rs"]
mod dropdown_close;
#[path = "dropdown_seed_deep.rs"]
mod dropdown_seed_deep;
#[path = "dropdown_viewport_deep.rs"]
mod dropdown_viewport_deep;
#[path = "hover_card.rs"]
mod hover_card;
#[path = "menu_bar.rs"]
mod menu_bar;
#[path = "overlay_padding.rs"]
mod overlay_padding;
#[path = "overlay_stack_color_deep.rs"]
mod overlay_stack_color_deep;
#[path = "overlay_stack_date_deep.rs"]
mod overlay_stack_date_deep;
#[path = "overlay_stack_dialogs_deep.rs"]
mod overlay_stack_dialogs_deep;
#[path = "overlay_stack_menus_deep.rs"]
mod overlay_stack_menus_deep;
#[path = "overlay_stack_pickers_deep.rs"]
mod overlay_stack_pickers_deep;
#[path = "overlays.rs"]
mod overlays;
#[path = "panel_padding.rs"]
mod panel_padding;
#[path = "placement.rs"]
mod placement;
#[path = "placement_extra.rs"]
mod placement_extra;
#[path = "popover_stack_deep.rs"]
mod popover_stack_deep;

#[path = "sheet_deep.rs"]
mod sheet_deep;
