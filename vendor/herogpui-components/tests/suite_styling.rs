//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "allocation_axes.rs"]
mod allocation_axes;
#[path = "component_themes.rs"]
mod component_themes;
#[path = "font_seams.rs"]
mod font_seams;
#[path = "hover_overrides.rs"]
mod hover_overrides;
#[path = "hover_repaint.rs"]
mod hover_repaint;
#[path = "parts_thumbs.rs"]
mod parts_thumbs;
#[path = "radius_builders.rs"]
mod radius_builders;
#[path = "radius_painted.rs"]
mod radius_painted;
#[path = "render_props.rs"]
mod render_props;
#[path = "size_enums_deep.rs"]
mod size_enums_deep;
#[path = "sx_ownership.rs"]
mod sx_ownership;
#[path = "sx_radius_parts.rs"]
mod sx_radius_parts;
#[path = "sx_slot.rs"]
mod sx_slot;
#[path = "text_size_knobs.rs"]
mod text_size_knobs;
