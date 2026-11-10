//! HeroGPUI components — a faithful Rust/GPUI port of the HeroUI v3 component
//! library. One module per `@heroui/*` package.
//!
//! Every component and builder type is `#[must_use]`: a builder that is built
//! and then dropped renders nothing, so the compiler says so.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! # use herogpui_components::Button;
//! # fn render() {
//! Button::new("save").label("Save"); // dropped: `unused_must_use` fires
//! # }
//! ```
//!
//! The same statement, used:
//!
//! ```
//! #![deny(unused_must_use)]
//! # use herogpui_components::Button;
//! # fn render() -> Button {
//! Button::new("save").label("Save")
//! # }
//! ```
#![allow(clippy::type_complexity)]
// Every public item is documented; `clippy -D warnings` keeps it that way.
#![warn(missing_docs)]

pub mod a11y;
pub mod accordion;
pub mod alert;
pub mod alert_dialog;
// Reached by path (`herogpui::anim::…`); not re-exported at the root.
pub mod anim;
pub mod assets;
pub mod autocomplete;
pub mod avatar;
pub mod avatar_group;
pub mod badge;
pub mod breadcrumbs;
pub mod button;
pub mod button_group;
pub mod calendar;
mod calendar_keys;
pub mod calendar_system;
pub mod calendar_view;
pub mod card;
pub mod checkbox;
pub mod chip;
pub mod close_button;
pub mod color_picker;
pub mod combo_box;
pub mod command_palette;
pub mod context_menu;
pub mod date_constraints;
pub mod date_picker;
pub mod disclosure;
pub mod drawer;
pub mod dropdown;
pub mod field;
pub mod filter;
pub mod form;
#[cfg(feature = "gallery-source")]
pub mod gallery_source;
pub mod hover_card;
pub mod i18n;
pub mod icon;
pub mod icons;
pub mod input;
pub mod input_group;
pub mod input_otp;
pub mod kbd;
pub mod link;
pub mod list_box;
pub(crate) mod list_core;
pub mod list_nav;
mod matches;
pub mod menu_bar;
pub mod meter;
pub mod modal;
pub mod number_field;
pub mod pagination;
pub mod picker_item;
pub mod popover;
pub mod progress;
pub mod radio_group;
pub mod range_calendar;
pub mod resizable;
pub mod scroll_shadow;
pub mod scrollbar;
pub mod select;
pub mod selection;
pub mod separator;
pub mod sheet;
pub mod sidebar;
pub mod skeleton;
pub mod slider;
pub mod spinner;
pub mod surface;
pub mod switch;
mod system_locale;
pub mod table;
pub mod tabs;
pub mod tag_group;
pub mod textarea;
pub mod time_field;
pub mod title_bar;
pub mod toast;
pub mod toggle_button;
pub mod toolbar;
pub mod tooltip;
pub mod traits;
pub mod tree_view;
pub mod typography;
pub(crate) mod util;
pub mod validation;
pub mod virtual_list;

// The shared v3 prop vocabularies, re-exported so `herogpui::components::*`
// is enough to build a UI.
pub use herogpui_core::{
    Backdrop, Color, FieldVariant, Orientation, Placement, PlacementAlign, Prominence,
    SelectionBehavior, SelectionMode, Size, SizeXl, Variant,
};

pub use accordion::*;
pub use alert::*;
pub use alert_dialog::*;
pub use assets::*;
pub use autocomplete::*;
pub use avatar::*;
pub use avatar_group::*;
pub use badge::*;
pub use breadcrumbs::*;
pub use button::*;
pub use button_group::*;
pub use calendar::*;
pub use calendar_view::*;
pub use card::*;
pub use checkbox::*;
pub use chip::*;
pub use close_button::*;
pub use color_picker::*;
pub use combo_box::*;
pub use command_palette::*;
pub use context_menu::*;
pub use date_constraints::*;
pub use date_picker::*;
pub use disclosure::*;
pub use drawer::*;
pub use dropdown::*;
pub use field::*;
pub use filter::*;
pub use form::*;
pub use hover_card::*;
pub use sheet::*;

/// `formatOptions` for the components that take it, re-exported so a caller
/// reaches it beside the component it configures.
pub use herogpui_core::{CurrencySign, NumberFormat, NumberStyle, UnitDisplay};
pub use icon::*;
pub use icons::*;
pub use input::*;
pub use input_group::*;
pub use input_otp::*;
pub use kbd::*;
pub use link::*;
pub use list_box::*;
pub use menu_bar::*;
pub use meter::*;
pub use modal::*;
pub use number_field::*;
pub use pagination::*;
pub use picker_item::*;
pub use popover::*;
pub use progress::*;
pub use radio_group::*;
pub use range_calendar::*;
pub use resizable::*;
pub use scroll_shadow::*;
pub use scrollbar::*;
pub use select::*;
pub use selection::*;
pub use separator::*;
pub use sidebar::*;
pub use skeleton::*;
pub use slider::*;
pub use spinner::*;
pub use surface::*;
pub use switch::*;
pub use table::*;
pub use tabs::*;
pub use tag_group::*;
pub use textarea::*;
pub use time_field::*;
pub use title_bar::*;
pub use toast::*;
pub use toggle_button::*;
pub use toolbar::*;
pub use tooltip::*;
pub use traits::{Disableable, Selectable, Sizable};
pub use tree_view::*;
pub use typography::*;
pub use util::{app_focus_root, FieldFocus, InteractiveState, SelectionValue};
pub use validation::*;
pub use virtual_list::*;

/// The helpers HeroGPUI supports for building custom widgets that match the
/// components: v3's radius scale read from the active theme, the field
/// metrics, the interactive cursor, the focus-visible modality, and the
/// vanilla-GPUI workarounds the components themselves use.
///
/// Everything else the components share stays private to this crate, so its
/// signatures can change without a breaking release.
pub mod extend {
    pub use crate::util::{
        app_focus_root, container_radius, control_radius, cursor_interactive, field_radius,
        focus_visible, hairline_radius, inner_fill_radius, interactive_cursor, key_radius,
        mark_radius, micro_radius, set_focus_visible, shift_wheel_scroll_x, small_radius,
        soft_radius, FIELD_HEIGHT, FIELD_ICON, FIELD_TEXT,
    };
}

/// Overlay-stack primitives exercised by this crate's own behavior tests.
/// Not public API: no semver guarantee, and it may change in any release.
#[doc(hidden)]
pub mod __private {
    /// The system date/time locale chain the date and time fields read, so
    /// the behavior tests can expect what the running machine's region says.
    pub use crate::system_locale::time_locale_tags;
    pub use crate::util::{
        dismiss_on_press_outside_with_token, overlay_phase, overlay_scope, overlay_scope_with_exit,
        DismissResult, OverlayPhase, OverlayToken,
    };
}
