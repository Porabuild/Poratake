//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "badge_parts.rs"]
mod badge_parts;
#[path = "button_geometry.rs"]
mod button_geometry;
#[path = "buttons.rs"]
mod buttons;
#[path = "checkbox_form_deep.rs"]
mod checkbox_form_deep;
#[path = "checkbox_motion.rs"]
mod checkbox_motion;
#[path = "chip_deep.rs"]
mod chip_deep;
#[path = "choice_controls_deep.rs"]
mod choice_controls_deep;
#[path = "close_button_deep.rs"]
mod close_button_deep;
#[path = "interaction.rs"]
mod interaction;
#[path = "link_deep.rs"]
mod link_deep;
