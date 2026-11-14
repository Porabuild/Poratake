//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "calendars_and_more.rs"]
mod calendars_and_more;
#[path = "calendars_deep.rs"]
mod calendars_deep;
#[path = "color_field_blur.rs"]
mod color_field_blur;
#[path = "color_geometry_deep.rs"]
mod color_geometry_deep;
#[path = "date_field_form_deep.rs"]
mod date_field_form_deep;
#[path = "date_field_picker_deep.rs"]
mod date_field_picker_deep;
#[path = "date_picker_close.rs"]
mod date_picker_close;
#[path = "date_picker_placement.rs"]
mod date_picker_placement;
#[path = "pickers.rs"]
mod pickers;
#[path = "pickers_deep.rs"]
mod pickers_deep;
#[path = "slider_form_deep.rs"]
mod slider_form_deep;
#[path = "slider_geometry_deep.rs"]
mod slider_geometry_deep;
#[path = "slider_number_deep.rs"]
mod slider_number_deep;

#[path = "slider_indicators.rs"]
mod slider_indicators;
