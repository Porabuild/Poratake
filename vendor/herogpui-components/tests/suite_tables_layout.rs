//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "full_width.rs"]
mod full_width;
#[path = "resizable.rs"]
mod resizable;
#[path = "scroll_shadow_deep.rs"]
mod scroll_shadow_deep;
#[path = "scrollbar_api.rs"]
mod scrollbar_api;
#[path = "sidebar.rs"]
mod sidebar;
#[path = "surface_deep.rs"]
mod surface_deep;
#[path = "table_and_drag.rs"]
mod table_and_drag;
#[path = "table_deep.rs"]
mod table_deep;
#[path = "table_extras.rs"]
mod table_extras;
#[path = "table_tabs_accordion.rs"]
mod table_tabs_accordion;
#[path = "title_bar.rs"]
mod title_bar;
#[path = "toolbar_divider_deep.rs"]
mod toolbar_divider_deep;
#[path = "toolbar_extras.rs"]
mod toolbar_extras;
#[path = "typography_deep.rs"]
mod typography_deep;
