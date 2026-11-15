//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "autocomplete_hover_deep.rs"]
mod autocomplete_hover_deep;
#[path = "combo_box_open.rs"]
mod combo_box_open;
#[path = "combo_box_selection.rs"]
mod combo_box_selection;
#[path = "field_keyboard_contracts.rs"]
mod field_keyboard_contracts;
#[path = "field_slots_deep.rs"]
mod field_slots_deep;
#[path = "fields.rs"]
mod fields;
#[path = "form_state_lifecycle.rs"]
mod form_state_lifecycle;
#[path = "forms_deep.rs"]
mod forms_deep;
#[path = "select_clear_deep.rs"]
mod select_clear_deep;
#[path = "text_fields.rs"]
mod text_fields;
#[path = "value_props.rs"]
mod value_props;
