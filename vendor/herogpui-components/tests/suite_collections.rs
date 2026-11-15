//! Integration tests grouped by feature area to keep link time bounded.

#[path = "harness/mod.rs"]
mod harness;
#[path = "source_scan/mod.rs"]
mod source_scan;

#[path = "collection_contracts.rs"]
mod collection_contracts;
#[path = "collections.rs"]
mod collections;
#[path = "collections_deep.rs"]
mod collections_deep;
#[path = "nav_deep.rs"]
mod nav_deep;
#[path = "parts_pagination.rs"]
mod parts_pagination;
#[path = "tabs_deep.rs"]
mod tabs_deep;
#[path = "tag_geometry_deep.rs"]
mod tag_geometry_deep;
#[path = "tree_view.rs"]
mod tree_view;
#[path = "virtual_and_feedback.rs"]
mod virtual_and_feedback;
#[path = "virtual_list.rs"]
mod virtual_list;
