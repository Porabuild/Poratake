//! Layout tokens taken from the Electron Tailwind spec so GPUI chrome
//! stays 1:1 with the renderer (`h-10`, `size-7`, `size-8`, `rounded-lg`,
//! `p-1`, `gap-0.5`, `rounded-4xl`).

/// `--radius` from `src/renderer/styles/base.css`, in pixels (0.125rem).
///
/// HeroUI's `@theme inline` in `themes/shared/theme.css` rebinds the whole
/// Tailwind radius scale onto this variable, and the app's `:root` is unlayered
/// so it beats HeroUI's own `@layer theme` default of `0.5rem`. Every
/// `rounded-*` utility in the renderer is therefore a multiple of 2px — a
/// quarter of the stock Tailwind value. Never write a `rounded-*` radius as the
/// stock Tailwind number; derive it here.
pub const ROOT_RADIUS: f32 = 2.0;
/// `rounded-sm` — `calc(var(--radius) * 0.5)`.
#[allow(dead_code)]
pub const RADIUS_SM: f32 = ROOT_RADIUS * 0.5;
/// `rounded-md` — `calc(var(--radius) * 0.75)`.
pub const RADIUS_MD: f32 = ROOT_RADIUS * 0.75;
/// `rounded-lg` — `calc(var(--radius) * 1)`.
pub const RADIUS_LG: f32 = ROOT_RADIUS;
/// `rounded-xl` — `calc(var(--radius) * 1.5)`.
pub const RADIUS_XL: f32 = ROOT_RADIUS * 1.5;
/// `rounded-2xl` — `calc(var(--radius) * 2)`.
pub const RADIUS_2XL: f32 = ROOT_RADIUS * 2.0;
/// `rounded-3xl` — `calc(var(--radius) * 3)`. Also `--field-radius`, which is
/// why fields and buttons share a corner in this theme.
pub const RADIUS_3XL: f32 = ROOT_RADIUS * 3.0;
/// `rounded-4xl` — `calc(var(--radius) * 4)`.
pub const RADIUS_4XL: f32 = ROOT_RADIUS * 4.0;

pub const TITLE_BAR_HEIGHT: f32 = 40.0;
pub const TITLE_BAR_PADDING_X: f32 = 8.0;
pub const TITLE_BAR_GAP: f32 = 4.0;
pub const TOOL_BUTTON_SIZE: f32 = 28.0;
pub const TOOL_BUTTON_ICON: f32 = 16.0;
pub const TOOL_OPTION_HEIGHT: f32 = 28.0;
pub const TOOL_OPTION_RADIUS: f32 = RADIUS_3XL;
pub const TOOL_OPTION_PAD_X: f32 = 8.0;
pub const TOOL_OPTION_GAP: f32 = 8.0;
pub const TOOL_OPTION_CHEVRON: f32 = 14.0;
/// `.select__indicator[data-slot="select-default-indicator"] { size-4 }`.
pub const SELECT_INDICATOR_SIZE: f32 = 16.0;
/// `.select__indicator { end-2 }` — the indicator is absolutely placed.
pub const SELECT_INDICATOR_INSET: f32 = 8.0;
/// `.select__trigger:has(.select__indicator) { pe-7 }` — the content reserves
/// room for the absolutely placed indicator.
pub const SELECT_INDICATOR_PAD_END: f32 = 28.0;
pub const COLOR_SWATCH_XS: f32 = 16.0;
pub const ZOOM_INSET: f32 = 16.0;
pub const ZOOM_PAD: f32 = 4.0;
pub const ZOOM_GAP: f32 = 2.0;
pub const ZOOM_RESET_MIN: f32 = 56.0;
#[allow(dead_code)]
pub const ACTION_BUTTON_SIZE: f32 = 32.0;
pub const SEPARATOR_HEIGHT: f32 = 18.0;
pub const SEPARATOR_INSET: f32 = 4.0;
pub const EDITOR_MIN_WIDTH: f32 = 950.0;
pub const EDITOR_MIN_HEIGHT: f32 = 650.0;
pub const EDITOR_SCREEN_PAD: f32 = 40.0;
pub const WINDOW_CONTROL_WIDTH: f32 = 46.0;
#[allow(dead_code)]
pub const WINDOW_CONTROLS_SPACER: f32 = WINDOW_CONTROL_WIDTH * 3.0;
pub const TRAFFIC_LIGHT_INSET: f32 = 120.0;
pub const VIDEO_TRAFFIC_LIGHT_PAD: f32 = 80.0;

pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

pub const BUTTON_RADIUS: f32 = RADIUS_3XL;
pub const BUTTON_MD_HEIGHT: f32 = 36.0;
pub const BUTTON_SM_HEIGHT: f32 = 32.0;
pub const BUTTON_LG_HEIGHT: f32 = 40.0;
pub const BUTTON_XS_HEIGHT: f32 = 28.0;
pub const BUTTON_MD_PAD_X: f32 = 16.0;
pub const BUTTON_SM_PAD_X: f32 = 12.0;
pub const BUTTON_XS_PAD_X: f32 = 10.0;
pub const BUTTON_MD_TEXT: f32 = 14.0;
pub const BUTTON_LG_TEXT: f32 = 16.0;
pub const BUTTON_XS_TEXT: f32 = 12.0;
pub const BUTTON_XS_ICON: f32 = 14.0;
/// `.button { gap-2 }`.
pub const BUTTON_GAP: f32 = 8.0;
/// `.button { transition: background-color 100ms var(--ease-out) }`.
pub const BUTTON_HOVER_MS: u64 = 100;
/// `button.css` presses scale the button: `0.97` at `md`, `0.98` at `sm`,
/// `0.96` at `lg`.
pub const BUTTON_PRESS_SCALE_MD: f32 = 0.97;
pub const BUTTON_PRESS_SCALE_SM: f32 = 0.98;
pub const BUTTON_PRESS_SCALE_LG: f32 = 0.96;
/// `shortcut-input.tsx`: `gap-1` compact, `min-w-36` / `min-w-16` for the
/// record button.
pub const SHORTCUT_GAP: f32 = 4.0;
pub const SHORTCUT_MIN_WIDTH: f32 = 144.0;
pub const SHORTCUT_MIN_WIDTH_SINGLE: f32 = 64.0;

pub const FIELD_MIN_HEIGHT: f32 = 36.0;
pub const FIELD_RADIUS: f32 = RADIUS_3XL;
pub const FIELD_PAD_X: f32 = 12.0;
pub const FIELD_PAD_Y: f32 = 8.0;
pub const FIELD_TEXT: f32 = 14.0;
/// Fields and select triggers transition their background over 150ms.
pub const FIELD_HOVER_MS: u64 = 150;

pub const SLIDER_TRACK: f32 = 20.0;
pub const SLIDER_KNOB_WIDTH: f32 = 24.0;
pub const SLIDER_KNOB_HEIGHT: f32 = 16.0;
pub const SLIDER_SM_TRACK: f32 = 6.0;
pub const SLIDER_SM_KNOB: f32 = 12.0;

pub const SWITCH_SM_TRACK: (f32, f32) = (32.0, 16.0);
pub const SWITCH_MD_TRACK: (f32, f32) = (40.0, 20.0);
pub const SWITCH_LG_TRACK: (f32, f32) = (48.0, 24.0);
// `switch.css` sizes the small thumb `1.03125rem` wide. Its code comment reads
// "~14.4px on desktop", which assumes a 14px root font; this app never changes
// the root size, so the rem resolves against 16px.
pub const SWITCH_SM_THUMB: (f32, f32) = (16.5, 12.0);
pub const SWITCH_MD_THUMB: (f32, f32) = (22.0, 16.0);
pub const SWITCH_LG_THUMB: (f32, f32) = (27.5, 20.0);
pub const SWITCH_MARGIN: f32 = 2.0;
pub const SWITCH_RADIUS: f32 = 9999.0;
#[allow(dead_code)]
pub const SWITCH_TRAVEL_MS: u64 = 200;

pub const TABS_RADIUS: f32 = 0.0;
pub const TABS_PAD: f32 = 0.0;
pub const TABS_GAP: f32 = 0.0;
pub const TAB_MIN_HEIGHT: f32 = 32.0;
pub const TAB_PAD_X: f32 = 16.0;
pub const TAB_RADIUS: f32 = 0.0;
pub const TAB_TEXT: f32 = 14.0;
pub const TAB_INDICATOR: f32 = 2.0;
/// `.tabs__indicator { transition-duration: 250ms }`.
pub const TAB_INDICATOR_MS: u64 = 250;
/// An unselected tab fades to `opacity-70` over 150ms while hovered.
pub const TAB_HOVER_OPACITY: f32 = 0.7;
pub const TAB_HOVER_MS: u64 = 150;

pub const TOOLTIP_RADIUS: f32 = RADIUS_XL;
pub const TOOLTIP_PAD: f32 = 8.0;
pub const TOOLTIP_TEXT: f32 = 12.0;
/// `.tooltip { max-w-xs }`.
pub const TOOLTIP_MAX_WIDTH: f32 = 320.0;

#[allow(dead_code)]
pub const DIALOG_FADE_MS: u64 = 200;
#[allow(dead_code)]
pub const DIALOG_ZOOM: f32 = 0.95;

pub const PREVIEW_WIDTH: f32 = 200.0;
pub const PREVIEW_HEIGHT: f32 = 140.0;
pub const PREVIEW_RADIUS: f32 = 8.0;
pub const PREVIEW_MARGIN: f32 = 24.0;
pub const PREVIEW_STACK_GAP: f32 = 12.0;
pub const PREVIEW_SHADOW_PADDING: f32 = 4.0;
pub const PREVIEW_MAX_STACK: usize = 4;
pub const PREVIEW_HOVER_SCALE: f32 = 1.05;
pub const PREVIEW_HOVER_MS: u64 = 200;
pub const PREVIEW_CONTROL: f32 = 24.0;
pub const PREVIEW_CONTROL_INSET: f32 = 8.0;
/// The centre actions' `rounded-full bg-background/80 px-3 py-1 text-xs`: the
/// 16px `text-xs` line box plus 4px of padding on each side. `rounded-full` on
/// that box is a radius of half its height.
pub const PREVIEW_PILL_HEIGHT: f32 = 24.0;

pub const OVERLAY_SURFACE_PADDING: f32 = 4.0;
pub const OVERLAY_SURFACE_GAP: f32 = 2.0;
pub const OVERLAY_SURFACE_RADIUS: f32 = RADIUS_4XL;
pub const OVERLAY_BUTTON_SIZE: f32 = 32.0;
pub const OVERLAY_BUTTON_RADIUS: f32 = RADIUS_3XL;
pub const OVERLAY_BORDER_WIDTH: f32 = 2.0;
pub const OVERLAY_HAIRLINE_HEIGHT: f32 = 20.0;
pub const OVERLAY_HAIRLINE_INSET: f32 = 2.0;
/// `capture-target-menu.tsx`: `h-8 w-12 min-w-12 gap-1 px-1.5` with a
/// `size-3` chevron.
pub const OVERLAY_TARGET_TRIGGER_WIDTH: f32 = 48.0;
pub const OVERLAY_TARGET_TRIGGER_PAD_X: f32 = 6.0;
pub const OVERLAY_TARGET_CHEVRON: f32 = 12.0;
pub const OVERLAY_TOOLBAR_TOP: f32 = 24.0;
pub const OVERLAY_TOOLBAR_TOP_MAC: f32 = 48.0;
pub const VIDEO_SIDEBAR_MIN: f32 = 240.0;
pub const VIDEO_SIDEBAR_MAX: f32 = 560.0;
pub const VIDEO_SIDEBAR_RESIZE: f32 = 6.0;
#[allow(dead_code)]
pub const DRAWING_TOOL_GRID_COLS: u32 = 5;
pub const DRAWING_TOOL_GRID_GAP: f32 = 4.0;
#[allow(dead_code)]
pub const DRAWING_TOOL_BUTTON: f32 = 32.0;
pub const DRAWING_STROKE_MIN: f64 = 1.0;
pub const DRAWING_STROKE_MAX: f64 = 16.0;

pub fn overlay_toolbar_top() -> f32 {
    if is_macos() {
        OVERLAY_TOOLBAR_TOP_MAC
    } else {
        OVERLAY_TOOLBAR_TOP
    }
}

pub const WALLPAPER_SHEET_WIDTH: f32 = 320.0;
pub const WALLPAPER_SHEET_PAD: f32 = 20.0;
pub const WALLPAPER_SHEET_GAP: f32 = 16.0;
pub const WALLPAPER_SHEET_INNER_GAP: f32 = 24.0;
pub const WALLPAPER_SECTION_GAP: f32 = 12.0;
pub const WALLPAPER_GRID_GAP: f32 = 8.0;
pub const WALLPAPER_GRID_COLS: u32 = 5;
pub const WALLPAPER_FRAME_COLS: u32 = 3;
pub const WALLPAPER_TILE_RADIUS: f32 = RADIUS_LG;
pub const WALLPAPER_SELECT_WIDTH: f32 = 96.0;
pub const WALLPAPER_FRAME_PREVIEW_H: f32 = 48.0;
pub const WALLPAPER_FRAME_TITLE_H: f32 = 14.0;
pub const WALLPAPER_FRAME_PAD: f32 = 8.0;
pub const WALLPAPER_FRAME_GAP: f32 = 6.0;
pub const SELECT_SM_HEIGHT: f32 = 28.0;
pub const SELECT_SM_TEXT: f32 = 12.0;
pub const TEXT_SM: f32 = 14.0;
pub const TEXT_XS: f32 = 12.0;
pub const VIDEO_ASPECT_COLS: u32 = 4;
pub const VIDEO_ASPECT_GAP: f32 = 6.0;
pub const VIDEO_ASPECT_PAD_X: f32 = 8.0;
pub const VIDEO_ASPECT_PAD_Y: f32 = 6.0;
pub const VIDEO_ASPECT_RADIUS: f32 = RADIUS_MD;
pub const VIDEO_PANEL_PAD: f32 = 16.0;
pub const VIDEO_PANEL_GAP: f32 = 16.0;
pub const SETTINGS_HEADER_TITLE: f32 = 14.0;
pub const SETTINGS_HEADER_DESC: f32 = 12.0;

pub fn wallpaper_tile_size(panel_width: f32, pad: f32) -> f32 {
    let inner = panel_width - pad * 2.0;
    (inner - WALLPAPER_GRID_GAP * (WALLPAPER_GRID_COLS as f32 - 1.0)) / WALLPAPER_GRID_COLS as f32
}

pub const VIDEO_SIDEBAR_WIDTH: f32 = 288.0;
pub const VIDEO_TAB_RAIL_WIDTH: f32 = 40.0;
#[allow(dead_code)]
pub const VIDEO_TAB_BUTTON_SIZE: f32 = 32.0;
pub const VIDEO_TIMELINE_TRACKS: u32 = 5;
pub const VIDEO_TIMELINE_SCROLLBAR: f32 = 12.0;
pub const VIDEO_FILENAME_SIZE: f32 = 14.0;

pub const SETTINGS_WINDOW_WIDTH: f32 = 880.0;
pub const SETTINGS_WINDOW_HEIGHT: f32 = 700.0;
pub const SETTINGS_SIDEBAR_WIDTH: f32 = 240.0;
pub const SETTINGS_CONTENT_MAX: f32 = 720.0;
pub const SETTINGS_CONTENT_PAD_X: f32 = 24.0;
pub const SETTINGS_CONTENT_PAD_TOP: f32 = 12.0;
pub const SETTINGS_CONTENT_PAD_BOTTOM: f32 = 32.0;
pub const SETTINGS_NAV_RADIUS: f32 = RADIUS_3XL;
pub const SETTINGS_NAV_GAP: f32 = 10.0;
pub const SETTINGS_NAV_PX: f32 = 10.0;
pub const SETTINGS_NAV_PY: f32 = 6.0;
pub const SETTINGS_HEADING_SIZE: f32 = 18.0;
pub const SETTINGS_HEADING_GAP: f32 = 16.0;
/// `tracking-[0.12em]` on the sidebar title, at its 12px size.
pub const SETTINGS_TITLE_TRACKING: f32 = TEXT_XS * 0.12;

pub const HISTORY_POPOVER_WIDTH: f32 = 400.0;
pub const HISTORY_POPOVER_HEIGHT: f32 = 500.0;
pub const HISTORY_POPOVER_GAP: f32 = 8.0;
pub const HISTORY_RADIUS: f32 = RADIUS_XL;
pub const HISTORY_TITLE_SIZE: f32 = 14.0;
pub const HISTORY_HEADER_PX: f32 = 16.0;
pub const HISTORY_HEADER_PY: f32 = 12.0;
pub const HISTORY_TOOLBAR_PX: f32 = 12.0;
pub const HISTORY_TOOLBAR_PY: f32 = 6.0;
pub const HISTORY_CONTENT_PAD: f32 = 12.0;
pub const HISTORY_GRID_GAP: f32 = 12.0;
pub const HISTORY_LIST_GAP: f32 = 8.0;
/// The header actions are `h-7` / `h-7 w-7`.
pub const HISTORY_ACTION_SIZE: f32 = 28.0;
/// The toolbar's filter chips and icon buttons are `h-6` / `h-6 w-6`.
pub const HISTORY_CHIP_HEIGHT: f32 = 24.0;
/// `px-2` on the filter chips.
pub const HISTORY_CHIP_PAD_X: f32 = 8.0;
/// The chips' glyphs are `h-3 w-3`, spaced with `mr-1` rather than `gap-2`.
pub const HISTORY_CHIP_ICON: f32 = 12.0;
pub const HISTORY_CHIP_ICON_GAP: f32 = 4.0;
/// The sort and layout buttons carry `h-3.5 w-3.5` glyphs.
pub const HISTORY_TOOL_ICON: f32 = 14.0;
/// The per-item overlay actions are `h-6 w-6` with `h-3 w-3` glyphs.
pub const HISTORY_ITEM_ACTION_SIZE: f32 = 24.0;
pub const HISTORY_ITEM_ACTION_ICON: f32 = 12.0;
/// Item text is `text-xs`.
pub const HISTORY_ITEM_TEXT: f32 = 12.0;
pub const HISTORY_EMPTY_ICON: f32 = 40.0;
pub const HISTORY_EMPTY_FILTER_ICON: f32 = 32.0;

pub const RECORDING_PRE_WIDTH: f32 = 236.0;
pub const RECORDING_WIDTH: f32 = 400.0;
pub const RECORDING_TARGET_LABEL_WIDTH: f32 = 140.0;
pub const RECORDING_WINDOW_HEIGHT: f32 = 52.0;
pub const RECORDING_BAR_PAD_TOP: f32 = 4.0;

pub const PIN_PAD: f32 = 0.0;
pub const PIN_OFFSET: f32 = 30.0;
pub const PIN_ORIGIN_Y: f32 = 20.0;
pub const PIN_RIGHT_MARGIN: f32 = 20.0;
pub const PIN_MIN_RIGHT: f32 = 120.0;

pub const ONBOARDING_WINDOW_WIDTH: f32 = 500.0;
pub const ONBOARDING_WINDOW_HEIGHT: f32 = 650.0;
pub const ONBOARDING_ICON: f32 = 64.0;
pub const ONBOARDING_ICON_RADIUS: f32 = RADIUS_2XL;
pub const ONBOARDING_TITLE_SIZE: f32 = 20.0;
pub const ONBOARDING_BODY_SIZE: f32 = 14.0;
pub const ONBOARDING_HINT_SIZE: f32 = 12.0;
pub const ONBOARDING_CARD_RADIUS: f32 = RADIUS_MD;
pub const ONBOARDING_CARD_PAD: f32 = 12.0;
pub const ONBOARDING_CARD_GAP: f32 = 12.0;
pub const ONBOARDING_DOT: f32 = 8.0;

pub const OVERLAY_DIM: f32 = 0.5;
pub const OVERLAY_PROMPT_TOP: f32 = 32.0;
pub const OVERLAY_PROMPT_TOP_MAC: f32 = 48.0;
pub const OVERLAY_PROMPT_TOOLBAR_GAP: f32 = 40.0;

pub fn overlay_prompt_top(toolbar: bool) -> f32 {
    if toolbar {
        return overlay_toolbar_top() + overlay_bar_height() + OVERLAY_PROMPT_TOOLBAR_GAP;
    }
    if is_macos() {
        return OVERLAY_PROMPT_TOP_MAC;
    }
    OVERLAY_PROMPT_TOP
}
pub const OVERLAY_PROMPT_PX: f32 = 16.0;
pub const OVERLAY_PROMPT_PY: f32 = 8.0;
pub const OVERLAY_PROMPT_SIZE: f32 = 14.0;
pub const OVERLAY_LABEL_RADIUS: f32 = RADIUS_MD;
pub const OVERLAY_LABEL_PX: f32 = 8.0;
pub const OVERLAY_LABEL_PY: f32 = 4.0;
pub const OVERLAY_LABEL_SIZE: f32 = 12.0;
pub const OVERLAY_LABEL_CLEARANCE: f32 = 36.0;
#[allow(dead_code)]
pub const OVERLAY_LABEL_BELOW_OFFSET: f32 = 32.0;
pub const OVERLAY_LABEL_ABOVE_INSET: f32 = 4.0;
#[allow(dead_code)]
pub const OVERLAY_FRAME_BORDER: f32 = 1.0;
pub const OVERLAY_HANDLE_THICKNESS: f32 = 4.0;
pub const OVERLAY_HANDLE_LENGTH: f32 = 20.0;

pub fn overlay_bar_height() -> f32 {
    OVERLAY_BORDER_WIDTH * 2.0 + OVERLAY_SURFACE_PADDING * 2.0 + OVERLAY_BUTTON_SIZE
}

pub fn recording_inner_bar_height() -> f32 {
    overlay_bar_height()
}

pub fn recording_control_width(recording: bool, has_target_name: bool) -> f32 {
    let base = if recording {
        RECORDING_WIDTH
    } else {
        RECORDING_PRE_WIDTH
    };
    if has_target_name {
        base + RECORDING_TARGET_LABEL_WIDTH
    } else {
        base
    }
}

pub fn recording_bar_origin(
    work_x: f32,
    work_y: f32,
    work_width: f32,
    bar_width: f32,
) -> (f32, f32) {
    let x = (work_x + (work_width - bar_width) / 2.0).round();
    let y = work_y + overlay_toolbar_top() - RECORDING_BAR_PAD_TOP;
    (x, y)
}

pub fn display_containing(
    displays: &[(f32, f32, f32, f32)],
    point_x: f32,
    point_y: f32,
) -> Option<(f32, f32, f32, f32)> {
    displays
        .iter()
        .copied()
        .find(|(x, y, width, height)| {
            point_x >= *x && point_x < *x + *width && point_y >= *y && point_y < *y + *height
        })
        .or_else(|| displays.first().copied())
}

pub fn overlay_label_below(selection_y: f32, selection_height: f32, viewport_height: f32) -> bool {
    selection_y + selection_height + OVERLAY_LABEL_CLEARANCE <= viewport_height
}

pub fn overlay_label_top(selection_y: f32, selection_height: f32, viewport_height: f32) -> f32 {
    if overlay_label_below(selection_y, selection_height, viewport_height) {
        selection_y + selection_height + 8.0
    } else {
        selection_y + OVERLAY_LABEL_ABOVE_INSET
    }
}

pub fn overlay_handle_rects(width: f32, height: f32) -> [(f32, f32, f32, f32); 12] {
    let thick = OVERLAY_HANDLE_THICKNESS;
    let long = OVERLAY_HANDLE_LENGTH;
    let mid_x = width / 2.0 - long / 2.0;
    let mid_y = height / 2.0 - long / 2.0;
    [
        (0.0, 0.0, long, thick),
        (0.0, 0.0, thick, long),
        (width - long, 0.0, long, thick),
        (width - thick, 0.0, thick, long),
        (0.0, height - thick, long, thick),
        (0.0, height - long, thick, long),
        (width - long, height - thick, long, thick),
        (width - thick, height - long, thick, long),
        (mid_x, 0.0, long, thick),
        (mid_x, height - thick, long, thick),
        (0.0, mid_y, thick, long),
        (width - thick, mid_y, thick, long),
    ]
}

pub fn editor_window_size(
    image_width: f32,
    image_height: f32,
    work_width: f32,
    work_height: f32,
    stored: Option<(f32, f32)>,
) -> (f32, f32) {
    let max_width = (work_width - EDITOR_SCREEN_PAD * 2.0).max(EDITOR_MIN_WIDTH);
    let max_height = (work_height - EDITOR_SCREEN_PAD * 2.0).max(EDITOR_MIN_HEIGHT);
    let mut width = image_width;
    let mut height = image_height + TITLE_BAR_HEIGHT;
    if width > max_width || height > max_height {
        let scale_x = max_width / image_width.max(1.0);
        let scale_y = (max_height - TITLE_BAR_HEIGHT) / image_height.max(1.0);
        let scale = scale_x.min(scale_y);
        width = (image_width * scale).floor();
        height = (image_height * scale).floor() + TITLE_BAR_HEIGHT;
    }
    if let Some((stored_width, stored_height)) = stored {
        width = stored_width.min(max_width);
        height = stored_height.min(max_height);
    }
    (width.max(EDITOR_MIN_WIDTH), height.max(EDITOR_MIN_HEIGHT))
}

pub fn pin_window_size(
    image_width: f32,
    image_height: f32,
    work_width: f32,
    work_height: f32,
) -> (f32, f32) {
    let width = image_width.max(1.0);
    let height = image_height.max(1.0);
    let scale = 1.0_f32
        .min((work_width * 0.5) / width)
        .min((work_height * 0.5) / height);
    ((width * scale).floor(), (height * scale).floor())
}

pub fn pin_window_origin(window_width: f32, work_width: f32, existing: usize) -> (f32, f32) {
    let offset = existing as f32 * PIN_OFFSET;
    let x = (work_width - window_width - PIN_RIGHT_MARGIN - offset).min(work_width - PIN_MIN_RIGHT);
    (x, PIN_ORIGIN_Y + offset)
}

pub fn history_popover_origin(
    tray: Option<(f32, f32, f32, f32)>,
    screen_width: f32,
    screen_height: f32,
) -> (f32, f32) {
    let Some((tray_x, tray_y, tray_width, tray_height)) = tray else {
        return (
            ((screen_width - HISTORY_POPOVER_WIDTH) / 2.0).round(),
            ((screen_height - HISTORY_POPOVER_HEIGHT) / 2.0).round(),
        );
    };
    let x = (tray_x + tray_width / 2.0 - HISTORY_POPOVER_WIDTH / 2.0).round();
    let y = tray_y + tray_height + HISTORY_POPOVER_GAP;
    let max_y = screen_height - HISTORY_POPOVER_HEIGHT - HISTORY_POPOVER_GAP;
    (x, y.min(max_y.max(0.0)))
}

pub fn video_timeline_tracks_height(track_height: f32) -> f32 {
    VIDEO_TIMELINE_TRACKS as f32 * track_height + VIDEO_TIMELINE_SCROLLBAR
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl PreviewCorner {
    pub fn parse(value: &str) -> Self {
        match value {
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            _ => Self::BottomRight,
        }
    }
}

pub fn preview_origin(
    display_x: f32,
    display_y: f32,
    display_width: f32,
    display_height: f32,
    index: usize,
    corner: PreviewCorner,
) -> (f32, f32) {
    let stack = index as f32 * (PREVIEW_HEIGHT + PREVIEW_STACK_GAP);
    let x = match corner {
        PreviewCorner::TopRight | PreviewCorner::BottomRight => {
            display_x + display_width - PREVIEW_MARGIN - PREVIEW_WIDTH
        }
        PreviewCorner::TopLeft | PreviewCorner::BottomLeft => display_x + PREVIEW_MARGIN,
    };
    let y = match corner {
        PreviewCorner::BottomLeft | PreviewCorner::BottomRight => {
            display_y + display_height - PREVIEW_MARGIN - PREVIEW_HEIGHT - stack
        }
        PreviewCorner::TopLeft | PreviewCorner::TopRight => display_y + PREVIEW_MARGIN + stack,
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::button::ButtonSize;
    use crate::windows::video_editor::timeline::TRACK_HEIGHT;

    /// The renderer's corner radii are not the stock Tailwind scale: HeroUI
    /// rebinds `--radius-*` onto `--radius`, which `base.css` pins at
    /// 0.125rem. Anchoring the scale here means a constant written as a stock
    /// Tailwind number (24 for `rounded-3xl`, 32 for `rounded-4xl`, …) fails
    /// the build instead of shipping a pill where the DOM draws a square.
    #[test]
    fn radius_scale_follows_the_css_custom_property() {
        assert_eq!(ROOT_RADIUS, 2.0, "--radius: 0.125rem at a 16px root");
        assert_eq!(RADIUS_SM, 1.0);
        assert_eq!(RADIUS_MD, 1.5);
        assert_eq!(RADIUS_LG, 2.0);
        assert_eq!(RADIUS_XL, 3.0);
        assert_eq!(RADIUS_2XL, 4.0);
        assert_eq!(RADIUS_3XL, 6.0);
        assert_eq!(RADIUS_4XL, 8.0);
        // `--field-radius: calc(var(--radius) * 3)` is the same 6px as
        // `rounded-3xl`, which is why fields read as button-like.
        assert_eq!(FIELD_RADIUS, RADIUS_3XL);
        assert_eq!(BUTTON_RADIUS, RADIUS_3XL);
        // Every scale step is a quarter of the stock Tailwind value.
        for (ours, stock) in [
            (RADIUS_SM, 4.0),
            (RADIUS_MD, 6.0),
            (RADIUS_LG, 8.0),
            (RADIUS_XL, 12.0),
            (RADIUS_2XL, 16.0),
            (RADIUS_3XL, 24.0),
            (RADIUS_4XL, 32.0),
        ] {
            assert_eq!(ours, stock / 4.0);
        }
    }

    #[test]
    fn editor_chrome_matches_electron() {
        assert_eq!(TITLE_BAR_HEIGHT, 40.0);
        assert_eq!(TOOL_BUTTON_SIZE, 28.0);
        assert_eq!(ACTION_BUTTON_SIZE, 32.0);
        assert_eq!(TITLE_BAR_PADDING_X, 8.0);
        assert_eq!(TITLE_BAR_GAP, 4.0);
        assert_eq!(ButtonSize::IconXs.chrome_size(), TOOL_BUTTON_SIZE);
        assert_eq!(ButtonSize::IconSm.chrome_size(), ACTION_BUTTON_SIZE);
        assert_eq!(TOOL_OPTION_HEIGHT, 28.0);
        assert_eq!(TOOL_OPTION_RADIUS, RADIUS_3XL);
        assert_eq!(TOOL_OPTION_PAD_X, 8.0);
        assert_eq!(TOOL_OPTION_GAP, 8.0);
        assert_eq!(TOOL_OPTION_CHEVRON, 14.0);
        assert_eq!(COLOR_SWATCH_XS, 16.0);
        assert_eq!(ZOOM_INSET, 16.0);
        assert_eq!(ZOOM_PAD, 4.0);
        assert_eq!(ZOOM_GAP, 2.0);
        assert_eq!(ZOOM_RESET_MIN, 56.0);
        assert_eq!(TOOL_OPTION_RADIUS, BUTTON_RADIUS);
        assert_eq!(EDITOR_MIN_WIDTH, 950.0);
        assert_eq!(EDITOR_MIN_HEIGHT, 650.0);
        assert_eq!(EDITOR_SCREEN_PAD, 40.0);
        assert_eq!(WINDOW_CONTROL_WIDTH, 46.0);
        assert_eq!(WINDOW_CONTROLS_SPACER, 138.0);
        assert_eq!(TRAFFIC_LIGHT_INSET, 120.0);
        assert_eq!(VIDEO_TRAFFIC_LIGHT_PAD, 80.0);
        assert_eq!(
            editor_window_size(200.0, 140.0, 1920.0, 1080.0, None),
            (950.0, 650.0)
        );
        assert_eq!(
            editor_window_size(200.0, 140.0, 1920.0, 1080.0, Some((1656.0, 976.0))),
            (1656.0, 976.0)
        );
        let scale = (1840.0_f32 / 4000.0).min((1000.0 - 40.0) / 3000.0);
        assert_eq!(
            editor_window_size(4000.0, 3000.0, 1920.0, 1080.0, None),
            (
                (4000.0 * scale).floor().max(EDITOR_MIN_WIDTH),
                ((3000.0 * scale).floor() + TITLE_BAR_HEIGHT).max(EDITOR_MIN_HEIGHT)
            )
        );
    }

    #[test]
    fn capture_preview_geometry_matches_design() {
        assert_eq!(PREVIEW_WIDTH, 200.0);
        assert_eq!(PREVIEW_HEIGHT, 140.0);
        assert_eq!(PREVIEW_RADIUS, 8.0);
        assert_eq!(PREVIEW_MARGIN, 24.0);
        assert_eq!(PREVIEW_STACK_GAP, 12.0);
        assert_eq!(PREVIEW_SHADOW_PADDING, 4.0);
        assert_eq!(PREVIEW_MAX_STACK, 4);
        assert_eq!(PREVIEW_HOVER_SCALE, 1.05);
        assert_eq!(PREVIEW_HOVER_MS, 200);
    }

    #[test]
    fn overlay_chrome_matches_electron() {
        assert_eq!(OVERLAY_SURFACE_PADDING, 4.0);
        assert_eq!(OVERLAY_SURFACE_GAP, 2.0);
        assert_eq!(OVERLAY_SURFACE_RADIUS, RADIUS_4XL);
        assert_eq!(OVERLAY_BUTTON_SIZE, 32.0);
        assert_eq!(OVERLAY_BUTTON_RADIUS, RADIUS_3XL);
        assert_eq!(OVERLAY_BORDER_WIDTH, 2.0);
        assert_eq!(overlay_bar_height(), 44.0);
        assert_eq!(ButtonSize::IconSm.chrome_size(), OVERLAY_BUTTON_SIZE);
        assert_eq!(OVERLAY_DIM, 0.5);
        assert_eq!(OVERLAY_PROMPT_SIZE, 14.0);
        assert_eq!(OVERLAY_HANDLE_THICKNESS, 4.0);
        assert_eq!(OVERLAY_HANDLE_LENGTH, 20.0);
        assert_eq!(OVERLAY_LABEL_BELOW_OFFSET, 32.0);
        assert_eq!(OVERLAY_LABEL_ABOVE_INSET, 4.0);
        assert_eq!(OVERLAY_FRAME_BORDER, 1.0);
        assert_eq!(OVERLAY_TOOLBAR_TOP, 24.0);
        assert_eq!(OVERLAY_TOOLBAR_TOP_MAC, 48.0);
        if is_macos() {
            assert_eq!(overlay_toolbar_top(), OVERLAY_TOOLBAR_TOP_MAC);
        } else {
            assert_eq!(overlay_toolbar_top(), OVERLAY_TOOLBAR_TOP);
        }
        assert_eq!(
            overlay_prompt_top(true),
            overlay_toolbar_top() + overlay_bar_height() + OVERLAY_PROMPT_TOOLBAR_GAP
        );
        let handles = overlay_handle_rects(200.0, 100.0);
        assert_eq!(handles.len(), 12);
        assert_eq!(handles[0], (0.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[8], (90.0, 0.0, 20.0, 4.0));
        assert_eq!(handles[11], (196.0, 40.0, 4.0, 20.0));
    }

    #[test]
    fn video_editor_chrome_matches_electron() {
        assert_eq!(TITLE_BAR_HEIGHT, 40.0);
        assert_eq!(VIDEO_SIDEBAR_WIDTH, 288.0);
        assert_eq!(VIDEO_TAB_RAIL_WIDTH, 40.0);
        assert_eq!(VIDEO_TAB_BUTTON_SIZE, 32.0);
        assert_eq!(video_timeline_tracks_height(TRACK_HEIGHT), 132.0);
        assert_eq!(ButtonSize::IconXs.chrome_size(), 28.0);
        assert_eq!(ButtonSize::IconSm.chrome_size(), 32.0);
        assert_eq!(VIDEO_SIDEBAR_MIN, 240.0);
        assert_eq!(VIDEO_SIDEBAR_MAX, 560.0);
        assert_eq!(VIDEO_SIDEBAR_RESIZE, 6.0);
        assert_eq!(DRAWING_TOOL_GRID_COLS, 5);
        assert_eq!(DRAWING_TOOL_BUTTON, 32.0);
        assert_eq!(DRAWING_STROKE_MIN, 1.0);
        assert_eq!(DRAWING_STROKE_MAX, 16.0);
    }

    #[test]
    fn settings_history_onboarding_match_electron() {
        assert_eq!(SETTINGS_WINDOW_WIDTH, 880.0);
        assert_eq!(SETTINGS_WINDOW_HEIGHT, 700.0);
        assert_eq!(SETTINGS_SIDEBAR_WIDTH, 240.0);
        assert_eq!(SETTINGS_CONTENT_MAX, 720.0);
        assert_eq!(SETTINGS_CONTENT_PAD_X, 24.0);
        assert_eq!(SETTINGS_CONTENT_PAD_TOP, 12.0);
        assert_eq!(SETTINGS_NAV_RADIUS, RADIUS_3XL);
        assert_eq!(HISTORY_POPOVER_WIDTH, 400.0);
        assert_eq!(HISTORY_POPOVER_HEIGHT, 500.0);
        assert_eq!(HISTORY_POPOVER_GAP, 8.0);
        assert_eq!(HISTORY_RADIUS, RADIUS_XL);
        assert_eq!(HISTORY_TITLE_SIZE, 14.0);
        assert_eq!(HISTORY_GRID_GAP, 12.0);
        assert_eq!(HISTORY_LIST_GAP, 8.0);
        assert_eq!(HISTORY_ACTION_SIZE, 28.0);
        assert_eq!(HISTORY_CHIP_HEIGHT, 24.0);
        assert_eq!(ButtonSize::Xs.chrome_size(), HISTORY_ACTION_SIZE);
        assert_eq!(ONBOARDING_WINDOW_WIDTH, 500.0);
        assert_eq!(ONBOARDING_WINDOW_HEIGHT, 650.0);
        assert_eq!(ONBOARDING_CARD_RADIUS, RADIUS_MD);
        assert_eq!(ONBOARDING_BODY_SIZE, 14.0);
        assert_eq!(OVERLAY_DIM, 0.5);
        assert_eq!(OVERLAY_PROMPT_SIZE, 14.0);
        assert_eq!(OVERLAY_LABEL_RADIUS, RADIUS_MD);
        assert_eq!(OVERLAY_FRAME_BORDER, 1.0);
        assert_eq!(TITLE_BAR_HEIGHT, 40.0);
    }

    #[test]
    fn recording_control_chrome_matches_electron() {
        assert_eq!(RECORDING_PRE_WIDTH, 236.0);
        assert_eq!(RECORDING_WIDTH, 400.0);
        assert_eq!(RECORDING_TARGET_LABEL_WIDTH, 140.0);
        assert_eq!(RECORDING_WINDOW_HEIGHT, 52.0);
        assert_eq!(RECORDING_BAR_PAD_TOP, 4.0);
        assert_eq!(overlay_bar_height(), 44.0);
        assert_eq!(recording_inner_bar_height(), overlay_bar_height());
        assert_ne!(RECORDING_WINDOW_HEIGHT, recording_inner_bar_height());
        assert_eq!(RECORDING_BAR_PAD_TOP + recording_inner_bar_height(), 48.0);
        assert_eq!(recording_control_width(false, false), 236.0);
        assert_eq!(recording_control_width(true, false), 400.0);
        assert_eq!(recording_control_width(false, true), 376.0);
        assert_eq!(recording_control_width(true, true), 540.0);
        assert_eq!(
            recording_bar_origin(0.0, 10.0, 1920.0, 236.0),
            (
                ((1920.0_f32 - 236.0) / 2.0).round(),
                10.0 + overlay_toolbar_top() - RECORDING_BAR_PAD_TOP
            )
        );
        assert_eq!(
            display_containing(
                &[(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 1920.0, 1080.0)],
                2000.0,
                100.0
            ),
            Some((1920.0, 0.0, 1920.0, 1080.0))
        );
        assert_eq!(PIN_PAD, 0.0);
        assert_eq!(
            pin_window_size(400.0, 300.0, 1920.0, 1080.0),
            (400.0, 300.0)
        );
        assert_eq!(
            pin_window_size(2000.0, 2000.0, 1920.0, 1080.0),
            (540.0, 540.0)
        );
    }

    #[test]
    fn widget_metrics_match_electron() {
        assert_eq!(BUTTON_RADIUS, RADIUS_3XL);
        assert_eq!(BUTTON_MD_HEIGHT, 36.0);
        assert_eq!(BUTTON_SM_HEIGHT, OVERLAY_BUTTON_SIZE);
        assert_eq!(BUTTON_LG_HEIGHT, TITLE_BAR_HEIGHT);
        assert_eq!(BUTTON_XS_HEIGHT, TOOL_BUTTON_SIZE);
        assert_eq!(ButtonSize::Md.chrome_size(), BUTTON_MD_HEIGHT);
        assert_eq!(ButtonSize::Sm.chrome_size(), BUTTON_SM_HEIGHT);
        assert_eq!(ButtonSize::Lg.chrome_size(), BUTTON_LG_HEIGHT);
        assert_eq!(ButtonSize::Xs.chrome_size(), BUTTON_XS_HEIGHT);
        assert_eq!(ButtonSize::IconXs.chrome_size(), TOOL_BUTTON_SIZE);
        assert_eq!(ButtonSize::IconSm.chrome_size(), OVERLAY_BUTTON_SIZE);
        assert_eq!(FIELD_MIN_HEIGHT, 36.0);
        assert_eq!(FIELD_RADIUS, RADIUS_3XL);
        assert_eq!(FIELD_PAD_X, 12.0);
        assert_eq!(FIELD_PAD_Y, 8.0);
        assert_eq!(FIELD_TEXT, 14.0);
        assert_eq!(SLIDER_TRACK, 20.0);
        assert_eq!(SLIDER_KNOB_WIDTH, 24.0);
        assert_eq!(SLIDER_KNOB_HEIGHT, 16.0);
        assert_eq!(SLIDER_SM_TRACK, 6.0);
        assert_eq!(SLIDER_SM_KNOB, 12.0);
        assert_eq!(TABS_RADIUS, 0.0);
        assert_eq!(TABS_PAD, 0.0);
        assert_eq!(TABS_GAP, 0.0);
        assert_eq!(TAB_MIN_HEIGHT, 32.0);
        assert_eq!(TAB_PAD_X, 16.0);
        assert_eq!(TAB_RADIUS, 0.0);
        assert_eq!(TAB_TEXT, 14.0);
        assert_eq!(TAB_INDICATOR, 2.0);
        assert_eq!(TOOLTIP_RADIUS, RADIUS_XL);
        assert_eq!(TOOLTIP_PAD, 8.0);
        assert_eq!(TOOLTIP_TEXT, 12.0);
        assert_eq!(SWITCH_RADIUS, 9999.0);
        assert_eq!(SWITCH_MD_TRACK, (40.0, 20.0));
        assert_eq!(SWITCH_SM_TRACK, (32.0, 16.0));
        assert_eq!(SWITCH_LG_TRACK, (48.0, 24.0));
    }

    #[test]
    fn preview_origin_stacks_from_the_chosen_corner() {
        let bottom_right = preview_origin(0.0, 0.0, 1920.0, 1080.0, 1, PreviewCorner::BottomRight);
        assert_eq!(
            bottom_right,
            (
                1920.0 - PREVIEW_MARGIN - PREVIEW_WIDTH,
                1080.0 - PREVIEW_MARGIN - PREVIEW_HEIGHT - (PREVIEW_HEIGHT + PREVIEW_STACK_GAP)
            )
        );
        let top_left = preview_origin(10.0, 20.0, 800.0, 600.0, 0, PreviewCorner::TopLeft);
        assert_eq!(top_left, (10.0 + PREVIEW_MARGIN, 20.0 + PREVIEW_MARGIN));
        assert_eq!(
            PreviewCorner::parse("bottom-right"),
            PreviewCorner::BottomRight
        );
        assert_eq!(PreviewCorner::parse("top-left"), PreviewCorner::TopLeft);
        assert_eq!(
            preview_origin(0.0, 0.0, 1920.0, 1080.0, 0, PreviewCorner::TopRight),
            (1920.0 - PREVIEW_MARGIN - PREVIEW_WIDTH, PREVIEW_MARGIN)
        );
        assert_eq!(
            preview_origin(0.0, 0.0, 1920.0, 1080.0, 2, PreviewCorner::BottomLeft),
            (
                PREVIEW_MARGIN,
                1080.0
                    - PREVIEW_MARGIN
                    - PREVIEW_HEIGHT
                    - 2.0 * (PREVIEW_HEIGHT + PREVIEW_STACK_GAP)
            )
        );
    }

    #[test]
    fn overlay_dim_and_label_placement_match_electron() {
        assert_eq!(OVERLAY_DIM, 0.5);
        assert!(overlay_label_below(10.0, 100.0, 200.0));
        assert!(!overlay_label_below(160.0, 20.0, 200.0));
        assert_eq!(overlay_label_top(10.0, 100.0, 200.0), 118.0);
        assert_eq!(overlay_label_top(160.0, 20.0, 200.0), 164.0);
    }

    #[test]
    fn recording_bar_width_by_mode_and_target_label() {
        assert_eq!(recording_control_width(false, false), RECORDING_PRE_WIDTH);
        assert_eq!(recording_control_width(true, false), RECORDING_WIDTH);
        assert_eq!(
            recording_control_width(false, true),
            RECORDING_PRE_WIDTH + RECORDING_TARGET_LABEL_WIDTH
        );
        assert_eq!(
            recording_control_width(true, true),
            RECORDING_WIDTH + RECORDING_TARGET_LABEL_WIDTH
        );
        let (x, y) = recording_bar_origin(100.0, 40.0, 800.0, 236.0);
        assert_eq!(y + RECORDING_BAR_PAD_TOP, 40.0 + overlay_toolbar_top());
        assert_eq!(x, (100.0_f32 + (800.0 - 236.0) / 2.0).round());
    }

    #[test]
    fn history_popover_origin_matches_electron() {
        assert_eq!(
            history_popover_origin(None, 1920.0, 1080.0),
            (
                ((1920.0 - HISTORY_POPOVER_WIDTH) / 2.0).round(),
                ((1080.0 - HISTORY_POPOVER_HEIGHT) / 2.0).round()
            )
        );
        let from_tray = history_popover_origin(Some((100.0, 10.0, 40.0, 28.0)), 1920.0, 1080.0);
        assert_eq!(
            from_tray,
            (
                (100.0 + 20.0 - HISTORY_POPOVER_WIDTH / 2.0).round(),
                10.0 + 28.0 + HISTORY_POPOVER_GAP
            )
        );
    }

    #[test]
    fn animation_constants_match_electron() {
        assert_eq!(PREVIEW_HOVER_SCALE, 1.05);
        assert_eq!(PREVIEW_HOVER_MS, 200);
        assert_eq!(SWITCH_TRAVEL_MS, 200);
        assert_eq!(DIALOG_FADE_MS, 200);
        assert_eq!(DIALOG_ZOOM, 0.95);
    }
}
