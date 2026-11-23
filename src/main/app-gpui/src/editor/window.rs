//! Editor window entity — owns editor state, wires the title bar and canvas
//! together, and hosts the color palette popover.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, DragMoveEvent, ExternalPaths, FileDropEvent,
    ModifiersChangedEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render,
    ScrollEvent, Styled, Subscription, Window,
};
use herogpui::gpui;

use crate::editor::actions;
use crate::editor::annotations::{self, Annotation, AnnotationHistory, Point, ResizeHandle};
use crate::editor::canvas::{CanvasSnapshot, EditorCanvas, LiveBackground};
use crate::editor::options::{
    EditorAction, EditorHandlers, EditorOption, MAX_ZOOM, MIN_ZOOM, ZOOM_STEP,
};
use crate::editor::text_input::{InlineTextInput, InlineTextState};
use crate::editor::title_bar::TitleBar;
use crate::editor::tool_options::ToolOptionsState;
use crate::theme::vars::active_theme;
use crate::ui::chrome;
use crate::ui::colors::Tool;
use crate::ui::icon_button;
use herogpui::components::{Button, InputState, Variant};

fn report_screenshot_save_failure(cx: &mut App, path: &Path, error: &dyn std::fmt::Display) {
    let message = format!("{}: {error}", path.display());
    eprintln!("[editor] screenshot save failed: {message}");
    crate::windows::toast::Toast::show(cx, "Screenshot Save Failed", message);
}

#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedEditorState {
    #[serde(default)]
    annotations: Vec<Annotation>,
    #[serde(default)]
    wallpaper: crate::editor::wallpaper::WallpaperSettings,
    #[serde(default)]
    layers: Vec<crate::editor::layers::ImageLayer>,
}

impl PersistedEditorState {
    pub(crate) fn load_for_screenshot(path: &str) -> Self {
        crate::history_store::editor_state_for_path(std::path::Path::new(path))
            .and_then(|state| serde_json::from_value(state).ok())
            .unwrap_or_default()
    }

    fn to_json_value(&self) -> Option<serde_json::Value> {
        let mut wallpaper = self.wallpaper.clone();
        if let Some(source) = wallpaper.background_image.as_deref() {
            if !source.starts_with("data:") {
                if let Ok(bytes) = std::fs::read(source) {
                    use base64::Engine;
                    let mime = match std::path::Path::new(source)
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .map(str::to_ascii_lowercase)
                        .as_deref()
                    {
                        Some("jpg" | "jpeg") => "image/jpeg",
                        Some("webp") => "image/webp",
                        Some("gif") => "image/gif",
                        Some("svg") => "image/svg+xml",
                        _ => "image/png",
                    };
                    wallpaper.background_image = Some(format!(
                        "data:{mime};base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    ));
                }
            }
        }
        serde_json::to_value(PersistedEditorState {
            annotations: self.annotations.clone(),
            wallpaper,
            layers: self.layers.clone(),
        })
        .ok()
    }
}

struct EditorStateWriter {
    revision: AtomicU64,
    persisted: parking_lot::Mutex<Option<PersistedEditorState>>,
}

impl EditorStateWriter {
    fn next_revision(&self) -> u64 {
        self.revision
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }

    fn write(&self, path: &Path, state: PersistedEditorState, revision: u64) -> bool {
        let mut persisted = self.persisted.lock();
        if self.revision.load(Ordering::Relaxed) != revision || persisted.as_ref() == Some(&state) {
            return true;
        }
        let Some(value) = state.to_json_value() else {
            return false;
        };
        if !crate::history_store::update_editor_state_by_path(path, value) {
            return false;
        }
        *persisted = Some(state);
        true
    }
}

struct PreCropState {
    base_image: Arc<image::DynamicImage>,
    width: f32,
    height: f32,
    annotations: Vec<Annotation>,
}

/// The in-progress custom background — port of `BackgroundEditor`'s state,
/// gradient or image, blank or loaded from a saved custom for editing.
pub struct BackgroundDraft {
    pub id: Option<String>,
    pub draft_type: BackgroundDraftType,
    pub colors: Vec<String>,
    pub angle: f64,
    pub active_color: Option<usize>,
    pub fields: Vec<gpui::Entity<InputState>>,
    pub image_url: Option<String>,
    pub preview: Option<Arc<gpui::RenderImage>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackgroundDraftType {
    Gradient,
    Image,
}

impl BackgroundDraft {
    fn new(id: Option<String>, colors: Vec<String>, angle: f64, cx: &mut App) -> Self {
        let fields = colors
            .iter()
            .map(|color| cx.new(|cx| InputState::with_value(cx, color.clone())))
            .collect();
        Self {
            id,
            draft_type: BackgroundDraftType::Gradient,
            colors,
            angle,
            active_color: None,
            fields,
            image_url: None,
            preview: None,
        }
    }
}

pub const BACKGROUND_DEFAULT_COLORS: [&str; 2] = ["#f97316", "#ec4899"];

pub const BACKGROUND_PALETTE: [&str; 16] = [
    "#ef4444", "#f97316", "#f59e0b", "#84cc16", "#22c55e", "#14b8a6", "#06b6d4", "#0ea5e9",
    "#3b82f6", "#6366f1", "#8b5cf6", "#a855f7", "#d946ef", "#ec4899", "#f43f5e", "#1e293b",
];

pub const BACKGROUND_MAX_COLORS: usize = 5;
pub const BACKGROUND_MIN_COLORS: usize = 2;

struct ResizeState {
    handle: ResizeHandle,
    start: Point,
    last: Point,
    original: Option<Vec<Annotation>>,
    anchor: Option<(f64, f64)>,
    initial_drag: Option<(f64, f64)>,
    initial_font_size: Option<f64>,
    initial_size: Option<(f64, f64)>,
    center: Option<(f64, f64)>,
    initial_rotation: Option<f64>,
    start_angle: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CropCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug)]
enum CropDrag {
    Move {
        start: Point,
        rect: (f64, f64, f64, f64),
    },
    Resize {
        corner: CropCorner,
        start: Point,
        rect: (f64, f64, f64, f64),
    },
}

const CROP_MIN_SIZE: f64 = 20.0;

fn normalize_crop_rect(rect: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let (x, y, width, height) = rect;
    (
        if width < 0.0 { x + width } else { x },
        if height < 0.0 { y + height } else { y },
        width.abs(),
        height.abs(),
    )
}

fn crop_handle_at(rect: (f64, f64, f64, f64), point: Point, tolerance: f64) -> Option<CropCorner> {
    let (x, y, width, height) = rect;
    let (px, py) = (f64::from(point.x), f64::from(point.y));
    for (corner, hx, hy) in [
        (CropCorner::TopLeft, x, y),
        (CropCorner::TopRight, x + width, y),
        (CropCorner::BottomLeft, x, y + height),
        (CropCorner::BottomRight, x + width, y + height),
    ] {
        if (px - hx).abs() <= tolerance && (py - hy).abs() <= tolerance {
            return Some(corner);
        }
    }
    None
}

fn point_in_crop_rect(rect: (f64, f64, f64, f64), point: Point) -> bool {
    let (x, y, width, height) = rect;
    let (px, py) = (f64::from(point.x), f64::from(point.y));
    px >= x && px <= x + width && py >= y && py <= y + height
}

/// Port of `getDropEdge` in `useImageDrop.ts`: the dragged image attaches to
/// whichever edge of the stage the pointer is nearest to.
fn drop_edge_for_point(
    position: gpui::Point<Pixels>,
    bounds: gpui::Bounds<Pixels>,
) -> crate::editor::layers::Edge {
    use crate::editor::layers::Edge;

    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0.0 || height <= 0.0 {
        return Edge::Right;
    }
    let rel_x = f32::from(position.x - bounds.origin.x) / width;
    let rel_y = f32::from(position.y - bounds.origin.y) / height;
    let dist_top = rel_y;
    let dist_bottom = 1.0 - rel_y;
    let dist_left = rel_x;
    let dist_right = 1.0 - rel_x;
    let min = dist_top.min(dist_bottom).min(dist_left).min(dist_right);
    if min == dist_top {
        Edge::Top
    } else if min == dist_bottom {
        Edge::Bottom
    } else if min == dist_left {
        Edge::Left
    } else {
        Edge::Right
    }
}

/// The `hasImageFile` gate: only drops carrying a decodable image light up
/// the stage. The decoder only builds png, jpeg and gif support.
fn is_image_path(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif")
    )
}

fn resize_crop_rect(
    rect: (f64, f64, f64, f64),
    corner: CropCorner,
    dx: f64,
    dy: f64,
    image_width: f64,
    image_height: f64,
) -> (f64, f64, f64, f64) {
    let (x, y, width, height) = rect;
    let mut rect = rect;
    match corner {
        CropCorner::TopLeft => {
            let new_width = width - dx;
            if new_width < CROP_MIN_SIZE {
                rect.0 = x + width - CROP_MIN_SIZE;
                rect.2 = CROP_MIN_SIZE;
            } else if x + dx < 0.0 {
                rect.0 = 0.0;
                rect.2 = x + width;
            } else {
                rect.0 = x + dx;
                rect.2 = new_width;
            }
            let new_height = height - dy;
            if new_height < CROP_MIN_SIZE {
                rect.1 = y + height - CROP_MIN_SIZE;
                rect.3 = CROP_MIN_SIZE;
            } else if y + dy < 0.0 {
                rect.1 = 0.0;
                rect.3 = y + height;
            } else {
                rect.1 = y + dy;
                rect.3 = new_height;
            }
        }
        CropCorner::TopRight => {
            let new_width = width + dx;
            rect.2 = if new_width < CROP_MIN_SIZE {
                CROP_MIN_SIZE
            } else if x + new_width > image_width {
                image_width - x
            } else {
                new_width
            };
            let new_height = height - dy;
            if new_height < CROP_MIN_SIZE {
                rect.1 = y + height - CROP_MIN_SIZE;
                rect.3 = CROP_MIN_SIZE;
            } else if y + dy < 0.0 {
                rect.1 = 0.0;
                rect.3 = y + height;
            } else {
                rect.1 = y + dy;
                rect.3 = new_height;
            }
        }
        CropCorner::BottomLeft => {
            let new_width = width - dx;
            if new_width < CROP_MIN_SIZE {
                rect.0 = x + width - CROP_MIN_SIZE;
                rect.2 = CROP_MIN_SIZE;
            } else if x + dx < 0.0 {
                rect.0 = 0.0;
                rect.2 = x + width;
            } else {
                rect.0 = x + dx;
                rect.2 = new_width;
            }
            let new_height = height + dy;
            rect.3 = if new_height < CROP_MIN_SIZE {
                CROP_MIN_SIZE
            } else if y + new_height > image_height {
                image_height - y
            } else {
                new_height
            };
        }
        CropCorner::BottomRight => {
            let new_width = width + dx;
            rect.2 = if new_width < CROP_MIN_SIZE {
                CROP_MIN_SIZE
            } else if x + new_width > image_width {
                image_width - x
            } else {
                new_width
            };
            let new_height = height + dy;
            rect.3 = if new_height < CROP_MIN_SIZE {
                CROP_MIN_SIZE
            } else if y + new_height > image_height {
                image_height - y
            } else {
                new_height
            };
        }
    }
    rect
}

type BackdropKey = (
    crate::editor::wallpaper::WallpaperSettings,
    u32,
    u32,
    Vec<crate::editor::layers::ImageLayer>,
    f64,
    Option<[u8; 3]>,
    Option<gpui::ImageId>,
    f64,
);

#[derive(Clone, PartialEq)]
struct LiveBackgroundSource {
    gradient: Option<crate::editor::wallpaper::GradientOption>,
    background_image: Option<String>,
    has_effects: bool,
}

impl LiveBackgroundSource {
    fn from_wallpaper(wallpaper: &crate::editor::wallpaper::WallpaperSettings) -> Self {
        Self {
            gradient: wallpaper.gradient.clone(),
            background_image: wallpaper.background_image.clone(),
            has_effects: wallpaper.background_blur > 0.0 || wallpaper.noise > 0.0,
        }
    }
}

#[derive(Clone, PartialEq)]
struct LiveBackgroundKey {
    source: LiveBackgroundSource,
    blur_bits: u64,
    noise_bits: u64,
}

impl LiveBackgroundKey {
    fn from_wallpaper(wallpaper: &crate::editor::wallpaper::WallpaperSettings) -> Self {
        Self {
            source: LiveBackgroundSource::from_wallpaper(wallpaper),
            blur_bits: wallpaper.background_blur.to_bits(),
            noise_bits: wallpaper.noise.to_bits(),
        }
    }
}

fn backdrop_layout_changed(previous: &BackdropKey, next: &BackdropKey) -> bool {
    previous.1 != next.1
        || previous.2 != next.2
        || previous.4 != next.4
        || previous.5 != next.5
        || previous.0.window_frame.style != next.0.window_frame.style
        || previous.0.balance != next.0.balance
        || previous.0.inset != next.0.inset
        || previous.0.corners != next.0.corners
        || previous.0.shadow != next.0.shadow
        || previous.0.spacing != next.0.spacing
        || previous.3 != next.3
        || previous.6 != next.6
        || previous.7 != next.7
}

fn backdrop_source_changed(previous: &BackdropKey, next: &BackdropKey) -> bool {
    previous.0.gradient != next.0.gradient || previous.0.background_image != next.0.background_image
}

pub struct EditorWindow {
    pub image: Option<Arc<gpui::RenderImage>>,
    pub base_image: Option<Arc<image::DynamicImage>>,
    base_pixmap: Option<(
        std::sync::Weak<image::DynamicImage>,
        Arc<tiny_skia::Pixmap>,
        bool,
    )>,
    pub is_copied: bool,
    copy_pending: bool,
    pin_pending: bool,
    copy_task: Option<gpui::Task<()>>,
    pub image_width: f32,
    pub image_height: f32,
    image_opaque: bool,
    pub file_path: String,
    pub tool: Tool,
    pub color_hex: String,
    pub stroke_width: f64,
    pub arrow_style: String,
    pub highlight_color: String,
    pub highlight_opacity: f64,
    pub number_style: String,
    pub number_size: String,
    pub number_start_value: f64,
    pub text_background: bool,
    pub text_font_size: f64,
    pub text_font_family: String,
    pub redact_style: String,
    pub redact_intensity: f64,
    pub shape_fill_mode: String,
    pub zoom: f32,
    pub history: AnnotationHistory,
    pub draft: Option<Annotation>,
    pub drag_start: Option<Point>,
    pending_pointer_update: Option<Point>,
    pending_stroke_points: Vec<Point>,
    pending_stroke_shift: bool,
    pointer_update_scheduled: bool,
    /// The inline field shown while a text annotation is being typed.
    text_editor: Option<(String, gpui::Entity<InlineTextState>)>,
    /// The pending crop rectangle in image coordinates.
    crop: Option<(f64, f64, f64, f64)>,
    /// An in-progress crop move or corner resize, mirroring `DragState` in
    /// `svg-crop-overlay.tsx`. Set on mousedown so the move path runs ahead
    /// of the new-rectangle drag.
    crop_drag: Option<CropDrag>,
    /// Pre-crop pixels, dimensions and annotations — port of Electron's
    /// `lastCropStateRef`, so undo after crop restores the image too.
    pre_crop: Option<PreCropState>,
    /// The zoom control's measured rect, so its `backdrop-blur-md` can sample
    /// the capture underneath it. Measured rather than computed because the
    /// bar's width follows its label.
    pub(crate) zoom_bar_bounds: std::rc::Rc<std::cell::RefCell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// The blurred crop behind the zoom control, rebuilt only when the sampled
    /// region moves.
    zoom_backdrop: Option<crate::editor::zoom_backdrop::ZoomBackdrop>,
    fit_for: Option<(gpui::Size<Pixels>, bool, (f32, f32))>,
    pub wallpaper: crate::editor::wallpaper::WallpaperSettings,
    native_scale: f64,
    window_scale: f64,
    wallpaper_preset_id: String,
    background_editor: Option<BackgroundDraft>,
    background_editor_generation: u64,
    background_picker_open: bool,
    background_previews: crate::editor::background::BackgroundPreviews,
    preset_draft: Option<PresetDraft>,
    sheet_closing: bool,
    sheet_close_task: Option<gpui::Task<()>>,
    pub cloud_upload: crate::cloud::UploadState,
    redact_patches: std::collections::HashMap<String, crate::editor::canvas::RedactPatch>,
    pub snapshot: SnapshotCell,
    pub bounds: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
    /// Images attached to the capture's edges.
    layers: Vec<crate::editor::layers::ImageLayer>,
    /// Whether the edge overlay for attaching a capture is showing.
    capture_mode: bool,
    meta_held: bool,
    /// The edge a dragged image would attach to, while an OS drag hovers the
    /// stage — the `useImageDrop` state.
    drop_edge: Option<crate::editor::layers::Edge>,
    /// The margin the balance option trims, recomputed when it is turned on.
    balance_crop: Option<(f32, f32, f32, f32)>,
    selected_annotations: Vec<String>,
    move_origin: Option<Point>,
    move_original: Option<(Point, Vec<Annotation>)>,
    resize_state: Option<ResizeState>,
    marquee: Option<(Point, Point)>,
    /// Annotations on the editor's own clipboard, from copy or cut.
    annotation_clipboard: Vec<Annotation>,
    /// The rasterized wallpaper backdrop and the settings it was made for,
    /// so it is only re-rendered when the wallpaper actually changes.
    backdrop: Option<Arc<gpui::RenderImage>>,
    backdrop_composite: bool,
    rounded_matte: Option<Arc<gpui::RenderImage>>,
    backdrop_tiles: Vec<crate::editor::export::PreviewEdgeImage>,
    preview_edges: Vec<crate::editor::export::PreviewEdgeImage>,
    image_tiles: Vec<crate::editor::export::PreviewEdgeImage>,
    retired_preview_edges: Vec<Arc<gpui::RenderImage>>,
    backdrop_key: Option<BackdropKey>,
    rendered_backdrop_key: Option<BackdropKey>,
    live_background_cache: Option<(LiveBackgroundKey, Arc<gpui::RenderImage>)>,
    backdrop_in_flight: bool,
    backdrop_pending: bool,
    inset_color: Option<String>,
    /// The stage's scroll position, which ctrl-drag pans.
    stage_scroll: gpui::ScrollHandle,
    /// Pointer position and scroll offset a pan started from.
    pan_origin: Option<(gpui::Point<Pixels>, gpui::Point<Pixels>)>,
    next_id: u32,
    editor_state_writer: Arc<EditorStateWriter>,
    requested_editor_state: Option<PersistedEditorState>,
    editor_state_write_task: Option<gpui::Task<()>>,
    pub focus_handle: gpui::FocusHandle,
    save_dialog_open: bool,
    bounds_sub: Option<Subscription>,
    activation_sub: Option<Subscription>,
}

struct PresetDraft {
    field: gpui::Entity<InputState>,
    _subscription: Subscription,
}

pub type SnapshotCell = Rc<RefCell<CanvasSnapshot>>;

pub(crate) struct PreparedScreenshot {
    base: Arc<image::DynamicImage>,
    image: Arc<gpui::RenderImage>,
    tiles: Vec<crate::editor::export::PreviewEdgeImage>,
}

impl PreparedScreenshot {
    pub(crate) fn open(path: &str) -> image::ImageResult<Self> {
        let decoded = crate::render::images::open_rgba8(path)?;
        let base = Arc::new(image::DynamicImage::ImageRgba8(decoded));
        let image = render_image_from_base(&base);
        let tiles = crate::editor::image_tiles::create(&image);
        Ok(Self { base, image, tiles })
    }

    pub(crate) fn dimensions(&self) -> (f32, f32) {
        (self.base.width() as f32, self.base.height() as f32)
    }
}

impl EditorWindow {
    #[cfg(any(test, feature = "parity"))]
    pub fn from_file(path: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::from_prepared(path, None, load_editor_image(path), window, cx)
    }

    #[cfg(test)]
    pub(crate) fn from_file_with_state(
        path: &str,
        persisted: PersistedEditorState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::from_prepared(path, Some(persisted), load_editor_image(path), window, cx)
    }

    pub(crate) fn from_prepared(
        path: &str,
        persisted: Option<PersistedEditorState>,
        prepared: Option<PreparedScreenshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let persisted =
            persisted.unwrap_or_else(|| PersistedEditorState::load_for_screenshot(path));
        let (width, height) = prepared
            .as_ref()
            .map_or((0., 0.), PreparedScreenshot::dimensions);
        let (base_image, image, image_tiles) = prepared.map_or_else(
            || (None, None, Vec::new()),
            |prepared| (Some(prepared.base), Some(prepared.image), prepared.tiles),
        );
        let prefs = crate::state::try_state(cx)
            .map(|service| service.config.get().editor)
            .unwrap_or_default();
        let persisted_snapshot = persisted.clone();
        let next_id = persisted
            .annotations
            .iter()
            .filter_map(|annotation| annotation.id().strip_prefix("ann-"))
            .filter_map(|id| id.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let image_opaque = image
            .as_ref()
            .and_then(|image| image.as_bytes(0))
            .is_some_and(|bytes| bytes.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));
        let mut editor = Self {
            image,
            base_image,
            base_pixmap: None,
            layers: persisted.layers,
            capture_mode: false,
            meta_held: false,
            drop_edge: None,
            balance_crop: None,
            selected_annotations: Vec::new(),
            move_origin: None,
            move_original: None,
            resize_state: None,
            marquee: None,
            annotation_clipboard: Vec::new(),
            backdrop: None,
            backdrop_composite: false,
            rounded_matte: None,
            backdrop_tiles: Vec::new(),
            preview_edges: Vec::new(),
            image_tiles,
            retired_preview_edges: Vec::new(),
            backdrop_key: None,
            rendered_backdrop_key: None,
            live_background_cache: None,
            backdrop_in_flight: false,
            backdrop_pending: false,
            inset_color: None,
            stage_scroll: gpui::ScrollHandle::new(),
            pan_origin: None,
            is_copied: false,
            copy_pending: false,
            pin_pending: false,
            copy_task: None,
            image_width: width,
            image_height: height,
            image_opaque,
            file_path: path.to_string(),
            tool: Tool::from_id(&prefs.last_tool)
                .filter(|tool| *tool != Tool::Wallpaper)
                .unwrap_or(Tool::Select),
            color_hex: prefs.color.clone(),
            stroke_width: prefs.stroke_width,
            arrow_style: prefs.arrow_style.clone(),
            highlight_color: prefs.highlight_color.clone(),
            highlight_opacity: prefs.highlight_opacity,
            number_style: prefs.number_style.clone(),
            number_size: prefs.number_size.clone(),
            number_start_value: prefs.number_start_value,
            text_background: prefs.text_background,
            text_font_size: prefs.text_font_size,
            text_font_family: prefs.text_font_family.clone(),
            redact_style: prefs.redact_style.clone(),
            redact_intensity: prefs.redact_intensity,
            shape_fill_mode: prefs.shape_fill_mode.clone(),
            zoom: 1.0,
            history: AnnotationHistory::new(persisted.annotations),
            draft: None,
            drag_start: None,
            pending_pointer_update: None,
            pending_stroke_points: Vec::new(),
            pending_stroke_shift: false,
            pointer_update_scheduled: false,
            text_editor: None,
            crop: None,
            crop_drag: None,
            pre_crop: None,
            zoom_bar_bounds: Rc::new(RefCell::new(None)),
            fit_for: None,
            zoom_backdrop: None,
            wallpaper: persisted.wallpaper,
            native_scale: crate::editor::wallpaper::native_scale(
                f64::from(width),
                f64::from(height),
                f64::from(window.scale_factor()),
            ),
            window_scale: f64::from(window.scale_factor()),
            wallpaper_preset_id: String::new(),
            background_editor: None,
            background_editor_generation: 0,
            background_picker_open: false,
            background_previews: crate::editor::background::BackgroundPreviews::default(),
            preset_draft: None,
            sheet_closing: false,
            sheet_close_task: None,
            cloud_upload: crate::cloud::UploadState::Idle,
            redact_patches: std::collections::HashMap::new(),
            snapshot: Rc::new(RefCell::new(CanvasSnapshot {
                image: None,
                image_tiles: Vec::new(),
                redact_patches: std::collections::HashMap::new(),
                image_width: width,
                image_height: height,
                image_opaque,
                zoom: 1.0,
                annotations: Vec::new(),
                draft: None,
                tool: Tool::from_id(&prefs.last_tool)
                    .filter(|tool| *tool != Tool::Wallpaper)
                    .unwrap_or(Tool::Select),
                color_hex: prefs.color.clone(),
                stroke_width: prefs.stroke_width,
                crop: None,
                wallpaper: crate::editor::wallpaper::WallpaperSettings::default(),
                frame_scale: 1.0,
                backdrop: None,
                backdrop_composite: false,
                rounded_matte: None,
                backdrop_tiles: Vec::new(),
                preview_edges: Vec::new(),
                live_background: LiveBackground::None,
                inset_color: None,
                selected: Vec::new(),
                moving: false,
                resizing: false,
                editing_text: None,
                marquee: None,
                balance_crop: None,
                layers: Vec::new(),
                spacing: 0.0,
            })),
            bounds: Rc::new(RefCell::new(None)),
            next_id,
            editor_state_writer: Arc::new(EditorStateWriter {
                revision: 0.into(),
                persisted: parking_lot::Mutex::new(Some(persisted_snapshot.clone())),
            }),
            requested_editor_state: Some(persisted_snapshot),
            editor_state_write_task: None,
            focus_handle: cx.focus_handle(),
            save_dialog_open: false,
            bounds_sub: None,
            activation_sub: None,
        };
        editor.activation_sub = Some(cx.observe_window_activation(window, |editor, window, cx| {
            if window.is_window_active() || !editor.meta_held {
                return;
            }
            editor.meta_held = false;
            cx.notify();
        }));
        editor.bounds_sub = Some(cx.observe_window_bounds(window, |_, window, _cx| {
            if window.is_maximized() {
                return;
            }
            let size = window.bounds().size;
            crate::config::window_state::set(
                crate::config::window_state::SCREENSHOT_EDITOR,
                f32::from(size.width),
                f32::from(size.height),
            );
        }));
        if editor.layers.is_empty()
            || editor.wallpaper.supports_edge_preview(true)
            || crate::render::window_frame::theme(&editor.wallpaper.window_frame.style).is_some()
        {
            editor.refresh_balance_crop();
        }
        editor.refresh_raster_patches();
        editor.sync_snapshot(cx);
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |editor, _| {
                editor.force_persist_editor_state();
                editor.release_gpu_resources(window);
            });
            crate::system::memory::reclaim_idle_memory();
            true
        });
        window.focus(&editor.focus_handle, cx);
        editor
    }

    fn release_gpu_resources(&mut self, window: &mut Window) {
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        if let Some(image) = self.backdrop.take() {
            let _ = window.drop_image(image);
        }
        if let Some(image) = self.rounded_matte.take() {
            let _ = window.drop_image(image);
        }
        for edge in self.preview_edges.drain(..) {
            let _ = window.drop_image(edge.image);
        }
        for image in self.retired_preview_edges.drain(..) {
            let _ = window.drop_image(image);
        }
        for tile in self.backdrop_tiles.drain(..) {
            let _ = window.drop_image(tile.image);
        }
        for tile in self.image_tiles.drain(..) {
            let _ = window.drop_image(tile.image);
        }
        for (_, patch) in self.redact_patches.drain() {
            let _ = window.drop_image(patch.image);
        }
        if let Some((_, image)) = self.live_background_cache.take() {
            let _ = window.drop_image(image);
        }
        if let Some(zoom_backdrop) = self.zoom_backdrop.take() {
            let _ = window.drop_image(zoom_backdrop.image());
        }
        for image in self.background_previews.drain_images() {
            let _ = window.drop_image(image);
        }
        if let Some(preview) = self
            .background_editor
            .take()
            .and_then(|draft| draft.preview)
        {
            let _ = window.drop_image(preview);
        }
        crate::editor::preview_raster::clear(window);
        crate::editor::export::clear_caches();
        crate::editor::wallpaper_svg::clear_pixmap_cache();
        self.base_image = None;
        self.base_pixmap = None;
        self.backdrop_key = None;
        self.backdrop_pending = false;
        self.rendered_backdrop_key = None;
        self.pre_crop = None;
        let mut snapshot = self.snapshot.borrow_mut();
        snapshot.image = None;
        snapshot.backdrop = None;
        snapshot.rounded_matte = None;
        snapshot.preview_edges.clear();
        snapshot.backdrop_tiles.clear();
        snapshot.image_tiles.clear();
        snapshot.redact_patches.clear();
        snapshot.live_background = LiveBackground::None;
        snapshot.inset_color = None;
    }

    /// Rasterizes the redacted pixels for every committed redaction once, so
    /// the canvas can show the exported result instead of a placeholder.
    /// Rotated text is rasterized the same way: GPUI cannot rotate elements,
    /// so the export renderer draws those annotations offscreen.
    fn refresh_raster_patches(&mut self) {
        if let Some(base) = self.base_image.clone() {
            let mut live = std::collections::HashMap::new();
            for annotation in self.history.current() {
                let Annotation::Redact {
                    id,
                    x,
                    y,
                    width,
                    height,
                    style,
                    intensity,
                } = annotation
                else {
                    continue;
                };
                if style == "pixelate" || style == "blur" {
                    continue;
                }
                let key = format!(
                    "{id}:{x}:{y}:{width}:{height}:{style}:{intensity}:{}",
                    self.native_scale
                );
                if let Some(existing) = self.redact_patches.get(&key) {
                    live.insert(key, existing.clone());
                    continue;
                }
                if let Some(patch) = render_redact_patch(
                    &base,
                    (*x, *y, *width, *height),
                    style,
                    *intensity,
                    self.native_scale,
                ) {
                    live.insert(key, patch);
                }
            }
            let previous = std::mem::replace(&mut self.redact_patches, live);
            for (key, patch) in previous {
                if !self.redact_patches.contains_key(&key) {
                    self.retired_preview_edges.push(patch.image);
                }
            }
        }
    }

    fn redact_patches_by_id(
        &self,
    ) -> std::collections::HashMap<String, crate::editor::canvas::RedactPatch> {
        self.history
            .current()
            .iter()
            .filter_map(|annotation| {
                let Annotation::Redact {
                    id,
                    x,
                    y,
                    width,
                    height,
                    style,
                    intensity,
                } = annotation
                else {
                    return None;
                };
                let key = format!(
                    "{id}:{x}:{y}:{width}:{height}:{style}:{intensity}:{}",
                    self.native_scale
                );
                self.redact_patches
                    .get(&key)
                    .map(|patch| (id.clone(), patch.clone()))
            })
            .collect()
    }

    /// Recomputes the balance crop, which only changes when the option is
    /// toggled or a different image is loaded.
    fn refresh_balance_crop(&mut self) {
        if !self.wallpaper.balance {
            if self.balance_crop.take().is_some() {
                self.inset_color = None;
            }
            return;
        }
        if self.balance_crop.is_some() {
            return;
        }
        let Some(base) = self.base_image.as_ref() else {
            return;
        };
        let bounds = match base.as_rgba8() {
            Some(rgba) => crate::editor::export::balance_bounds(rgba),
            None => crate::editor::export::balance_bounds(&base.to_rgba8()),
        };
        let crop = bounds.map(|bounds| {
            (
                bounds.left as f32,
                bounds.top as f32,
                bounds.right as f32,
                bounds.bottom as f32,
            )
        });
        if self.balance_crop != crop {
            self.inset_color = None;
        }
        self.balance_crop = crop;
    }

    fn retire_preview_edges(&mut self) {
        self.retired_preview_edges
            .extend(self.preview_edges.drain(..).map(|edge| edge.image));
    }

    fn replace_image(&mut self, image: Arc<gpui::RenderImage>) {
        self.retired_preview_edges
            .extend(self.image_tiles.drain(..).map(|tile| tile.image));
        self.image_opaque = image
            .as_bytes(0)
            .is_some_and(|bytes| bytes.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));
        if let Some(previous) = self.image.replace(image.clone()) {
            self.retired_preview_edges.push(previous);
        }
        self.image_tiles = crate::editor::image_tiles::create(&image);
    }

    fn retire_backdrop(&mut self) {
        if let Some(backdrop) = self.backdrop.take() {
            self.retired_preview_edges.push(backdrop);
        }
        if let Some(matte) = self.rounded_matte.take() {
            self.retired_preview_edges.push(matte);
        }
        self.backdrop_composite = false;
        self.rendered_backdrop_key = None;
        self.retired_preview_edges
            .extend(self.backdrop_tiles.drain(..).map(|tile| tile.image));
        self.retire_preview_edges();
    }

    fn retire_live_background(&mut self) {
        if let Some((_, image)) = self.live_background_cache.take() {
            self.retired_preview_edges.push(image);
        }
    }

    fn replace_live_background(&mut self, key: LiveBackgroundKey, image: Arc<gpui::RenderImage>) {
        if let Some((_, previous)) = self.live_background_cache.replace((key, image)) {
            self.retired_preview_edges.push(previous);
        }
    }

    fn ensure_inset_color(&mut self) {
        if self.wallpaper.inset <= 0.0 || self.inset_color.is_some() {
            return;
        }
        let balance_crop = self.balance_crop_bounds().unwrap_or_default();
        self.inset_color = self
            .base_image
            .as_ref()
            .and_then(|base| match base.as_rgba8() {
                Some(rgba) => {
                    crate::render::color_detection::dominant_inset_color_rgba(rgba, balance_crop)
                }
                None => {
                    let pixmap = crate::editor::export::from_dynamic_image(base)?;
                    crate::render::color_detection::dominant_inset_color(
                        pixmap.as_ref(),
                        balance_crop,
                    )
                }
            });
    }

    fn balance_crop_bounds(&self) -> Option<crate::render::color_detection::ContentBounds> {
        self.balance_crop.map(|(left, top, right, bottom)| {
            crate::render::color_detection::ContentBounds {
                left: left as u32,
                top: top as u32,
                right: right as u32,
                bottom: bottom as u32,
            }
        })
    }

    fn find_background_preview(
        &self,
        source: Option<&str>,
        cx: &App,
    ) -> Option<Arc<gpui::RenderImage>> {
        let source = source?;
        let customs = crate::state::try_state(cx)
            .map(|service| service.config.get().wallpaper.custom_backgrounds)
            .unwrap_or_default();
        for custom in &customs {
            if let crate::config::schema::CustomBackgroundData::Image { data } = &custom.data {
                if data.image_url == source {
                    if let crate::editor::background::PreviewState::Ready(image) =
                        self.background_previews.custom(&custom.id)
                    {
                        return Some(image);
                    }
                    return None;
                }
            }
        }
        if self.background_previews.desktop_source() == Some(source) {
            if let crate::editor::background::PreviewState::Ready(image) =
                self.background_previews.desktop()
            {
                return Some(image);
            }
        }
        None
    }

    fn resolve_live_background(
        &self,
        wallpaper: &crate::editor::wallpaper::WallpaperSettings,
        cx: &App,
    ) -> LiveBackground {
        if !wallpaper.has_background() {
            return LiveBackground::None;
        }
        let source = LiveBackgroundSource::from_wallpaper(wallpaper);
        if let Some((cached, image)) = &self.live_background_cache {
            if cached.source == source {
                let cover = wallpaper.background_image.is_some()
                    || wallpaper
                        .gradient
                        .as_ref()
                        .is_some_and(|g| crate::editor::wallpaper_svg::is_preset(&g.id));
                return LiveBackground::Image {
                    image: image.clone(),
                    cover,
                };
            }
        }
        if let Some(background_image) = wallpaper.background_image.as_deref() {
            if let Some(image) = self.find_background_preview(Some(background_image), cx) {
                return LiveBackground::Image { image, cover: true };
            }
            return LiveBackground::Solid(
                crate::theme::color::Srgba::from_hex("#18181b").to_hsla(),
            );
        }
        let Some(preset) = wallpaper.gradient.as_ref() else {
            return LiveBackground::None;
        };
        if crate::editor::wallpaper_svg::is_preset(&preset.id) {
            if let Some(image) = crate::editor::wallpaper_svg::preview_render_image(&preset.id) {
                return LiveBackground::Image { image, cover: true };
            }
        }
        if preset.colors.len() >= 2 {
            let from = crate::theme::color::Srgba::parse(
                preset
                    .colors
                    .first()
                    .map(String::as_str)
                    .unwrap_or("#000000"),
            )
            .to_hsla();
            let to = crate::theme::color::Srgba::parse(
                preset
                    .colors
                    .last()
                    .map(String::as_str)
                    .unwrap_or("#ffffff"),
            )
            .to_hsla();
            return LiveBackground::LinearGradient {
                angle: preset.angle as f32,
                from,
                to,
            };
        }
        if let Some(single) = preset.colors.first() {
            return LiveBackground::Solid(crate::theme::color::Srgba::parse(single).to_hsla());
        }
        LiveBackground::None
    }

    fn compute_backdrop_plan(&self, cx: &App) -> Option<(BackdropKey, f64, f64, [u8; 3], bool)> {
        let wallpaper = self.wallpaper.for_scale(self.native_scale);
        if !wallpaper.is_active_with_layers(!self.layers.is_empty()) {
            return None;
        }
        // The frame surrounds the whole group, so the layout is computed from
        // the attached layers' bounds rather than the capture alone.
        let (image_width, image_height) = crate::editor::wallpaper::balanced_size(
            f64::from(self.image_width),
            f64::from(self.image_height),
            self.balance_crop,
            !self.layers.is_empty()
                && !wallpaper.supports_edge_preview(true)
                && crate::render::window_frame::theme(&wallpaper.window_frame.style).is_none(),
        );
        let title_bar = if self.layers.is_empty() {
            0.0
        } else {
            crate::render::window_frame::title_bar_height(
                &wallpaper.window_frame.style,
                self.native_scale,
            )
        };
        let group = crate::editor::layers::compute_with_title_bar(
            image_width,
            image_height,
            &self.layers,
            wallpaper.spacing.max(0.0),
            wallpaper.layer_inset(!self.layers.is_empty()),
            title_bar,
        );
        let ((width, height), _) = crate::editor::wallpaper::layout(
            &wallpaper,
            group.width,
            group.height,
            self.native_scale,
            !self.layers.is_empty(),
        );
        let size = (
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
        );
        #[cfg(target_os = "macos")]
        let rounded_preview = (size.0 > 16384 || size.1 > 16384)
            && self.layers.is_empty()
            && !wallpaper.balance
            && wallpaper.corners > 0.0
            && wallpaper.shadow <= 0.0
            && wallpaper.inset <= 0.0
            && crate::render::window_frame::theme(&wallpaper.window_frame.style).is_none();
        #[cfg(not(target_os = "macos"))]
        let rounded_preview = false;
        let background = gpui::Rgba::from(active_theme(cx).background);
        let stage_background = [background.r, background.g, background.b]
            .map(|channel| (channel * 255.0).round() as u8);
        let key = (
            wallpaper.clone(),
            size.0,
            size.1,
            self.layers.clone(),
            self.native_scale,
            (!wallpaper.has_background()
                || wallpaper.supports_edge_preview(!self.layers.is_empty())
                || crate::editor::export::composites_wallpaper_effects(&wallpaper, &self.layers)
                || ((size.0 > 16384 || size.1 > 16384)
                    && crate::editor::export::composites_primary_backdrop(
                        &wallpaper,
                        &self.layers,
                    )))
            .then_some(stage_background),
            (rounded_preview
                || wallpaper.inset > 0.0
                || crate::editor::export::composites_primary_backdrop(&wallpaper, &self.layers)
                || (!self.layers.is_empty() && wallpaper.supports_edge_preview(true))
                || (!self.layers.is_empty()
                    && crate::render::window_frame::theme(&wallpaper.window_frame.style)
                        .is_some()))
            .then(|| self.image.as_ref().map(|image| image.id))
            .flatten(),
            self.window_scale,
        );
        Some((
            key,
            image_width,
            image_height,
            stage_background,
            rounded_preview,
        ))
    }

    /// Renders the wallpaper backdrop when the settings or the image size have
    /// changed since the last one.
    fn refresh_backdrop(&mut self, cx: &mut Context<Self>) {
        let Some((key, image_width, image_height, stage_background, rounded_preview)) =
            self.compute_backdrop_plan(cx)
        else {
            self.backdrop_pending = self.backdrop_in_flight;
            self.retire_backdrop();
            self.retire_live_background();
            self.backdrop_key = None;
            return;
        };
        let wallpaper = key.0.clone();
        let size = (key.1, key.2);
        self.ensure_inset_color();
        if !wallpaper.has_background() {
            self.retire_live_background();
        } else {
            let source = LiveBackgroundSource::from_wallpaper(&wallpaper);
            if self
                .live_background_cache
                .as_ref()
                .is_some_and(|(cached, _)| cached.source != source)
            {
                self.retire_live_background();
            }
        }
        let can_live_switch_source = !wallpaper.has_background()
            || wallpaper.gradient.is_some()
            || self
                .find_background_preview(wallpaper.background_image.as_deref(), cx)
                .is_some();
        let should_retire = |previous: &BackdropKey| {
            backdrop_layout_changed(previous, &key)
                || (backdrop_source_changed(previous, &key) && can_live_switch_source)
        };
        if self.backdrop_in_flight {
            if self
                .rendered_backdrop_key
                .as_ref()
                .is_some_and(should_retire)
            {
                self.retire_backdrop();
            }
            if self.backdrop_key.as_ref() != Some(&key) {
                self.backdrop_pending = true;
            }
            return;
        }
        if self.backdrop_key.as_ref() == Some(&key) {
            return;
        }
        if self
            .rendered_backdrop_key
            .as_ref()
            .or(self.backdrop_key.as_ref())
            .is_some_and(should_retire)
        {
            self.retire_backdrop();
        }
        let task_key = key.clone();
        self.backdrop_key = Some(key);
        let frame_scale = self.native_scale;
        let layers = self.layers.clone();
        let inset_color = self.inset_color.clone();
        let balance_bounds = self.balance_crop_bounds();
        let base_image = self.base_image.clone();
        let live_key = LiveBackgroundKey::from_wallpaper(&wallpaper);
        let include_live_background = self
            .live_background_cache
            .as_ref()
            .is_none_or(|(cached, _)| *cached != live_key);
        let cached_base_pixmap = base_image.as_ref().and_then(|base| {
            let (weak, pixmap, opaque) = self.base_pixmap.as_ref()?;
            weak.upgrade()
                .filter(|cached| Arc::ptr_eq(cached, base))
                .map(|_| (pixmap.clone(), *opaque))
        });
        let needs_primary_pixmap =
            crate::editor::export::backdrop_needs_primary_pixmap(&wallpaper, &layers)
                || rounded_preview;
        let task = cx.background_executor().spawn(async move {
            let computed_base_pixmap = if cached_base_pixmap.is_none() && needs_primary_pixmap {
                base_image
                    .as_deref()
                    .and_then(crate::render::gradient::premultiply_dynamic_image)
                    .map(|(pixmap, opaque)| (Arc::new(pixmap), opaque))
            } else {
                None
            };
            let primary_pixmap = cached_base_pixmap
                .as_ref()
                .or(computed_base_pixmap.as_ref());
            let rendered = crate::editor::export::render_backdrop_cached(
                &wallpaper,
                size.0,
                size.1,
                image_width,
                image_height,
                inset_color.clone(),
                &layers,
                base_image.as_deref(),
                primary_pixmap.map(|(pixmap, opaque)| (pixmap.as_ref(), *opaque)),
                balance_bounds.map(Some),
                frame_scale,
                stage_background,
                include_live_background,
            );
            #[cfg(target_os = "macos")]
            let (rendered, rounded_matte) = {
                let mut rendered = rendered;
                let matte = if rounded_preview {
                    match (rendered.as_mut(), primary_pixmap, base_image.as_ref()) {
                        (Some(result), Some((primary, opaque)), _)
                            if !result.composite && !result.preview_edges.is_empty() =>
                        {
                            crate::editor::export::render_rounded_preview_with_pixmap(
                                primary,
                                *opaque,
                                &wallpaper,
                                frame_scale,
                            )
                            .map(|composed| std::mem::replace(&mut result.backdrop, composed))
                        }
                        (Some(result), None, Some(base))
                            if !result.composite && !result.preview_edges.is_empty() =>
                        {
                            crate::editor::export::render_rounded_preview(
                                base,
                                &wallpaper,
                                frame_scale,
                            )
                            .map(|composed| std::mem::replace(&mut result.backdrop, composed))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                (rendered, matte)
            };
            #[cfg(not(target_os = "macos"))]
            let rounded_matte = {
                let _ = rounded_preview;
                None
            };
            #[cfg(target_os = "macos")]
            let backdrop_tiles = rendered
                .as_ref()
                .filter(|result| !result.composite)
                .map(|result| crate::editor::image_tiles::create(&result.backdrop))
                .unwrap_or_default();
            #[cfg(not(target_os = "macos"))]
            let backdrop_tiles = Vec::new();
            let new_base_pixmap = computed_base_pixmap
                .zip(base_image.as_ref().map(Arc::downgrade))
                .map(|((pixmap, opaque), weak)| (weak, pixmap, opaque));
            (
                rendered,
                inset_color,
                backdrop_tiles,
                rounded_matte,
                new_base_pixmap,
            )
        });
        self.backdrop_in_flight = true;
        cx.spawn(async move |entity, cx| {
            let (rendered, inset_color, backdrop_tiles, rounded_matte, new_base_pixmap) =
                task.await;
            let _ = entity.update(cx, |editor, cx| {
                editor.backdrop_in_flight = false;
                if editor.base_image.is_none() && editor.image.is_none() {
                    editor.backdrop_pending = false;
                    return;
                }
                if let Some(entry) = new_base_pixmap {
                    if editor.base_image.as_ref().is_some_and(|current| {
                        entry
                            .0
                            .upgrade()
                            .is_some_and(|cached| Arc::ptr_eq(&cached, current))
                    }) {
                        editor.base_pixmap = Some(entry);
                    }
                }
                let pending = std::mem::take(&mut editor.backdrop_pending);
                let current_key = editor.compute_backdrop_plan(cx).map(|(key, ..)| key);
                let compatible = current_key.as_ref().is_some_and(|current| {
                    !backdrop_layout_changed(&task_key, current)
                        && !backdrop_source_changed(&task_key, current)
                });
                if compatible {
                    let (backdrop, preview_edges, composite, live_bg) = rendered
                        .map(|result| {
                            (
                                Some(result.backdrop),
                                result.preview_edges,
                                result.composite,
                                result.live_background,
                            )
                        })
                        .unwrap_or_default();
                    editor.retire_backdrop();
                    editor.backdrop = backdrop;
                    editor.backdrop_composite = composite;
                    editor.rounded_matte = rounded_matte;
                    editor.backdrop_tiles = backdrop_tiles;
                    editor.preview_edges = preview_edges;
                    editor.rendered_backdrop_key = Some(task_key.clone());
                    if let Some(live_bg) = live_bg {
                        editor.replace_live_background(
                            LiveBackgroundKey::from_wallpaper(&task_key.0),
                            live_bg,
                        );
                    }
                    editor.inset_color = inset_color;
                    cx.notify();
                } else if let Some(result) = rendered {
                    if let Some(live_bg) = result.live_background {
                        let live_key = LiveBackgroundKey::from_wallpaper(&task_key.0);
                        if current_key.as_ref().is_some_and(|current| {
                            LiveBackgroundSource::from_wallpaper(&current.0) == live_key.source
                        }) {
                            editor.replace_live_background(live_key, live_bg);
                            cx.notify();
                        }
                    }
                }
                if pending || current_key.as_ref() != Some(&task_key) {
                    editor.backdrop_key = None;
                    editor.refresh_backdrop(cx);
                }
            });
        })
        .detach();
    }

    fn sync_snapshot(&mut self, cx: &mut Context<Self>) {
        let wallpaper = self.wallpaper.for_scale(self.native_scale);
        let live_background = self.resolve_live_background(&wallpaper, cx);
        {
            let mut snap = self.snapshot.borrow_mut();
            snap.redact_patches = self.redact_patches_by_id();
            snap.crop = self.crop;
            snap.wallpaper = wallpaper.clone();
            snap.frame_scale = self.native_scale;
            snap.backdrop = self.backdrop.clone();
            snap.backdrop_composite = self.backdrop_composite;
            snap.rounded_matte = self.rounded_matte.clone();
            snap.backdrop_tiles = self.backdrop_tiles.clone();
            snap.preview_edges = self.preview_edges.clone();
            snap.live_background = live_background;
            snap.inset_color = self.inset_color.clone();
            snap.selected = self.selected_annotations.clone();
            snap.moving = self.move_original.is_some();
            snap.resizing = self
                .resize_state
                .as_ref()
                .is_some_and(|state| state.original.is_some());
            snap.editing_text = self.text_editor.as_ref().map(|(id, _)| id.clone());
            snap.marquee = self.marquee.map(|(start, current)| {
                let left = f64::from(start.x).min(f64::from(current.x));
                let top = f64::from(start.y).min(f64::from(current.y));
                let right = f64::from(start.x).max(f64::from(current.x));
                let bottom = f64::from(start.y).max(f64::from(current.y));
                (left, top, right - left, bottom - top)
            });
            snap.balance_crop = self.balance_crop;
            snap.layers = self.layers.clone();
            snap.spacing = wallpaper.spacing;
            snap.image = self.image.clone();
            snap.image_tiles = self.image_tiles.clone();
            snap.image_width = self.image_width;
            snap.image_height = self.image_height;
            snap.image_opaque = self.image_opaque;
            snap.zoom = self.zoom;
            snap.annotations = self.history.current().to_vec();
            snap.draft = self.draft.clone();
            snap.tool = self.tool;
            snap.color_hex = self.color_hex.clone();
            snap.stroke_width = self.stroke_width;
        }
        self.persist_editor_state(cx);
    }

    fn is_interactive_transform(&self) -> bool {
        self.move_original.is_some()
            || self
                .resize_state
                .as_ref()
                .is_some_and(|state| state.original.is_some())
            || self.marquee.is_some()
            || self.crop_drag.is_some()
            || (self.drag_start.is_some() && (self.draft.is_some() || self.tool == Tool::Crop))
    }

    fn sync_interactive_snapshot(&mut self) {
        let mut snap = self.snapshot.borrow_mut();
        let current = self.history.current();
        if snap.annotations.len() == current.len()
            && snap
                .annotations
                .iter()
                .zip(current.iter())
                .all(|(a, b)| a.id() == b.id())
        {
            if self.move_original.is_some() || self.resize_state.is_some() {
                for (dst, src) in snap.annotations.iter_mut().zip(current.iter()) {
                    if self.selected_annotations.iter().any(|id| dst.id() == id) {
                        if self.move_original.is_some() {
                            dst.set_translated_from(src, 0.0, 0.0);
                        } else {
                            *dst = src.clone();
                        }
                    }
                }
            }
        } else {
            snap.annotations = current.to_vec();
        }
        snap.draft = self.draft.clone();
        snap.crop = self.crop;
        snap.marquee = self.marquee.map(|(start, current)| {
            let left = f64::from(start.x).min(f64::from(current.x));
            let top = f64::from(start.y).min(f64::from(current.y));
            let right = f64::from(start.x).max(f64::from(current.x));
            let bottom = f64::from(start.y).max(f64::from(current.y));
            (left, top, right - left, bottom - top)
        });
        snap.selected = self.selected_annotations.clone();
        snap.moving = self.move_original.is_some();
        snap.resizing = self
            .resize_state
            .as_ref()
            .is_some_and(|state| state.original.is_some());
    }

    fn current_persisted_state(&self) -> PersistedEditorState {
        PersistedEditorState {
            annotations: self.history.current().to_vec(),
            wallpaper: self.wallpaper.clone(),
            layers: self.layers.clone(),
        }
    }

    fn persist_editor_state(&mut self, cx: &mut Context<Self>) {
        if self.file_path.is_empty()
            || self.draft.is_some()
            || self.move_origin.is_some()
            || self.resize_state.is_some()
        {
            return;
        }
        if self.requested_editor_state.as_ref().is_some_and(|prev| {
            prev.wallpaper == self.wallpaper
                && prev.layers == self.layers
                && prev.annotations == self.history.current()
        }) {
            return;
        }

        let state = self.current_persisted_state();
        self.requested_editor_state = Some(state.clone());
        let writer = self.editor_state_writer.clone();
        let revision = writer.next_revision();
        let path = std::path::PathBuf::from(&self.file_path);
        self.editor_state_write_task = Some(cx.spawn(async move |editor, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let written = cx
                .background_executor()
                .spawn(async move { writer.write(&path, state, revision) })
                .await;
            if !written {
                let _ = editor.update(cx, |editor, _| editor.requested_editor_state = None);
            }
        }));
    }

    fn force_persist_editor_state(&mut self) {
        self.editor_state_write_task.take();
        if self.file_path.is_empty() {
            return;
        }
        let state = self.current_persisted_state();
        let revision = self.editor_state_writer.next_revision();
        self.editor_state_writer
            .write(Path::new(&self.file_path), state, revision);
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub(crate) fn add_annotation_for_parity(&mut self, cx: &mut Context<Self>) {
        let mut annotations = self.history.current().to_vec();
        annotations.push(Annotation::Rectangle {
            id: self.annotation_id(),
            x: 24.,
            y: 24.,
            width: 80.,
            height: 60.,
            stroke: "#ff0000".into(),
            stroke_width: 3.,
            fill: None,
        });
        self.push_annotations(annotations, cx);
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn annotation_id(&mut self) -> String {
        self.next_id += 1;
        format!("ann-{}", self.next_id)
    }

    /// The topmost annotation under `point`, which is the last one drawn.
    fn annotation_at(&self, point: Point) -> Option<String> {
        crate::render::annotations::screenshot_annotation_at(
            self.history.current(),
            (f64::from(point.x), f64::from(point.y)),
            self.zoom,
        )
        .map(|annotation| annotation.id().to_string())
    }

    pub fn toggle_capture_mode(&mut self, cx: &mut Context<Self>) {
        self.capture_mode = !self.capture_mode;
        cx.notify();
    }

    /// Captures a fresh area and attaches it to one of the capture's edges —
    /// the `handleCaptureAndAttach` flow. The editor hides itself first so the
    /// new pixels never contain it, and the coordinator re-shows it once the
    /// capture lands or the overlay is dismissed.
    pub fn attach_layer(
        &mut self,
        edge: crate::editor::layers::Edge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let coordinator = crate::state::coordinator(cx);
        if coordinator.read(cx).has_pending_editor_attach() {
            return;
        }
        self.capture_mode = false;
        let handle = window.window_handle();
        let editor = cx.entity().downgrade();
        coordinator.update(cx, |coordinator, _| {
            coordinator.begin_editor_attach(handle, editor, edge);
        });
        crate::system::window_visibility::hide(window);
        crate::capture::start_area_selection(
            crate::capture::intent::CaptureIntent::EditorAttach,
            cx,
        );
        cx.notify();
    }

    pub fn push_image_layer(
        &mut self,
        image_url: String,
        natural_width: f64,
        natural_height: f64,
        edge: crate::editor::layers::Edge,
        cx: &mut Context<Self>,
    ) {
        let id = format!("layer-{}-{}", self.layers.len() + 1, self.next_layer_id());
        self.layers.push(crate::editor::layers::ImageLayer {
            id,
            image_url,
            natural_width,
            natural_height,
            edge,
        });
        self.refresh_backdrop(cx);
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn next_layer_id(&mut self) -> u32 {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    fn update_drop_edge(&mut self, position: gpui::Point<Pixels>, bounds: gpui::Bounds<Pixels>) {
        let edge = drop_edge_for_point(position, bounds);
        if self.drop_edge != Some(edge) {
            self.drop_edge = Some(edge);
        }
    }

    /// Attaches the first image from an OS file drop to the hovered edge —
    /// the `handleImageDrop` flow.
    fn handle_image_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let edge = self.drop_edge.take();
        let Some(edge) = edge else {
            cx.notify();
            return;
        };
        let Some(path) = paths.0.iter().find(|path| is_image_path(path)) else {
            cx.notify();
            return;
        };
        let bytes = std::fs::read(path).ok();
        let image = bytes
            .as_ref()
            .and_then(|bytes| image::load_from_memory(bytes).ok());
        let (Some(bytes), Some(image)) = (bytes, image) else {
            crate::windows::toast::Toast::show(
                cx,
                "Could not attach",
                "That file could not be read as an image.",
            );
            cx.notify();
            return;
        };
        use base64::Engine;
        self.push_image_layer(
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
            image.width() as f64,
            image.height() as f64,
            edge,
            cx,
        );
    }

    pub fn clear_layers(&mut self, cx: &mut Context<Self>) {
        if self.layers.is_empty() {
            return;
        }
        self.layers.clear();
        self.refresh_backdrop(cx);
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn update_crop_drag(&mut self, point: Point) {
        let Some(drag) = self.crop_drag else {
            return;
        };
        let image_width = f64::from(self.image_width);
        let image_height = f64::from(self.image_height);
        match drag {
            CropDrag::Move { start, rect } => {
                let dx = f64::from(point.x) - f64::from(start.x);
                let dy = f64::from(point.y) - f64::from(start.y);
                self.crop = Some((
                    (rect.0 + dx).clamp(0.0, image_width - rect.2),
                    (rect.1 + dy).clamp(0.0, image_height - rect.3),
                    rect.2,
                    rect.3,
                ));
            }
            CropDrag::Resize {
                corner,
                start,
                rect,
            } => {
                let dx = f64::from(point.x) - f64::from(start.x);
                let dy = f64::from(point.y) - f64::from(start.y);
                self.crop = Some(resize_crop_rect(
                    rect,
                    corner,
                    dx,
                    dy,
                    image_width,
                    image_height,
                ));
            }
        }
    }

    fn move_selected(&mut self, point: Point) {
        let Some(origin) = self.move_origin else {
            return;
        };
        if self.selected_annotations.is_empty() {
            return;
        }
        if point == origin {
            return;
        }
        if !self.history.current().iter().any(|annotation| {
            self.selected_annotations
                .iter()
                .any(|id| annotation.id() == id)
        }) {
            self.move_origin = Some(point);
            return;
        }
        let (start, original) = self
            .move_original
            .get_or_insert_with(|| (origin, self.history.current().to_vec()));
        let total_dx = (point.x - start.x) as f64;
        let total_dy = (point.y - start.y) as f64;
        for (dst, src) in self.history.current_mut().iter_mut().zip(original.iter()) {
            if self.selected_annotations.iter().any(|id| dst.id() == id) {
                dst.set_translated_from(src, total_dx, total_dy);
            }
        }
        self.move_origin = Some(point);
    }

    fn selected_handle_at(&self, point: Point) -> Option<ResizeHandle> {
        let [id] = self.selected_annotations.as_slice() else {
            return None;
        };
        let annotation = self
            .history
            .current()
            .iter()
            .find(|annotation| annotation.id() == id)?;
        annotation.handle_at(point, 6.0 / f64::from(self.zoom))
    }

    fn begin_resize(&mut self, handle: ResizeHandle, point: Point) {
        let [id] = self.selected_annotations.as_slice() else {
            return;
        };
        let id = id.clone();
        let Some(annotation) = self
            .history
            .current()
            .iter()
            .find(|annotation| annotation.id() == id)
            .cloned()
        else {
            return;
        };
        let mut state = ResizeState {
            handle,
            start: point,
            last: point,
            original: None,
            anchor: None,
            initial_drag: None,
            initial_font_size: None,
            initial_size: None,
            center: None,
            initial_rotation: None,
            start_angle: None,
        };
        match &annotation {
            Annotation::Rectangle {
                x,
                y,
                width,
                height,
                ..
            }
            | Annotation::Redact {
                x,
                y,
                width,
                height,
                ..
            } => {
                let (visual_left, visual_top, visual_right, visual_bottom) = (
                    width.min(0.0) + x,
                    height.min(0.0) + y,
                    width.max(0.0) + x,
                    height.max(0.0) + y,
                );
                let (anchor, drag) = match handle {
                    ResizeHandle::TopLeft => {
                        ((visual_right, visual_bottom), (visual_left, visual_top))
                    }
                    ResizeHandle::TopRight => {
                        ((visual_left, visual_bottom), (visual_right, visual_top))
                    }
                    ResizeHandle::BottomLeft => {
                        ((visual_right, visual_top), (visual_left, visual_bottom))
                    }
                    ResizeHandle::BottomRight => {
                        ((visual_left, visual_top), (visual_right, visual_bottom))
                    }
                    _ => return,
                };
                state.anchor = Some(anchor);
                state.initial_drag = Some(drag);
            }
            Annotation::Text { .. } => {
                let Some(text_box) = annotation.text_box() else {
                    return;
                };
                state.initial_font_size = match &annotation {
                    Annotation::Text { font_size, .. } => Some(*font_size),
                    _ => None,
                };
                state.initial_size = Some((text_box.width, text_box.height));
                state.center = Some((text_box.center_x, text_box.center_y));
                state.initial_rotation = Some(text_box.rotation);
                if handle == ResizeHandle::Rotate {
                    state.start_angle = Some(
                        (f64::from(point.y) - text_box.center_y)
                            .atan2(f64::from(point.x) - text_box.center_x),
                    );
                } else if handle != ResizeHandle::BottomRight {
                    return;
                }
            }
            Annotation::Circle { .. } => match handle {
                ResizeHandle::TopLeft
                | ResizeHandle::TopRight
                | ResizeHandle::BottomLeft
                | ResizeHandle::BottomRight => {}
                _ => return,
            },
            Annotation::Line { .. } => match handle {
                ResizeHandle::Start | ResizeHandle::End => {}
                _ => return,
            },
            Annotation::Arrow { .. } => match handle {
                ResizeHandle::Start | ResizeHandle::End | ResizeHandle::Bend => {}
                _ => return,
            },
            Annotation::Pen { .. } | Annotation::Highlight { .. } | Annotation::Number { .. } => {
                return;
            }
        }
        self.resize_state = Some(state);
    }

    fn resize_selected(&mut self, point: Point) {
        let Some(mut state) = self.resize_state.take() else {
            return;
        };
        let dx = f64::from(point.x) - f64::from(state.last.x);
        let dy = f64::from(point.y) - f64::from(state.last.y);
        let total_dx = f64::from(point.x) - f64::from(state.start.x);
        let total_dy = f64::from(point.y) - f64::from(state.start.y);
        if dx == 0.0 && dy == 0.0 {
            self.resize_state = Some(state);
            return;
        }
        if let [id] = self.selected_annotations.as_slice() {
            if let Some(index) = self
                .history
                .current()
                .iter()
                .position(|annotation| annotation.id() == id)
            {
                if state.original.is_none() {
                    let mut candidate = self.history.current()[index].clone();
                    apply_resize(&mut candidate, &state, dx, dy, total_dx, total_dy, point);
                    if candidate != self.history.current()[index] {
                        state.original = Some(self.history.current().to_vec());
                        self.history.current_mut()[index] = candidate;
                    }
                } else {
                    apply_resize(
                        &mut self.history.current_mut()[index],
                        &state,
                        dx,
                        dy,
                        total_dx,
                        total_dy,
                        point,
                    );
                }
            }
        }
        state.last = point;
        self.resize_state = Some(state);
    }

    pub fn delete_selected_annotation(&mut self, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        if self.selected_annotations.is_empty() {
            return;
        }
        let selected = std::mem::take(&mut self.selected_annotations);
        let mut annotations: Vec<Annotation> = self
            .history
            .current()
            .iter()
            .filter(|annotation| !selected.iter().any(|id| annotation.id() == id))
            .cloned()
            .collect();
        self.renumber_annotations(&mut annotations);
        self.push_annotations(annotations, cx);
        self.refresh_raster_patches();
        self.sync_snapshot(cx);
        cx.notify();
    }

    /// `handleCopyAnnotations` — returns false when nothing was selected, so
    /// the caller can fall back to copying the image.
    pub fn copy_selected_annotation(&mut self, cx: &mut Context<Self>) -> bool {
        self.finish_active_transform(cx);
        if self.selected_annotations.is_empty() {
            return false;
        }
        let copied: Vec<Annotation> = self
            .history
            .current()
            .iter()
            .filter(|annotation| {
                self.selected_annotations
                    .iter()
                    .any(|id| annotation.id() == id)
            })
            .cloned()
            .collect();
        if copied.is_empty() {
            return false;
        }
        self.annotation_clipboard = copied;
        true
    }

    pub fn cut_selected_annotation(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.copy_selected_annotation(cx) {
            return false;
        }
        self.delete_selected_annotation(cx);
        true
    }

    pub fn select_all_annotations(&mut self, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        if self.text_editor.is_some() {
            return;
        }
        self.selected_annotations = self
            .history
            .current()
            .iter()
            .map(|annotation| annotation.id().to_string())
            .collect();
        self.sync_snapshot(cx);
        cx.notify();
    }

    /// `handlePasteAnnotations` — a pasted copy is offset so it does not hide
    /// the original.
    pub fn paste_annotations(&mut self, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        if self.annotation_clipboard.is_empty() {
            return;
        }
        const PASTE_OFFSET: f64 = 12.0;
        let mut annotations = self.history.current().to_vec();
        let mut pasted_ids = Vec::new();
        for source in self.annotation_clipboard.clone() {
            let mut copy = source;
            let id = self.annotation_id();
            match &mut copy {
                Annotation::Pen { id: value, .. }
                | Annotation::Highlight { id: value, .. }
                | Annotation::Rectangle { id: value, .. }
                | Annotation::Circle { id: value, .. }
                | Annotation::Line { id: value, .. }
                | Annotation::Arrow { id: value, .. }
                | Annotation::Text { id: value, .. }
                | Annotation::Number { id: value, .. }
                | Annotation::Redact { id: value, .. } => *value = id.clone(),
            }
            copy.translate(PASTE_OFFSET, PASTE_OFFSET);
            pasted_ids.push(id);
            annotations.push(copy);
        }
        self.push_annotations(annotations, cx);
        self.selected_annotations = pasted_ids;
        self.refresh_raster_patches();
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn start_stroke(
        &mut self,
        point: Point,
        shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_active_transform(cx);
        let tool = self.tool;
        if tool != Tool::Crop {
            if let Some(handle) = self.selected_handle_at(point) {
                self.begin_resize(handle, point);
                self.sync_snapshot(cx);
                cx.notify();
                return;
            }
        }
        let (color, stroke_width) = if tool == Tool::Highlight {
            (self.highlight_color.clone(), self.stroke_width * 2.5)
        } else {
            (self.color_hex.clone(), self.stroke_width)
        };
        match tool {
            Tool::Pen => {
                self.drag_start = Some(point);
                self.draft = Some(Annotation::Pen {
                    id: self.annotation_id(),
                    points: vec![point.x as f64, point.y as f64],
                    stroke: color,
                    stroke_width,
                });
            }
            Tool::Highlight => {
                self.drag_start = Some(point);
                self.draft = Some(Annotation::Highlight {
                    id: self.annotation_id(),
                    points: vec![point.x as f64, point.y as f64],
                    fill: color,
                    opacity: self.highlight_opacity,
                    stroke_width,
                });
            }
            Tool::Number => {
                let value = self.next_number_value();
                self.drag_start = None;
                let annotation = Annotation::Number {
                    id: self.annotation_id(),
                    x: point.x as f64,
                    y: point.y as f64,
                    value,
                    display_value: crate::editor::options::number_display_value(
                        value,
                        &self.number_style,
                    ),
                    fill: color,
                    size: self.number_size.clone(),
                };
                let mut annotations = self.history.current().to_vec();
                annotations.push(annotation);
                self.push_annotations(annotations, cx);
            }
            Tool::Text => {
                self.drag_start = None;
                if let Some(id) = self.text_annotation_at(point) {
                    self.begin_text_reedit(&id, window, cx);
                    return;
                }
                self.begin_text(point, color, cx);
                self.focus_text_editor(window, cx);
            }
            Tool::Crop => {
                if let Some(rect) = self.crop {
                    let normalized = normalize_crop_rect(rect);
                    let tolerance = 8.0 / f64::from(self.zoom);
                    if let Some(corner) = crop_handle_at(normalized, point, tolerance) {
                        self.crop = Some(normalized);
                        self.crop_drag = Some(CropDrag::Resize {
                            corner,
                            start: point,
                            rect: normalized,
                        });
                    } else if point_in_crop_rect(normalized, point) {
                        self.crop = Some(normalized);
                        self.crop_drag = Some(CropDrag::Move {
                            start: point,
                            rect: normalized,
                        });
                    } else {
                        self.drag_start = Some(point);
                        self.crop = Some((
                            point.x.clamp(0.0, self.image_width) as f64,
                            point.y.clamp(0.0, self.image_height) as f64,
                            0.0,
                            0.0,
                        ));
                    }
                } else {
                    self.drag_start = Some(point);
                    self.crop = Some((
                        point.x.clamp(0.0, self.image_width) as f64,
                        point.y.clamp(0.0, self.image_height) as f64,
                        0.0,
                        0.0,
                    ));
                }
            }
            Tool::Redact => {
                self.drag_start = Some(point);
                self.draft = Some(Annotation::Redact {
                    id: self.annotation_id(),
                    x: point.x as f64,
                    y: point.y as f64,
                    width: 0.0,
                    height: 0.0,
                    style: self.redact_style.clone(),
                    intensity: self.redact_intensity,
                });
            }
            Tool::Rectangle | Tool::Circle => {
                self.drag_start = Some(point);
                let fill = (self.shape_fill_mode == "filled").then(|| color.clone());
                self.draft = Some(build_shape(
                    tool,
                    self.annotation_id(),
                    point,
                    point,
                    &color,
                    stroke_width,
                    fill,
                ));
            }
            Tool::Line | Tool::Arrow => {
                self.drag_start = Some(point);
                let arrow_style = (tool == Tool::Arrow).then(|| self.arrow_style.clone());
                self.draft = Some(build_segment(
                    tool,
                    self.annotation_id(),
                    point,
                    point,
                    &color,
                    stroke_width,
                    arrow_style,
                ));
            }
            Tool::Select => {
                if shift {
                    if let Some(id) = self.annotation_at(point) {
                        if let Some(index) = self
                            .selected_annotations
                            .iter()
                            .position(|selected| selected == &id)
                        {
                            self.selected_annotations.remove(index);
                        } else {
                            self.selected_annotations.push(id);
                        }
                    }
                } else if let Some(id) = self.annotation_at(point) {
                    if !self.selected_annotations.contains(&id) {
                        self.selected_annotations = vec![id];
                    }
                    self.move_origin = Some(point);
                    self.move_original = None;
                } else {
                    self.marquee = Some((point, point));
                }
            }
            _ => {}
        }
        self.sync_snapshot(cx);
    }

    fn extend_stroke(&mut self, point: Point) {
        let Some(start) = self.drag_start else {
            return;
        };
        let tool = self.tool;
        let color = self.color_hex.clone();
        let stroke_width = self.stroke_width;
        let shape_fill = (self.shape_fill_mode == "filled").then(|| color.clone());
        let arrow_style = self.arrow_style.clone();
        if tool == Tool::Crop {
            if let Some((x, y, width, height)) = &mut self.crop {
                let clamped_x = point.x.clamp(0.0, self.image_width) as f64;
                let clamped_y = point.y.clamp(0.0, self.image_height) as f64;
                *width = clamped_x - *x;
                *height = clamped_y - *y;
            }
            self.sync_interactive_snapshot();
            return;
        }

        match (&mut self.draft, tool) {
            (Some(draft @ (Annotation::Pen { .. } | Annotation::Highlight { .. })), _) => {
                draft.push_point(point);
            }
            (
                Some(Annotation::Redact {
                    x,
                    y,
                    width,
                    height,
                    ..
                }),
                Tool::Redact,
            ) => {
                // The renderer keeps the signed extent it was dragged with and
                // normalizes on render, so a backwards drag round-trips.
                *x = start.x as f64;
                *y = start.y as f64;
                *width = (point.x - start.x) as f64;
                *height = (point.y - start.y) as f64;
            }
            (Some(draft @ Annotation::Rectangle { .. }), Tool::Rectangle)
            | (Some(draft @ Annotation::Circle { .. }), Tool::Circle) => {
                *draft = build_shape(
                    tool,
                    draft.id().to_string(),
                    start,
                    point,
                    &color,
                    stroke_width,
                    shape_fill.clone(),
                );
            }
            (Some(draft @ Annotation::Line { .. }), Tool::Line)
            | (Some(draft @ Annotation::Arrow { .. }), Tool::Arrow) => {
                *draft = build_segment(
                    tool,
                    draft.id().to_string(),
                    start,
                    point,
                    &color,
                    stroke_width,
                    (tool == Tool::Arrow).then(|| arrow_style.clone()),
                );
            }
            _ => {}
        }
        self.sync_interactive_snapshot();
    }

    fn queue_pointer_update(
        &mut self,
        point: Point,
        shift: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.draft,
            Some(Annotation::Pen { .. } | Annotation::Highlight { .. })
        ) {
            self.pending_stroke_points.push(point);
            self.pending_stroke_shift = shift;
        } else {
            self.pending_pointer_update = Some(point);
        }
        if self.pointer_update_scheduled {
            return;
        }
        self.pointer_update_scheduled = true;
        let editor = cx.entity().downgrade();
        window.on_next_frame(move |_window, cx| {
            let _ = editor.update(cx, |editor, cx| {
                editor.apply_pending_pointer_update(cx);
                cx.notify();
            });
        });
    }

    fn apply_pending_pointer_update(&mut self, _cx: &mut Context<Self>) {
        self.pointer_update_scheduled = false;
        if let Some(point) = self.pending_pointer_update.take() {
            if self.resize_state.is_some() {
                self.resize_selected(point);
                self.sync_interactive_snapshot();
                return;
            }
            if self.move_origin.is_some() {
                self.move_selected(point);
                self.sync_interactive_snapshot();
                return;
            }
            if self.marquee.is_some() {
                if let Some((_, current)) = self.marquee.as_mut() {
                    *current = point;
                }
                self.sync_interactive_snapshot();
                return;
            }
            if self.crop_drag.is_some() {
                self.update_crop_drag(point);
                self.sync_interactive_snapshot();
                return;
            }
            if self.drag_start.is_some() {
                self.extend_stroke(point);
            }
        }
        if self.drag_start.is_none() || self.pending_stroke_points.is_empty() {
            self.pending_stroke_points.clear();
            return;
        }
        let points = std::mem::take(&mut self.pending_stroke_points);
        let shift = std::mem::replace(&mut self.pending_stroke_shift, false);
        let Some(draft @ (Annotation::Pen { .. } | Annotation::Highlight { .. })) = &mut self.draft
        else {
            return;
        };
        let mut changed = false;
        if shift {
            if let Some(last) = points.last() {
                draft.constrain_to_axis(*last);
                changed = true;
            }
        } else {
            for point in points {
                changed |= draft.push_point(point);
            }
        }
        if changed {
            self.sync_interactive_snapshot();
        }
    }

    /// Port of `applyCrop`: crops the base image and shifts every annotation
    /// into the new coordinate space.
    pub fn apply_crop(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.finish_active_transform(cx);
        let Some((x, y, width, height)) = self.crop.take() else {
            return;
        };
        let (left, top) = (
            if width < 0.0 { x + width } else { x },
            if height < 0.0 { y + height } else { y },
        );
        let (crop_width, crop_height) = (width.abs(), height.abs());
        if crop_width < 1.0 || crop_height < 1.0 {
            self.sync_snapshot(cx);
            cx.notify();
            return;
        }

        let pre_crop = self.base_image.clone().map(|base_image| PreCropState {
            base_image,
            width: self.image_width,
            height: self.image_height,
            annotations: self.history.current().to_vec(),
        });

        if let Some(base) = &self.base_image {
            let cropped = image::imageops::crop_imm(
                base.as_ref(),
                left.max(0.0) as u32,
                top.max(0.0) as u32,
                crop_width as u32,
                crop_height as u32,
            )
            .to_image();
            self.image_width = cropped.width() as f32;
            self.image_height = cropped.height() as f32;

            let base = image::DynamicImage::ImageRgba8(cropped);
            self.replace_image(render_image_from_base(&base));
            self.base_image = Some(Arc::new(base));
            self.inset_color = None;
            self.balance_crop = None;
            self.refresh_balance_crop();
        }

        let mut annotations = self.history.current().to_vec();
        for annotation in &mut annotations {
            annotation.translate(-left, -top);
        }
        annotations.retain(|annotation| {
            let (min_x, min_y, max_x, max_y) = annotation.bounds();
            max_x >= 0.0 && max_y >= 0.0 && min_x <= crop_width && min_y <= crop_height
        });
        self.history.push(annotations);
        self.pre_crop = pre_crop;

        self.refresh_raster_patches();
        self.sync_snapshot(cx);
        cx.notify();
    }

    pub fn cancel_crop(&mut self, cx: &mut Context<Self>) {
        self.crop = None;
        self.crop_drag = None;
        self.drag_start = None;
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn begin_text(&mut self, point: Point, color: String, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.selected_annotations.clear();
        let id = self.annotation_id();
        self.draft = Some(Annotation::Text {
            id: id.clone(),
            x: point.x as f64,
            y: point.y as f64,
            text: String::new(),
            font_size: self.text_font_size,
            fill: color,
            font_family: Some(self.text_font_family.clone()),
            background_color: self
                .text_background
                .then(|| annotations::TEXT_BG_COLOR.to_string()),
            background_opacity: None,
            background_padding: self.text_background.then_some(annotations::Offset {
                x: annotations::TEXT_BG_PADDING_X,
                y: annotations::TEXT_BG_PADDING_Y,
            }),
            background_radius: self.text_background.then_some(annotations::TEXT_BG_RADIUS),
            rotation: None,
        });

        let field = cx.new(|cx| InlineTextState::new(cx));
        self.text_editor = Some((id, field));
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn text_annotation_at(&self, point: Point) -> Option<String> {
        let id = self.annotation_at(point)?;
        matches!(
            self.history
                .current()
                .iter()
                .find(|annotation| annotation.id() == id),
            Some(Annotation::Text { .. })
        )
        .then_some(id)
    }

    pub fn cancel_text(&mut self, cx: &mut Context<Self>) {
        if self.text_editor.take().is_none() {
            return;
        }
        self.draft = None;
        self.sync_snapshot(cx);
        cx.notify();
    }

    /// Commits the in-progress text annotation, dropping it when empty.
    /// A draft that matches a committed annotation is a re-edit: the text is
    /// replaced, or the annotation deleted when emptied.
    pub fn commit_text(&mut self, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        let Some((_, _)) = self.text_editor.take() else {
            return;
        };
        let Some(draft) = self.draft.take() else {
            self.sync_snapshot(cx);
            cx.notify();
            return;
        };
        if let Annotation::Text { id, text, .. } = &draft {
            let mut annotations = self.history.current().to_vec();
            if annotations.iter().any(|annotation| annotation.id() == id) {
                if text.trim().is_empty() {
                    annotations.retain(|annotation| annotation.id() != id);
                    self.selected_annotations.retain(|selected| selected != id);
                } else if let Some(existing) = annotations
                    .iter_mut()
                    .find(|annotation| annotation.id() == id)
                {
                    *existing = draft;
                }
                if annotations != self.history.current() {
                    self.push_annotations(annotations, cx);
                    self.refresh_raster_patches();
                }
            } else if !text.trim().is_empty() {
                annotations.push(draft);
                self.push_annotations(annotations, cx);
            }
        }
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn begin_text_reedit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text(cx);
        let Some(existing) = self
            .history
            .current()
            .iter()
            .find(|annotation| annotation.id() == id)
            .cloned()
        else {
            return;
        };
        let Annotation::Text { text, .. } = &existing else {
            return;
        };
        let text = text.clone();
        self.draft = Some(existing);
        let field = cx.new(|cx| InlineTextState::with_value(cx, text));
        self.text_editor = Some((id.to_string(), field));
        self.sync_snapshot(cx);
        cx.notify();
        self.focus_text_editor(window, cx);
    }

    fn text_editor_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (_, field) = self.text_editor.as_ref()?;
        let Some(Annotation::Text {
            x,
            y,
            font_size,
            fill,
            font_family,
            background_color,
            background_padding,
            background_radius,
            ..
        }) = &self.draft
        else {
            return None;
        };
        let owner = cx.entity().downgrade();
        let commit_owner = owner.clone();
        let cancel_owner = owner.clone();
        let family = font_family
            .as_deref()
            .unwrap_or(annotations::DEFAULT_TEXT_FONT);
        let text_color = crate::theme::color::Srgba::parse(fill).to_hsla();
        let bg_color = background_color
            .as_deref()
            .map(|bg| crate::theme::color::Srgba::parse(bg).to_hsla());
        let (padding_x, padding_y) = if bg_color.is_some() {
            let padding = background_padding.unwrap_or(annotations::Offset {
                x: annotations::TEXT_BG_PADDING_X,
                y: annotations::TEXT_BG_PADDING_Y,
            });
            (padding.x as f32 * self.zoom, padding.y as f32 * self.zoom)
        } else {
            (3.0, 1.0)
        };
        let radius = if bg_color.is_some() {
            background_radius.unwrap_or(annotations::TEXT_BG_RADIUS) as f32 * self.zoom
        } else {
            2.0
        };
        let theme = active_theme(cx);
        Some(
            InlineTextInput {
                state: field.clone(),
                x: *x,
                y: *y,
                zoom: self.zoom,
                font_size: *font_size,
                font_family: crate::editor::text_render::ui_family(family),
                text_color,
                background_color: bg_color,
                padding_x,
                padding_y,
                radius,
                border_color: theme.primary.opacity(0.8),
                selection_color: theme.primary.opacity(0.28),
                on_change: Rc::new(move |value, _window, app| {
                    let value = value.to_string();
                    if let Some(owner) = owner.upgrade() {
                        owner.update(app, |editor, cx| {
                            if let Some(Annotation::Text { text, .. }) = &mut editor.draft {
                                *text = value.clone();
                            }
                            editor.sync_snapshot(cx);
                            cx.notify();
                        });
                    }
                }),
                on_commit: Rc::new(move |window, app| {
                    if let Some(owner) = commit_owner.upgrade() {
                        owner.update(app, |editor, cx| {
                            editor.commit_text(cx);
                            window.focus(&editor.focus_handle, cx);
                        });
                    }
                }),
                on_cancel: Rc::new(move |window, app| {
                    if let Some(owner) = cancel_owner.upgrade() {
                        owner.update(app, |editor, cx| {
                            editor.cancel_text(cx);
                            window.focus(&editor.focus_handle, cx);
                        });
                    }
                }),
            }
            .into_any_element(),
        )
    }

    /// Port of `getNextNumberValue`: badges count up from the configured
    /// start value in creation order.
    fn next_number_value(&self) -> f64 {
        let placed = self
            .history
            .current()
            .iter()
            .filter(|annotation| matches!(annotation, Annotation::Number { .. }))
            .count();
        self.number_start_value + placed as f64
    }

    fn finish_annotation_edit(&mut self, original: Option<Vec<Annotation>>, returned: bool) {
        let Some(original) = original else {
            return;
        };
        if returned {
            self.history.replace_current(original);
            return;
        }
        let updated = self.history.current().to_vec();
        self.history.replace_current(original);
        if updated != self.history.current() {
            self.history.push(updated);
        }
    }

    fn finish_active_transform(&mut self, cx: &mut Context<Self>) {
        if self.move_origin.is_some() || self.resize_state.is_some() || self.crop_drag.is_some() {
            self.apply_pending_pointer_update(cx);
            self.finish_stroke(cx);
        }
    }

    fn finish_stroke(&mut self, cx: &mut Context<Self>) {
        if self.crop_drag.take().is_some() {
            self.sync_snapshot(cx);
            return;
        }
        if let Some(origin) = self.move_origin.take() {
            let original = self.move_original.take();
            let returned = original.as_ref().is_some_and(|(start, _)| *start == origin);
            self.finish_annotation_edit(original.map(|(_, annotations)| annotations), returned);
            self.refresh_raster_patches();
            self.sync_snapshot(cx);
            return;
        }
        if let Some(state) = self.resize_state.take() {
            self.finish_annotation_edit(state.original, state.last == state.start);
            self.refresh_raster_patches();
            self.sync_snapshot(cx);
            return;
        }
        if let Some((start, current)) = self.marquee.take() {
            let left = f64::from(start.x).min(f64::from(current.x));
            let top = f64::from(start.y).min(f64::from(current.y));
            let right = f64::from(start.x).max(f64::from(current.x));
            let bottom = f64::from(start.y).max(f64::from(current.y));
            if right - left > 5.0 || bottom - top > 5.0 {
                self.selected_annotations = self
                    .history
                    .current()
                    .iter()
                    .filter(|annotation| {
                        let (a_left, a_top, a_right, a_bottom) = annotation.bounds();
                        a_left <= right && a_right >= left && a_top <= bottom && a_bottom >= top
                    })
                    .map(|annotation| annotation.id().to_string())
                    .collect();
            } else {
                self.selected_annotations.clear();
            }
            self.sync_snapshot(cx);
            return;
        }
        if self.text_editor.is_some() {
            self.drag_start = None;
            return;
        }
        if let Some(draft) = self.draft.take() {
            let has_extent = match &draft {
                Annotation::Pen { points, .. } | Annotation::Highlight { points, .. } => {
                    points.len() > 2
                }
                Annotation::Rectangle { width, height, .. }
                | Annotation::Redact { width, height, .. } => {
                    width.abs() > 1.0 || height.abs() > 1.0
                }
                Annotation::Circle { radius, .. } => *radius > 1.0,
                Annotation::Line { points, .. } | Annotation::Arrow { points, .. } => {
                    (points[0] - points[2]).abs() > 1.0 || (points[1] - points[3]).abs() > 1.0
                }
                Annotation::Number { .. } => true,
                Annotation::Text { text, .. } => !text.trim().is_empty(),
            };
            if has_extent {
                let mut annotations = self.history.current().to_vec();
                annotations.push(draft);
                self.push_annotations(annotations, cx);
                self.refresh_raster_patches();
            }
        }
        if self.tool == Tool::Crop && self.drag_start.is_some() {
            if let Some((_, _, width, height)) = self.crop {
                if width.abs() < 2.0 || height.abs() < 2.0 {
                    self.crop = None;
                }
            }
        }
        self.drag_start = None;
        self.sync_snapshot(cx);
    }
}

fn render_redact_patch(
    base: &image::DynamicImage,
    rect: (f64, f64, f64, f64),
    style: &str,
    intensity: f64,
    native_scale: f64,
) -> Option<crate::editor::canvas::RedactPatch> {
    let (x, y, width, height) = rect;
    let (x, y, width, height) = crate::editor::annotations::normalize_rect(x, y, width, height);
    if width <= 0. || height <= 0. {
        return None;
    }
    let left = x.floor();
    let top = y.floor();
    let x0 = left.max(0.0) as u32;
    let y0 = top.max(0.0) as u32;
    if x0 >= base.width() || y0 >= base.height() {
        return None;
    }
    let w = ((x + width).ceil() - f64::from(x0))
        .max(0.0)
        .min(f64::from(base.width() - x0)) as u32;
    let h = ((y + height).ceil() - f64::from(y0))
        .max(0.0)
        .min(f64::from(base.height() - y0)) as u32;
    if w == 0 || h == 0 {
        return None;
    }

    let mut patch = match base.as_rgba8() {
        Some(rgba) => image::imageops::crop_imm(rgba, x0, y0, w, h).to_image(),
        None => image::imageops::crop_imm(&base.to_rgba8(), x0, y0, w, h).to_image(),
    };
    crate::editor::export::redact_region(
        &mut patch,
        (0, 0, w as i64, h as i64),
        style,
        intensity,
        native_scale,
    );

    // GPUI composites in BGRA.
    for pixel in patch.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Some(crate::editor::canvas::RedactPatch {
        image: Arc::new(gpui::RenderImage::new(smallvec::smallvec![
            image::Frame::new(patch)
        ])),
        x: f64::from(x0),
        y: f64::from(y0),
        width: f64::from(w),
        height: f64::from(h),
    })
}

/// Port of the rectangle and circle branches of `useDrawingTools`: a
/// rectangle keeps the signed drag extent, a circle takes the drag's midpoint
/// and half its diagonal.
fn build_shape(
    tool: Tool,
    id: String,
    start: Point,
    end: Point,
    color: &str,
    stroke_width: f64,
    fill: Option<String>,
) -> Annotation {
    let stroke = color.to_string();

    if tool == Tool::Circle {
        let radius = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt() as f64 / 2.0;
        return Annotation::Circle {
            id,
            x: ((start.x + end.x) / 2.0) as f64,
            y: ((start.y + end.y) / 2.0) as f64,
            radius,
            stroke,
            stroke_width,
            fill,
        };
    }
    Annotation::Rectangle {
        id,
        x: start.x as f64,
        y: start.y as f64,
        width: (end.x - start.x) as f64,
        height: (end.y - start.y) as f64,
        stroke,
        stroke_width,
        fill,
    }
}

fn apply_resize(
    annotation: &mut Annotation,
    state: &ResizeState,
    dx: f64,
    dy: f64,
    total_dx: f64,
    total_dy: f64,
    point: Point,
) {
    match annotation {
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            ..
        }
        | Annotation::Redact {
            x,
            y,
            width,
            height,
            ..
        } => {
            let (Some((anchor_x, anchor_y)), Some((drag_x, drag_y))) =
                (state.anchor, state.initial_drag)
            else {
                return;
            };
            *x = anchor_x;
            *y = anchor_y;
            *width = drag_x + total_dx - anchor_x;
            *height = drag_y + total_dy - anchor_y;
        }
        Annotation::Circle { radius, .. } => {
            let delta = match state.handle {
                ResizeHandle::BottomRight => (dx + dy) / 2.0,
                ResizeHandle::TopLeft => (-dx - dy) / 2.0,
                ResizeHandle::TopRight => (dx - dy) / 2.0,
                ResizeHandle::BottomLeft => (-dx + dy) / 2.0,
                _ => return,
            };
            *radius = (*radius + delta).max(5.0);
        }
        Annotation::Line { points, .. } => match state.handle {
            ResizeHandle::Start => {
                points[0] += dx;
                points[1] += dy;
            }
            ResizeHandle::End => {
                points[2] += dx;
                points[3] += dy;
            }
            _ => {}
        },
        Annotation::Arrow {
            points,
            bend_offset,
            ..
        } => match state.handle {
            ResizeHandle::Start => {
                points[0] += dx;
                points[1] += dy;
            }
            ResizeHandle::End => {
                points[2] += dx;
                points[3] += dy;
            }
            ResizeHandle::Bend => {
                let bend = bend_offset.get_or_insert(annotations::Offset { x: 0.0, y: 0.0 });
                bend.x += dx;
                bend.y += dy;
            }
            _ => {}
        },
        Annotation::Text {
            font_size,
            rotation,
            ..
        } => match state.handle {
            ResizeHandle::Rotate => {
                let (Some((center_x, center_y)), Some(initial), Some(start_angle)) =
                    (state.center, state.initial_rotation, state.start_angle)
                else {
                    return;
                };
                let current = (f64::from(point.y) - center_y).atan2(f64::from(point.x) - center_x);
                *rotation = Some(initial + (current - start_angle) * 180.0 / std::f64::consts::PI);
            }
            ResizeHandle::BottomRight => {
                let (Some(initial_font), Some((initial_width, initial_height))) =
                    (state.initial_font_size, state.initial_size)
                else {
                    return;
                };
                let scale = ((initial_width + total_dx) / initial_width)
                    .max((initial_height + total_dy) / initial_height)
                    .max(0.1);
                *font_size = (initial_font * scale).round().max(8.0);
            }
            _ => {}
        },
        Annotation::Pen { .. } | Annotation::Highlight { .. } | Annotation::Number { .. } => {}
    }
}

fn build_segment(
    tool: Tool,
    id: String,
    start: Point,
    end: Point,
    color: &str,
    stroke_width: f64,
    arrow_style: Option<String>,
) -> Annotation {
    let stroke = color.to_string();
    let points = [start.x as f64, start.y as f64, end.x as f64, end.y as f64];
    if tool == Tool::Arrow {
        Annotation::Arrow {
            id,
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset: None,
        }
    } else {
        Annotation::Line {
            id,
            points,
            stroke,
            stroke_width,
        }
    }
}

/// Converts a window-space position into image coordinates using the recorded
/// canvas frame bounds and current zoom.
fn to_canvas_point(
    position: gpui::Point<Pixels>,
    bounds: Option<gpui::Bounds<Pixels>>,
    zoom: f32,
) -> Point {
    match bounds {
        Some(bounds) => {
            let scale = zoom.max(0.0001);
            Point {
                x: ((position.x - bounds.left()) / scale)
                    .max(Pixels::ZERO)
                    .into(),
                y: ((position.y - bounds.top()) / scale)
                    .max(Pixels::ZERO)
                    .into(),
            }
        }
        None => Point { x: 0.0, y: 0.0 },
    }
}

impl EditorWindow {
    /// The canvas at 1x, wallpaper frame and attached layers included -- the
    /// `canvasWidth`/`canvasHeight` that `useContentDimensions` reports.
    fn content_size(&self) -> (f32, f32) {
        let wallpaper = self.wallpaper.for_scale(self.native_scale);
        let (image_width, image_height) = crate::editor::wallpaper::balanced_size(
            f64::from(self.image_width),
            f64::from(self.image_height),
            self.balance_crop,
            !self.layers.is_empty()
                && !wallpaper.supports_edge_preview(true)
                && crate::render::window_frame::theme(&wallpaper.window_frame.style).is_none(),
        );
        let title_bar = if self.layers.is_empty() {
            0.0
        } else {
            crate::render::window_frame::title_bar_height(
                &wallpaper.window_frame.style,
                self.native_scale,
            )
        };
        let group = crate::editor::layers::compute_with_title_bar(
            image_width,
            image_height,
            &self.layers,
            wallpaper.spacing.max(0.0),
            wallpaper.layer_inset(!self.layers.is_empty()),
            title_bar,
        );
        if wallpaper.is_active_with_layers(!self.layers.is_empty()) {
            let ((width, height), _) = crate::editor::wallpaper::layout(
                &wallpaper,
                group.width,
                group.height,
                self.native_scale,
                !self.layers.is_empty(),
            );
            (width as f32, height as f32)
        } else {
            (group.width as f32, group.height as f32)
        }
    }

    fn fit_to_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sheet_open = self.tool == Tool::Wallpaper;
        let viewport = window.viewport_size();
        let content = self.content_size();
        if self.fit_for == Some((viewport, sheet_open, content)) {
            return;
        }
        self.fit_for = Some((viewport, sheet_open, content));

        let (width, height) = content;
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.set_zoom(
            crate::editor::zoom_fit::optimal_zoom(viewport, width, height, sheet_open),
            cx,
        );
    }

    /// Rebuilds the zoom control's `backdrop-blur-md` from the previous frame's
    /// measurements, and asks for one more frame when it changed so the new crop
    /// is actually shown.
    fn refresh_zoom_backdrop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::editor::zoom_backdrop;

        let previous = self.zoom_backdrop.as_ref().map(|backdrop| backdrop.key());
        let previous_image = self.zoom_backdrop.as_ref().map(|backdrop| backdrop.image());
        let key = (|| {
            let bar = (*self.zoom_bar_bounds.borrow())?;
            let content = (*self.bounds.borrow())?;
            let base = self.base_image.as_ref()?;
            zoom_backdrop::sample_rect(
                bar,
                content,
                self.zoom,
                base.width() as f32,
                base.height() as f32,
            )
        })();

        self.zoom_backdrop = match (key, self.base_image.as_ref()) {
            (Some(key), Some(base)) => zoom_backdrop::build(self.zoom_backdrop.take(), key, base),
            _ => None,
        };

        if let Some(old_image) = previous_image {
            let still_live = self
                .zoom_backdrop
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(&current.image(), &old_image));
            if !still_live {
                let _ = window.drop_image(old_image);
            }
        }

        if self.zoom_backdrop.as_ref().map(|b| b.key()) != previous {
            cx.notify();
        }
    }
}

impl EditorWindow {
    fn open_preset_draft(&mut self, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(cx));
        let subscription = cx.observe(&field, |_, _, cx| cx.notify());
        self.preset_draft = Some(PresetDraft {
            field,
            _subscription: subscription,
        });
    }

    fn ensure_background_previews(&mut self, cx: &mut Context<Self>) {
        let size =
            chrome::wallpaper_tile_size(chrome::WALLPAPER_SHEET_WIDTH, chrome::WALLPAPER_SHEET_PAD);
        if crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::DesktopWallpaper,
        ) && !self.background_previews.desktop_requested()
        {
            self.background_previews.request_desktop();
            let daemon = crate::state::state(cx).daemon;
            cx.spawn(async move |entity, cx| {
                let (source, image) = cx
                    .background_executor()
                    .spawn(async move {
                        let Some(source) = crate::editor::background::desktop_wallpaper(&daemon)
                        else {
                            return (None, None);
                        };
                        let image = crate::editor::background::preview_image(&source, size);
                        (image.is_some().then_some(source), image)
                    })
                    .await;
                let _ = entity.update(cx, |editor, cx| {
                    editor.background_previews.set_desktop(source, image);
                    cx.notify();
                });
            })
            .detach();
        }

        let customs = crate::state::state(cx)
            .config
            .get()
            .wallpaper
            .custom_backgrounds;
        for background in customs {
            let crate::config::schema::CustomBackgroundData::Image { data } = &background.data
            else {
                continue;
            };
            if self.background_previews.custom_requested(&background.id) {
                continue;
            }
            self.background_previews.request_custom(&background.id);
            let id = background.id.clone();
            let source = data.image_url.clone();
            cx.spawn(async move |entity, cx| {
                let image = cx
                    .background_executor()
                    .spawn(async move { crate::editor::background::preview_image(&source, size) })
                    .await;
                let _ = entity.update(cx, |editor, cx| {
                    editor.background_previews.set_custom(&id, image);
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

impl Render for EditorWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let theme = active_theme(cx);
        let native_scale = crate::editor::wallpaper::native_scale(
            f64::from(self.image_width),
            f64::from(self.image_height),
            f64::from(window.scale_factor()),
        );
        let mut scale_or_layout_changed = false;
        if self.native_scale != native_scale {
            self.native_scale = native_scale;
            self.refresh_raster_patches();
            scale_or_layout_changed = true;
        }
        let window_scale = f64::from(window.scale_factor());
        if self.window_scale != window_scale {
            self.window_scale = window_scale;
            self.retire_backdrop();
            self.backdrop_key = None;
            scale_or_layout_changed = true;
        }
        let zoom_before = self.zoom;
        self.fit_to_window(window, cx);
        if self.zoom != zoom_before {
            scale_or_layout_changed = true;
        }
        for image in self.retired_preview_edges.drain(..) {
            let _ = window.drop_image(image);
        }
        if scale_or_layout_changed || !self.is_interactive_transform() {
            self.refresh_backdrop(cx);
            self.sync_snapshot(cx);
        } else {
            self.sync_interactive_snapshot();
        }
        self.refresh_zoom_backdrop(window, cx);

        let handlers = build_handlers(&weak);
        let shortcuts = crate::state::state(cx)
            .config
            .read(|config| config.shortcuts.clone());

        let sheet_open = self.tool == Tool::Wallpaper;
        if sheet_open {
            self.ensure_background_previews(cx);
        }
        if self.sheet_closing {
            self.schedule_sheet_unmount(cx);
        }
        let sheet_mounted = sheet_open || self.sheet_closing;
        let preset_draft = self.preset_draft.as_ref().map(|draft| {
            let name = draft.field.read(cx).value().to_string();
            (draft.field.clone(), name)
        });
        let capture_overlay = actions::capture_overlay_visible(self.capture_mode, self.meta_held);

        let snapshot_cell = self.snapshot.clone();
        let bounds_cell = self.bounds.clone();

        let down_entity = cx.entity().downgrade();
        let move_entity = cx.entity().downgrade();
        let up_entity = cx.entity().downgrade();

        let root = div()
            .id("editor-window")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .font_family(crate::ui::font::UI_FONT)
            .text_color(theme.foreground)
            .track_focus(&self.focus_handle)
            .on_key_down(|event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("s") {
                    eprintln!(
                        "[diag-key] key={} mods={:?} focused={}",
                        event.keystroke.key,
                        event.keystroke.modifiers,
                        window.focused(cx).is_some()
                    );
                }
            })
            .on_modifiers_changed(cx.listener(
                |this, event: &ModifiersChangedEvent, _window, cx| {
                    let held = actions::is_meta_held(&event.modifiers, cx);
                    if this.meta_held == held {
                        return;
                    }
                    this.meta_held = held;
                    cx.notify();
                },
            ));
        let root = if self.text_editor.is_none() {
            root.key_context("Editor")
        } else {
            root
        };

        let content = root
            .on_action(
                cx.listener(|this, _: &actions::ToolSelect, _, cx| this.set_tool(Tool::Select, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolPen, _, cx| this.set_tool(Tool::Pen, cx)),
            )
            .on_action(cx.listener(|this, _: &actions::ToolHighlight, _, cx| {
                this.set_tool(Tool::Highlight, cx)
            }))
            .on_action(cx.listener(|this, _: &actions::ToolRectangle, _, cx| {
                this.set_tool(Tool::Rectangle, cx)
            }))
            .on_action(
                cx.listener(|this, _: &actions::ToolCircle, _, cx| this.set_tool(Tool::Circle, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolLine, _, cx| this.set_tool(Tool::Line, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolArrow, _, cx| this.set_tool(Tool::Arrow, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolText, _, cx| this.set_tool(Tool::Text, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolNumber, _, cx| this.set_tool(Tool::Number, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolRedact, _, cx| this.set_tool(Tool::Redact, cx)),
            )
            .on_action(
                cx.listener(|this, _: &actions::ToolCrop, _, cx| this.set_tool(Tool::Crop, cx)),
            )
            .on_action(cx.listener(|this, _: &actions::ToolWallpaper, _, cx| {
                this.set_tool(Tool::Wallpaper, cx)
            }))
            .on_action(cx.listener(|this, _: &actions::Undo, _, cx| this.undo(cx)))
            .on_action(cx.listener(|this, _: &actions::Redo, _, cx| this.redo(cx)))
            .on_action(
                cx.listener(|this, _: &actions::SaveScreenshot, window, cx| {
                    this.save_as(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &actions::PrintScreenshot, _, cx| {
                this.print(cx);
            }))
            .on_action(
                cx.listener(|this, _: &actions::DeleteScreenshot, window, cx| {
                    this.delete_file(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &actions::ApplyCrop, _, cx| {
                if this.crop.is_some() {
                    this.apply_crop(cx);
                } else {
                    this.commit_text(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &actions::CancelCrop, _, cx| {
                if this.crop.is_some() {
                    this.cancel_crop(cx);
                } else if this.text_editor.is_some() {
                    this.cancel_text(cx);
                } else if this.draft.is_some() {
                    this.draft = None;
                    this.sync_snapshot(cx);
                    cx.notify();
                } else {
                    this.finish_active_transform(cx);
                    this.selected_annotations.clear();
                    this.move_origin = None;
                    this.move_original = None;
                    this.resize_state = None;
                    this.sync_snapshot(cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &actions::ToggleCaptureMode, _, cx| {
                this.toggle_capture_mode(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::CloudUpload, _, cx| {
                this.upload_to_cloud(cx);
            }))
            .on_action(
                cx.listener(|this, _: &actions::CopyAnnotation, window, cx| {
                    // A selected annotation is copied; otherwise the whole image
                    // is, which is what the renderer's Ctrl+C falls back to.
                    if !this.copy_selected_annotation(cx) {
                        this.copy_to_clipboard(window, cx);
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &actions::CutAnnotation, _, cx| {
                this.cut_selected_annotation(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::PasteAnnotation, _, cx| {
                this.paste_annotations(cx);
            }))
            .on_action(
                cx.listener(|this, _: &actions::SelectAllAnnotations, _, cx| {
                    this.select_all_annotations(cx);
                }),
            )
            .on_action(cx.listener(|this, _: &actions::DeleteAnnotation, _, cx| {
                this.delete_selected_annotation(cx);
            }))
            .on_action(cx.listener(|this, _: &actions::ZoomIn, _, cx| {
                this.set_zoom(this.zoom + ZOOM_STEP, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &actions::ZoomOut, _, cx| {
                this.set_zoom(this.zoom - ZOOM_STEP, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &actions::ZoomReset, _, cx| {
                this.set_zoom(1.0, cx);
                cx.notify();
            }))
            .child(TitleBar {
                options: self.tool_options_state(),
                highlight_color: self.highlight_color.clone().into(),
                shortcuts: shortcuts.editor.clone(),
                cloud_upload_shortcut: shortcuts.editor_actions.upload_to_cloud.clone().into(),
                can_undo: self.history.can_undo(),
                can_redo: self.history.can_redo(),
                is_copied: self.is_copied,
                is_copying: self.copy_pending,
                is_pinning: self.pin_pending,
                is_uploading: self.cloud_upload == crate::cloud::UploadState::Uploading,
                is_upload_done: self.cloud_upload == crate::cloud::UploadState::Success,
                is_capture_mode: self.capture_mode,
                handlers: handlers.clone(),
            })
            .child(
                div()
                    .id("editor-body")
                    .relative()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .when(sheet_mounted, |el| {
                        el.child(crate::editor::wallpaper_sheet::render(
                            &crate::editor::wallpaper_sheet::SheetState {
                                wallpaper: &self.wallpaper,
                                has_layers: !self.layers.is_empty(),
                                preset_id: &self.wallpaper_preset_id,
                                draft: self.background_editor.as_ref(),
                                background_picker_open: self.background_picker_open,
                                previews: &self.background_previews,
                                preset_draft: preset_draft.as_ref().map(|(field, name)| {
                                    crate::editor::wallpaper_sheet::PresetDraftView {
                                        field,
                                        name: name.as_str(),
                                    }
                                }),
                                closing: self.sheet_closing,
                            },
                            &handlers,
                            window,
                            cx,
                        ))
                    })
                    .child(
                        div()
                            .id("canvas-area")
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_hidden()
                            .cursor(crate::editor::options::tool_cursor(self.tool))
                            .child(EditorCanvas::new(
                                snapshot_cell,
                                bounds_cell,
                                self.stage_scroll.clone(),
                                self.text_editor_overlay(cx),
                            ))
                            .when(capture_overlay, |el| {
                                let entity = cx.entity().downgrade();
                                el.child(crate::editor::canvas::capture_edge_overlay(
                                    &theme,
                                    std::rc::Rc::new(move |edge, window, cx| {
                                        if let Some(entity) = entity.upgrade() {
                                            entity.update(cx, |editor, cx| {
                                                editor.attach_layer(edge, window, cx)
                                            });
                                        }
                                    }),
                                ))
                            })
                            .when(self.drop_edge.is_some(), |el| {
                                el.child(crate::editor::canvas::drop_zone_overlay(
                                    self.drop_edge,
                                    &theme,
                                ))
                            })
                            .on_drag_move::<ExternalPaths>(cx.listener(
                                |this, event: &DragMoveEvent<ExternalPaths>, _window, cx| {
                                    if !event.drag(cx).0.iter().any(|path| is_image_path(path)) {
                                        if this.drop_edge.take().is_some() {
                                            cx.notify();
                                        }
                                        return;
                                    }
                                    let before = this.drop_edge;
                                    this.update_drop_edge(event.event.position, event.bounds);
                                    if this.drop_edge != before {
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_drop::<ExternalPaths>(cx.listener(
                                |this, paths: &ExternalPaths, _window, cx| {
                                    this.handle_image_drop(paths, cx);
                                },
                            ))
                            .on_file_drop_exit(cx.listener(
                                |this, _event: &FileDropEvent, _window, cx| {
                                    if this.drop_edge.take().is_some() {
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_mouse_down(gpui::MouseButton::Left, {
                                let entity = down_entity;
                                move |event: &MouseDownEvent, window, cx| {
                                    entity
                                        .update(cx, |editor, cx| {
                                            // Ctrl/Cmd-drag pans the stage instead of
                                            // drawing, matching `usePanOnDrag`.
                                            if event.modifiers.secondary(cx) {
                                                editor.pan_origin = Some((
                                                    event.position,
                                                    editor.stage_scroll.offset(),
                                                ));
                                                return;
                                            }
                                            if editor.text_editor.is_some() {
                                                editor.commit_text(cx);
                                                window.focus(&editor.focus_handle, cx);
                                                window.prevent_default();
                                                return;
                                            }
                                            let bounds = *editor.bounds.borrow();
                                            let point = to_canvas_point(
                                                event.position,
                                                bounds,
                                                editor.zoom,
                                            );
                                            if event.click_count >= 2 && editor.tool != Tool::Crop {
                                                if let Some(id) = editor.text_annotation_at(point) {
                                                    editor.begin_text_reedit(&id, window, cx);
                                                    cx.notify();
                                                    return;
                                                }
                                            }
                                            editor.start_stroke(
                                                point,
                                                event.modifiers.shift,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        })
                                        .ok();
                                }
                            })
                            .on_mouse_move({
                                let entity = move_entity;
                                move |event: &MouseMoveEvent, window, cx| {
                                    let position = event.position;
                                    entity
                                        .update(cx, |editor, cx| {
                                            if let Some((pointer, offset)) = editor.pan_origin {
                                                editor.stage_scroll.set_offset(gpui::point(
                                                    offset.x + (position.x - pointer.x),
                                                    offset.y + (position.y - pointer.y),
                                                ));
                                                cx.notify();
                                                return;
                                            }
                                            if let Some((_, field)) = editor.text_editor.clone() {
                                                if field.read(cx).is_selecting() {
                                                    field.update(cx, |state, cx| {
                                                        if state.update_drag_selection(position) {
                                                            cx.notify();
                                                        }
                                                    });
                                                    return;
                                                }
                                            }
                                            if editor.move_origin.is_some()
                                                || editor.drag_start.is_some()
                                                || editor.resize_state.is_some()
                                                || editor.marquee.is_some()
                                                || editor.crop_drag.is_some()
                                            {
                                                let bounds = *editor.bounds.borrow();
                                                let point =
                                                    to_canvas_point(position, bounds, editor.zoom);
                                                editor.queue_pointer_update(
                                                    point,
                                                    event.modifiers.shift,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })
                                        .ok();
                                }
                            })
                            .on_mouse_up(gpui::MouseButton::Left, {
                                let entity = up_entity;
                                move |event: &MouseUpEvent, window, cx| {
                                    entity
                                        .update(cx, |editor, cx| {
                                            if let Some((pointer, offset)) =
                                                editor.pan_origin.take()
                                            {
                                                editor.stage_scroll.set_offset(gpui::point(
                                                    offset.x + (event.position.x - pointer.x),
                                                    offset.y + (event.position.y - pointer.y),
                                                ));
                                            } else if editor.move_origin.is_some()
                                                || editor.drag_start.is_some()
                                                || editor.resize_state.is_some()
                                                || editor.marquee.is_some()
                                                || editor.crop_drag.is_some()
                                            {
                                                let point = to_canvas_point(
                                                    event.position,
                                                    *editor.bounds.borrow(),
                                                    editor.zoom,
                                                );
                                                editor.queue_pointer_update(
                                                    point,
                                                    event.modifiers.shift,
                                                    window,
                                                    cx,
                                                );
                                            }
                                            if let Some((_, field)) = editor.text_editor.clone() {
                                                field.update(cx, |state, _| state.end_selection());
                                                return;
                                            }
                                            editor.apply_pending_pointer_update(cx);
                                            editor.finish_stroke(cx);
                                            cx.notify();
                                        })
                                        .ok();
                                }
                            })
                            .on_scroll({
                                let entity = cx.entity().downgrade();
                                move |event: &ScrollEvent, _window, cx| {
                                    if !event.modifiers.secondary(cx) {
                                        return;
                                    }
                                    let delta = event.delta.pixel_delta(px(36.0)).y;
                                    entity
                                        .update(cx, |editor, cx| {
                                            let step = if delta > Pixels::ZERO {
                                                -ZOOM_STEP
                                            } else if delta < Pixels::ZERO {
                                                ZOOM_STEP
                                            } else {
                                                return;
                                            };
                                            editor.set_zoom(editor.zoom + step, cx);
                                            cx.stop_propagation();
                                            cx.notify();
                                        })
                                        .ok();
                                }
                            })
                            .child(zoom_control(
                                self.zoom,
                                self.zoom_backdrop.as_ref().map(|b| b.image()),
                                self.zoom_bar_bounds.clone(),
                                &handlers,
                                &theme,
                            )),
                    ),
            );
        crate::ui::window_root::focus_root(content, window, cx)
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        self.force_persist_editor_state();
        crate::system::memory::reclaim_idle_memory();
    }
}

fn build_handlers(weak: &gpui::WeakEntity<EditorWindow>) -> EditorHandlers {
    let option_entity = weak.clone();
    let action_entity = weak.clone();

    EditorHandlers {
        on_option: Rc::new(move |option, window, cx: &mut App| {
            let Some(entity) = option_entity.upgrade() else {
                return;
            };
            if option == EditorOption::WallpaperEditorPickImage {
                entity.update(cx, |editor, cx| {
                    editor.pick_background_image(window, cx);
                    editor.refresh_balance_crop();
                    editor.refresh_backdrop(cx);
                    editor.sync_snapshot(cx);
                    cx.notify();
                });
                return;
            }
            entity.update(cx, |editor, cx| {
                editor.apply_option(option, cx);
                editor.refresh_balance_crop();
                editor.refresh_backdrop(cx);
                editor.sync_snapshot(cx);
                cx.notify();
            });
        }),
        on_action: Rc::new(move |action, window, cx: &mut App| {
            let Some(entity) = action_entity.upgrade() else {
                return;
            };
            entity.update(cx, |editor, cx| {
                editor.apply_action(action, window, cx);
                cx.notify();
            });
        }),
    }
}

const ZOOM_BUTTON_SIZE: f32 = 28.0;

fn zoom_control(
    zoom: f32,
    backdrop: Option<std::sync::Arc<gpui::RenderImage>>,
    bar_bounds: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
    handlers: &EditorHandlers,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::AnyElement {
    let zoom_out = handlers.option(EditorOption::Zoom(zoom - ZOOM_STEP));
    let zoom_reset = handlers.option(EditorOption::Zoom(1.0));
    let zoom_in = handlers.option(EditorOption::Zoom(zoom + ZOOM_STEP));
    let zoom_out_name = "Zoom Out";
    let zoom_in_name = "Zoom In";

    div()
        .id("zoom-control")
        .absolute()
        .bottom(px(chrome::ZOOM_INSET))
        .right(px(chrome::ZOOM_INSET))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(chrome::ZOOM_GAP))
        .rounded(px(chrome::BUTTON_RADIUS))
        // `bg-surface/90 backdrop-blur-md`. The blurred crop of the capture goes
        // underneath, then the 90% surface over it, so the tenth that shows
        // through is low-passed rather than sharp. With no crop -- the bar is
        // over the flat stage, where blur is the identity -- the surface stands
        // alone, exactly as before.
        .overflow_hidden()
        .when_some(backdrop, |el, image| {
            el.child(
                gpui::img(image)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(gpui::ObjectFit::Fill),
            )
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(chrome::BUTTON_RADIUS))
                .bg(theme.surface.opacity(0.9)),
        )
        .child(
            gpui::canvas(
                move |rect, window, _cx| {
                    // The backdrop is built from this measurement, so it can
                    // only appear on the frame after the bar first lands, or
                    // after it moves. Asking for that frame here -- and only on
                    // a change -- converges rather than spinning.
                    if bar_bounds.borrow().is_none_or(|previous| previous != rect) {
                        *bar_bounds.borrow_mut() = Some(rect);
                        window.refresh();
                    }
                },
                |_, (), _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .p(px(chrome::ZOOM_PAD))
        .shadow_lg()
        .child(icon_button::with_tooltip(
            zoom_out_name,
            icon_button::compact_sm("zoom-out", "minus", zoom_out_name)
                .width(px(ZOOM_BUTTON_SIZE))
                .height(px(ZOOM_BUTTON_SIZE))
                .is_disabled(zoom <= MIN_ZOOM)
                .on_press(move |_event, window, cx| zoom_out(window, cx)),
        ))
        .child(
            div()
                .w(px(chrome::ZOOM_RESET_MIN))
                .flex_shrink_0()
                .debug_selector(|| "zoom-reset-button".to_string())
                .child(
                    herogpui::components::Tooltip::new("Reset Zoom").child(
                        Button::new("zoom-reset")
                            .variant(Variant::Ghost)
                            .recipe("compact")
                            .recipe("muted")
                            .width(px(chrome::ZOOM_RESET_MIN))
                            .height(px(ZOOM_BUTTON_SIZE))
                            .label(format!("{}%", (zoom * 100.0).round() as i32))
                            .on_press(move |_event, window, cx| zoom_reset(window, cx)),
                    ),
                ),
        )
        .child(icon_button::with_tooltip(
            zoom_in_name,
            icon_button::compact_sm("zoom-in", "plus", zoom_in_name)
                .width(px(ZOOM_BUTTON_SIZE))
                .height(px(ZOOM_BUTTON_SIZE))
                .is_disabled(zoom >= MAX_ZOOM)
                .on_press(move |_event, window, cx| zoom_in(window, cx)),
        ))
        .into_any_element()
}

impl EditorWindow {
    fn tool_options_state(&self) -> ToolOptionsState {
        ToolOptionsState {
            tool: self.tool,
            color_hex: self.color_hex.clone().into(),
            stroke_width: self.stroke_width,
            arrow_style: self.arrow_style.clone().into(),
            highlight_opacity: self.highlight_opacity,
            number_style: self.number_style.clone().into(),
            number_size: self.number_size.clone().into(),
            number_start_value: self.number_start_value,
            text_background: self.text_background,
            text_font_size: self.text_font_size,
            text_font_family: self.text_font_family.clone().into(),
            redact_style: self.redact_style.clone().into(),
            redact_intensity: self.redact_intensity,
            shape_fill_mode: self.shape_fill_mode.clone().into(),
            wallpaper: self.wallpaper.clone(),
            has_layers: !self.layers.is_empty(),
        }
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.sync_snapshot(cx);
    }

    fn apply_option(&mut self, option: EditorOption, cx: &mut Context<Self>) {
        if self.text_editor.is_some()
            && matches!(
                option,
                EditorOption::Color(_)
                    | EditorOption::TextBackground(_)
                    | EditorOption::TextFontSize(_)
                    | EditorOption::TextFontFamily(_)
            )
        {
            if let Some(draft) = self.draft.as_mut() {
                draft.apply_option(&option);
            }
        }
        self.apply_option_to_selection(&option, cx);
        let persist = matches!(
            option,
            EditorOption::Tool(_)
                | EditorOption::Color(_)
                | EditorOption::StrokeWidth(_)
                | EditorOption::ArrowStyle(_)
                | EditorOption::HighlightOpacity(_)
                | EditorOption::HighlightColor(_)
                | EditorOption::NumberStyle(_)
                | EditorOption::NumberSize(_)
                | EditorOption::NumberStartValue(_)
                | EditorOption::TextBackground(_)
                | EditorOption::TextFontSize(_)
                | EditorOption::TextFontFamily(_)
                | EditorOption::RedactStyle(_)
                | EditorOption::RedactIntensity(_)
                | EditorOption::ShapeFillMode(_)
        );
        match option {
            EditorOption::Tool(tool) => self.set_tool(tool, cx),
            EditorOption::Color(value) => self.color_hex = value.to_string(),
            EditorOption::StrokeWidth(value) => self.stroke_width = value,
            EditorOption::ArrowStyle(value) => self.arrow_style = value.to_string(),
            EditorOption::HighlightOpacity(value) => self.highlight_opacity = value,
            EditorOption::HighlightColor(value) => self.highlight_color = value.to_string(),
            EditorOption::NumberStyle(value) => {
                self.number_style = value.to_string();
                self.renumber_current_without_history(cx);
            }
            EditorOption::NumberSize(value) => self.number_size = value.to_string(),
            EditorOption::NumberStartValue(value) => {
                self.number_start_value = value;
                self.renumber_current_without_history(cx);
            }
            EditorOption::TextBackground(value) => self.text_background = value,
            EditorOption::TextFontSize(value) => self.text_font_size = value,
            EditorOption::TextFontFamily(value) => self.text_font_family = value.to_string(),
            EditorOption::RedactStyle(value) => self.redact_style = value.to_string(),
            EditorOption::RedactIntensity(value) => self.redact_intensity = value,
            EditorOption::ShapeFillMode(value) => self.shape_fill_mode = value.to_string(),
            EditorOption::WallpaperGradient(value) => {
                let gradient = crate::editor::wallpaper::preset(&value);
                self.wallpaper.set_gradient(gradient);
            }
            EditorOption::WallpaperPadding(value) => self.wallpaper.padding = value,
            EditorOption::WallpaperCorners(value) => self.wallpaper.corners = value,
            EditorOption::WallpaperShadow(value) => self.wallpaper.shadow = value,
            EditorOption::WallpaperAspectRatio(value) => {
                self.wallpaper.aspect_ratio = value.to_string()
            }
            EditorOption::WallpaperFrame(value) => {
                self.wallpaper.window_frame.style = value.to_string()
            }
            EditorOption::WallpaperBalance(value) => self.wallpaper.balance = value,
            EditorOption::WallpaperBlur(value) => self.wallpaper.background_blur = value,
            EditorOption::WallpaperNoise(value) => self.wallpaper.noise = value,
            EditorOption::WallpaperInset(value) => self.wallpaper.inset = value,
            EditorOption::WallpaperSpacing(value) => self.wallpaper.spacing = value,
            EditorOption::ClearAttachedImages => self.clear_layers(cx),
            EditorOption::WallpaperUseDesktop => {
                if !crate::system::capabilities::is_supported(
                    crate::system::capabilities::Feature::DesktopWallpaper,
                ) {
                    return;
                }
                if let Some(source) = self.background_previews.desktop_source().map(str::to_owned) {
                    self.wallpaper.set_background_image(Some(source));
                } else {
                    let daemon = crate::state::state(cx).daemon;
                    let background = cx.background_executor().clone();
                    cx.spawn(async move |entity, cx| {
                        let source =
                            background
                                .spawn(async move {
                                    crate::editor::background::desktop_wallpaper(&daemon)
                                })
                                .await;
                        let _ = entity.update(cx, |editor, cx| match source {
                            Some(source) => {
                                editor
                                    .background_previews
                                    .set_desktop_source(source.clone());
                                editor.wallpaper.set_background_image(Some(source));
                                editor.refresh_backdrop(cx);
                                editor.sync_snapshot(cx);
                                cx.notify();
                            }
                            None => crate::windows::toast::Toast::show(
                                cx,
                                "Wallpaper unavailable",
                                "The desktop wallpaper could not be read.",
                            ),
                        });
                    })
                    .detach();
                }
            }
            EditorOption::WallpaperClear => {
                self.wallpaper.set_background_image(None);
                self.wallpaper.set_gradient(None);
            }
            EditorOption::WallpaperCustom(id) => {
                let config = crate::state::state(cx).config.get();
                if let Some(background) = config
                    .wallpaper
                    .custom_backgrounds
                    .iter()
                    .find(|background| background.id == id.as_ref())
                {
                    match &background.data {
                        crate::config::schema::CustomBackgroundData::Gradient { data } => {
                            self.wallpaper.set_gradient(Some(
                                crate::editor::wallpaper::GradientOption {
                                    id: data.gradient.id.clone(),
                                    colors: data.gradient.colors.clone(),
                                    angle: data.gradient.angle,
                                },
                            ));
                        }
                        crate::config::schema::CustomBackgroundData::Image { data } => {
                            self.wallpaper
                                .set_background_image(Some(data.image_url.clone()));
                        }
                    }
                }
            }
            EditorOption::WallpaperDeleteCustom(id) => {
                let using = crate::state::state(cx)
                    .config
                    .get()
                    .wallpaper
                    .custom_backgrounds
                    .iter()
                    .find(|background| background.id == id.as_ref())
                    .cloned();
                crate::state::state(cx).config.update(|settings| {
                    settings
                        .wallpaper
                        .custom_backgrounds
                        .retain(|background| background.id != id.as_ref());
                });
                if let Some(background) = using {
                    let matches = match &background.data {
                        crate::config::schema::CustomBackgroundData::Gradient { data } => {
                            self.wallpaper.gradient.as_ref().is_some_and(|gradient| {
                                gradient.id == background.id || gradient.id == data.gradient.id
                            })
                        }
                        crate::config::schema::CustomBackgroundData::Image { data } => {
                            self.wallpaper.background_image.as_deref()
                                == Some(data.image_url.as_str())
                        }
                    };
                    if matches {
                        self.wallpaper.set_background_image(None);
                        self.wallpaper.set_gradient(None);
                    }
                }
            }
            EditorOption::WallpaperApplyPreset(id) => {
                let config = crate::state::state(cx).config.get();
                if let Some(preset) = config
                    .wallpaper
                    .presets
                    .iter()
                    .find(|preset| preset.id == id.as_ref())
                    .cloned()
                {
                    crate::editor::wallpaper::apply_preset(&mut self.wallpaper, &preset);
                    self.wallpaper_preset_id = preset.id;
                }
            }
            EditorOption::WallpaperPresetDraftOpen => self.open_preset_draft(cx),
            EditorOption::WallpaperPresetDraftCancel => self.preset_draft = None,
            EditorOption::WallpaperSavePreset => {
                let Some(name) = self
                    .preset_draft
                    .as_ref()
                    .map(|draft| draft.field.read(cx).value().to_string())
                    .and_then(|value| crate::editor::wallpaper::preset_save_name(&value))
                else {
                    return;
                };
                let id = format!(
                    "preset-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis())
                        .unwrap_or(0)
                );
                let preset =
                    crate::editor::wallpaper::to_schema_preset(&self.wallpaper, id.clone(), name);
                crate::state::state(cx).config.update(|settings| {
                    settings.wallpaper.presets.push(preset);
                });
                self.wallpaper_preset_id = id;
                self.preset_draft = None;
            }
            EditorOption::WallpaperDeletePreset => {
                let selected = self.wallpaper_preset_id.clone();
                if !selected.is_empty() {
                    crate::state::state(cx).config.update(|settings| {
                        settings
                            .wallpaper
                            .presets
                            .retain(|preset| preset.id != selected);
                        if settings.wallpaper.default_preset_id.as_deref()
                            == Some(selected.as_str())
                        {
                            settings.wallpaper.default_preset_id = None;
                        }
                    });
                    self.wallpaper_preset_id.clear();
                }
            }
            EditorOption::WallpaperToggleDefaultPreset => {
                let selected = self.wallpaper_preset_id.clone();
                if !selected.is_empty() {
                    crate::state::state(cx).config.update(|settings| {
                        if settings.wallpaper.default_preset_id.as_deref()
                            == Some(selected.as_str())
                        {
                            settings.wallpaper.default_preset_id = None;
                        } else {
                            settings.wallpaper.default_preset_id = Some(selected.clone());
                        }
                    });
                }
            }
            EditorOption::WallpaperEditorOpen(id) => {
                self.open_background_editor(id.map(|id| id.to_string()), cx);
            }
            EditorOption::WallpaperEditorClose => {
                self.background_editor_generation =
                    self.background_editor_generation.wrapping_add(1);
                if let Some(preview) = self
                    .background_editor
                    .take()
                    .and_then(|draft| draft.preview)
                {
                    self.retired_preview_edges.push(preview);
                }
            }
            EditorOption::WallpaperEditorColor(index, value) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    if let Some(color) = draft.colors.get_mut(index) {
                        *color = value.to_string();
                    }
                }
            }
            EditorOption::WallpaperEditorAddColor => {
                if let Some(draft) = self.background_editor.as_mut() {
                    if draft.colors.len() < BACKGROUND_MAX_COLORS {
                        let color = BACKGROUND_PALETTE
                            .iter()
                            .find(|candidate| !draft.colors.iter().any(|c| c == *candidate))
                            .unwrap_or(&"#3b82f6")
                            .to_string();
                        draft
                            .fields
                            .push(cx.new(|cx| InputState::with_value(cx, color.clone())));
                        draft.colors.push(color);
                    }
                }
            }
            EditorOption::WallpaperEditorRemoveColor(index) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    if draft.colors.len() > BACKGROUND_MIN_COLORS && index < draft.colors.len() {
                        draft.colors.remove(index);
                        if index < draft.fields.len() {
                            draft.fields.remove(index);
                        }
                        draft.active_color = None;
                    }
                }
            }
            EditorOption::WallpaperEditorActiveColor(index) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    draft.active_color = if draft.active_color == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                }
            }
            EditorOption::WallpaperEditorPickColor(value) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    if let Some(index) = draft.active_color {
                        if let Some(color) = draft.colors.get_mut(index) {
                            *color = value.to_string();
                            if index < draft.fields.len() {
                                draft.fields[index] =
                                    cx.new(|cx| InputState::with_value(cx, value.to_string()));
                            }
                        }
                        draft.active_color = None;
                    }
                }
            }
            EditorOption::WallpaperEditorAngle(value) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    draft.angle = value.round().clamp(0.0, 360.0);
                }
            }
            EditorOption::WallpaperEditorTab(value) => {
                if let Some(draft) = self.background_editor.as_mut() {
                    draft.draft_type = if value.as_ref() == "image" {
                        BackgroundDraftType::Image
                    } else {
                        BackgroundDraftType::Gradient
                    };
                }
            }
            EditorOption::WallpaperEditorPickImage => {}
            EditorOption::WallpaperEditorSave => {
                self.save_background_editor(cx);
            }
            EditorOption::Zoom(value) => self.set_zoom(value, cx),
        }
        if persist {
            self.persist_preferences(cx);
        }
        self.sync_snapshot(cx);
    }

    #[cfg(test)]
    pub fn apply_option_for_test(&mut self, option: EditorOption, cx: &mut Context<Self>) {
        self.apply_option(option, cx);
    }

    fn open_background_editor(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.background_editor_generation = self.background_editor_generation.wrapping_add(1);
        if let Some(id) = id {
            let config = crate::state::state(cx).config.get();
            let existing = config
                .wallpaper
                .custom_backgrounds
                .iter()
                .find(|background| background.id == id);
            match existing.map(|existing| &existing.data) {
                Some(crate::config::schema::CustomBackgroundData::Gradient { data }) => {
                    let gradient = &data.gradient;
                    self.background_editor = Some(BackgroundDraft::new(
                        Some(id),
                        gradient.colors.clone(),
                        gradient.angle,
                        cx,
                    ));
                    return;
                }
                Some(crate::config::schema::CustomBackgroundData::Image { data }) => {
                    let mut draft = BackgroundDraft::new(
                        Some(id),
                        BACKGROUND_DEFAULT_COLORS
                            .iter()
                            .map(|color| color.to_string())
                            .collect(),
                        135.0,
                        cx,
                    );
                    draft.draft_type = BackgroundDraftType::Image;
                    draft.image_url = Some(data.image_url.clone());
                    draft.preview = crate::render::images::load_source(&data.image_url)
                        .map(|image| render_image_from_base(&image));
                    self.background_editor = Some(draft);
                    return;
                }
                None => {}
            }
        }
        self.background_editor = Some(BackgroundDraft::new(
            None,
            BACKGROUND_DEFAULT_COLORS
                .iter()
                .map(|color| color.to_string())
                .collect(),
            135.0,
            cx,
        ));
    }

    fn pick_background_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.background_picker_open || self.background_editor.is_none() {
            return;
        }
        self.background_picker_open = true;
        let generation = self.background_editor_generation;
        let dialog = crate::editor::background::image_picker(window);
        cx.spawn(async move |editor, cx| {
            let path = dialog
                .pick_file()
                .await
                .map(|file| file.path().to_string_lossy().to_string());
            let preview = if let Some(path) = path.as_ref() {
                let path = path.clone();
                cx.background_executor()
                    .spawn(async move {
                        crate::render::images::load_source(&path)
                            .map(|image| render_image_from_base(&image))
                    })
                    .await
            } else {
                None
            };
            let _ = editor.update(cx, |this, cx| {
                this.finish_background_image_picker(generation, path, preview, cx);
            });
        })
        .detach();
    }

    fn finish_background_image_picker(
        &mut self,
        generation: u64,
        path: Option<String>,
        preview: Option<Arc<gpui::RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.background_picker_open = false;
        if generation != self.background_editor_generation {
            cx.notify();
            return;
        }
        let Some(path) = path else {
            cx.notify();
            return;
        };
        let previous_preview = {
            let Some(draft) = self.background_editor.as_mut() else {
                cx.notify();
                return;
            };
            let previous = draft.preview.take();
            draft.preview = preview;
            draft.image_url = Some(path);
            previous
        };
        if let Some(previous) = previous_preview {
            self.retired_preview_edges.push(previous);
        }
        self.refresh_balance_crop();
        self.refresh_backdrop(cx);
        self.sync_snapshot(cx);
        cx.notify();
    }

    fn save_background_editor(&mut self, cx: &mut Context<Self>) {
        if self.background_picker_open {
            return;
        }
        let Some(draft) = self.background_editor.take() else {
            return;
        };
        self.background_editor_generation = self.background_editor_generation.wrapping_add(1);
        let id = draft.id.clone().unwrap_or_else(|| {
            format!(
                "custom-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0)
            )
        });
        if draft.draft_type == BackgroundDraftType::Image {
            let Some(image_url) = draft.image_url.clone() else {
                self.background_editor = Some(draft);
                return;
            };
            let background = crate::config::schema::CustomBackground {
                id: id.clone(),
                data: crate::config::schema::CustomBackgroundData::Image {
                    data: crate::config::schema::ImageBackgroundData {
                        image_url: image_url.clone(),
                    },
                },
            };
            crate::state::state(cx).config.update(|settings| {
                if let Some(existing) = settings
                    .wallpaper
                    .custom_backgrounds
                    .iter_mut()
                    .find(|existing| existing.id == id)
                {
                    *existing = background;
                } else {
                    settings.wallpaper.custom_backgrounds.push(background);
                }
            });
            self.wallpaper.set_gradient(None);
            self.wallpaper.set_background_image(Some(image_url));
            return;
        }
        let background = crate::config::schema::CustomBackground {
            id: id.clone(),
            data: crate::config::schema::CustomBackgroundData::Gradient {
                data: crate::config::schema::GradientBackgroundData {
                    gradient: crate::config::schema::GradientOption {
                        id: id.clone(),
                        colors: draft.colors.clone(),
                        angle: draft.angle,
                    },
                },
            },
        };
        crate::state::state(cx).config.update(|settings| {
            if let Some(existing) = settings
                .wallpaper
                .custom_backgrounds
                .iter_mut()
                .find(|existing| existing.id == id)
            {
                *existing = background;
            } else {
                settings.wallpaper.custom_backgrounds.push(background);
            }
        });
        self.wallpaper.set_background_image(None);
        self.wallpaper
            .set_gradient(Some(crate::editor::wallpaper::GradientOption {
                id,
                colors: draft.colors,
                angle: draft.angle,
            }));
    }

    fn persist_preferences(&self, cx: &mut Context<Self>) {
        let Some(service) = crate::state::try_state(cx) else {
            return;
        };
        let tool = self.tool;
        let color = self.color_hex.clone();
        let stroke_width = self.stroke_width;
        let arrow_style = self.arrow_style.clone();
        let highlight_color = self.highlight_color.clone();
        let highlight_opacity = self.highlight_opacity;
        let number_style = self.number_style.clone();
        let number_size = self.number_size.clone();
        let number_start_value = self.number_start_value;
        let text_background = self.text_background;
        let text_font_size = self.text_font_size;
        let text_font_family = self.text_font_family.clone();
        let redact_style = self.redact_style.clone();
        let redact_intensity = self.redact_intensity;
        let shape_fill_mode = self.shape_fill_mode.clone();
        service.config.update(|settings| {
            let editor = &mut settings.editor;
            editor.color = color;
            editor.stroke_width = stroke_width;
            editor.arrow_style = arrow_style;
            editor.highlight_color = highlight_color;
            editor.highlight_opacity = highlight_opacity;
            editor.number_style = number_style;
            editor.number_size = number_size;
            editor.number_start_value = number_start_value;
            editor.text_background = text_background;
            editor.text_font_size = text_font_size;
            editor.text_font_family = text_font_family;
            editor.redact_style = redact_style;
            editor.redact_intensity = redact_intensity;
            editor.shape_fill_mode = shape_fill_mode;
            if tool != Tool::Wallpaper {
                editor.last_tool = tool.id().to_string();
            }
        });
    }

    fn apply_option_to_selection(&mut self, option: &EditorOption, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        if self.selected_annotations.is_empty() {
            return;
        }
        let mut annotations = self.history.current().to_vec();
        let mut changed = false;
        for annotation in &mut annotations {
            if self
                .selected_annotations
                .iter()
                .any(|id| annotation.id() == id)
            {
                changed |= annotation.apply_option(option);
            }
        }
        if !changed || annotations == self.history.current() {
            return;
        }
        self.push_annotations(annotations, cx);
        self.refresh_raster_patches();
        self.sync_snapshot(cx);
    }

    fn apply_action(&mut self, action: EditorAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            EditorAction::Undo => self.undo(cx),
            EditorAction::Redo => self.redo(cx),
            EditorAction::Copy => self.copy_to_clipboard(window, cx),
            EditorAction::Save => self.save_as(window, cx),
            EditorAction::CloudUpload => self.upload_to_cloud(cx),
            EditorAction::CaptureToggle => self.toggle_capture_mode(cx),
            EditorAction::Pin => self.pin_screenshot(window, cx),
        }
    }

    fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        let tool = crate::editor::options::toggled_tool(self.tool, tool);
        if self.tool == tool {
            return;
        }
        self.commit_text(cx);
        if self.tool == Tool::Wallpaper {
            self.begin_sheet_close();
        }
        if self.tool == Tool::Crop {
            self.crop = None;
            self.crop_drag = None;
        }
        self.tool = tool;
        self.sync_snapshot(cx);
    }

    fn begin_sheet_close(&mut self) {
        self.preset_draft = None;
        self.sheet_closing = true;
    }

    fn schedule_sheet_unmount(&mut self, cx: &mut Context<Self>) {
        if self.sheet_close_task.is_some() {
            return;
        }
        self.sheet_close_task = Some(cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    crate::editor::wallpaper_sheet::SHEET_ANIMATION_MS,
                ))
                .await;
            let _ = entity.update(cx, |editor, cx| {
                editor.sheet_closing = false;
                editor.sheet_close_task = None;
                cx.notify();
            });
        }));
    }

    fn focus_text_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, field)) = &self.text_editor else {
            return;
        };
        let focus = field.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.prevent_default();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.finish_active_transform(cx);
        if let Some(pre) = self.pre_crop.take() {
            self.image_width = pre.width;
            self.image_height = pre.height;
            self.replace_image(render_image_from_base(&pre.base_image));
            self.base_image = Some(pre.base_image);
            self.inset_color = None;
            self.balance_crop = None;
            self.refresh_balance_crop();
            self.history.push(pre.annotations);
            self.refresh_raster_patches();
            self.sync_snapshot(cx);
            return;
        }
        self.history.undo();
        self.refresh_raster_patches();
        self.sync_snapshot(cx);
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        self.commit_text(cx);
        self.finish_active_transform(cx);
        self.history.redo();
        self.refresh_raster_patches();
        self.sync_snapshot(cx);
    }

    /// Port of `renumberAnnotations`: number badges count up from the start
    /// value in list order.
    fn renumber_annotations(&self, annotations: &mut [Annotation]) {
        let mut value = self.number_start_value;
        for annotation in annotations.iter_mut() {
            if let Annotation::Number {
                value: badge,
                display_value,
                ..
            } = annotation
            {
                *badge = value;
                *display_value =
                    crate::editor::options::number_display_value(value, &self.number_style);
                value += 1.0;
            }
        }
    }

    /// Port of the auto-renumber effect: style/start changes resequence
    /// badges in place, without pushing an undo step.
    fn renumber_current_without_history(&mut self, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        let mut annotations = self.history.current().to_vec();
        let before = annotations.clone();
        self.renumber_annotations(&mut annotations);
        if annotations != before {
            self.history.replace_current(annotations);
            self.refresh_raster_patches();
        }
    }

    /// Pushes an annotation revision from an add/delete path, dropping the
    /// pre-crop state the way Electron clears `lastCropStateRef` there.
    fn push_annotations(&mut self, annotations: Vec<Annotation>, cx: &mut Context<Self>) {
        self.finish_active_transform(cx);
        self.pre_crop = None;
        self.history.push(annotations);
    }
}
#[cfg(any(test, feature = "parity"))]
fn load_editor_image(path: &str) -> Option<PreparedScreenshot> {
    match PreparedScreenshot::open(path) {
        Ok(image) => Some(image),
        Err(error) => {
            eprintln!("[editor] failed to load {path}: {error}");
            None
        }
    }
}

fn render_image_from_base(base: &image::DynamicImage) -> Arc<gpui::RenderImage> {
    // GPUI composites in BGRA; swap channels once at decode time.
    let mut buffer = base.to_rgba8();
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(buffer)
    ]))
}

#[derive(Default)]
struct EditorCopyGeneration(u64);

impl gpui::Global for EditorCopyGeneration {}

struct ScreenshotComposition {
    base_image: Option<Arc<image::DynamicImage>>,
    width: u32,
    height: u32,
    annotations: Vec<Annotation>,
    wallpaper: crate::editor::wallpaper::WallpaperSettings,
    layers: Vec<crate::editor::layers::ImageLayer>,
    scale: f64,
}

impl ScreenshotComposition {
    fn render(self) -> image::RgbaImage {
        crate::editor::export::compose_with_layers(
            self.base_image.as_deref(),
            self.width,
            self.height,
            &self.annotations,
            &self.wallpaper.for_scale(self.scale),
            &self.layers,
            self.scale,
        )
    }

    fn encode(self, jpeg: bool) -> anyhow::Result<Vec<u8>> {
        if !jpeg {
            return match self.opaque_png_source() {
                Some(source) => crate::render::images::encode_srgb_png(source),
                None => crate::render::images::encode_srgb_png(&self.render()),
            };
        }
        let image = self.render();
        let image = image::DynamicImage::ImageRgba8(image).into_rgb8();
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92).encode_image(&image)?;
        Ok(bytes)
    }

    fn opaque_png_source(&self) -> Option<&image::RgbaImage> {
        if !self.annotations.is_empty()
            || !self.layers.is_empty()
            || self.wallpaper.is_active(false)
            || self.wallpaper.for_scale(self.scale).is_active(false)
        {
            return None;
        }
        let image = self.base_image.as_deref()?.as_rgba8()?;
        if image.width() == 0 || image.height() == 0 {
            return None;
        }
        image
            .as_raw()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
            .then_some(image)
    }

    fn write(self, path: &Path, jpeg: bool) -> anyhow::Result<Option<String>> {
        let bytes = self.encode(jpeg)?;
        #[cfg(windows)]
        let warning = crate::editor::screenshot_save::write(path, &bytes)?;
        #[cfg(unix)]
        let warning = crate::editor::screenshot_save_unix::write(path, &bytes)?;
        Ok(warning)
    }
}

impl EditorWindow {
    fn composition(&self) -> ScreenshotComposition {
        ScreenshotComposition {
            base_image: self.base_image.clone(),
            width: self.image_width as u32,
            height: self.image_height as u32,
            annotations: self.history.current().to_vec(),
            wallpaper: self.wallpaper.clone(),
            layers: self.layers.clone(),
            scale: self.native_scale,
        }
    }

    /// Renders the current composition (base + annotations) to PNG bytes at
    /// natural size.
    #[cfg(any(test, feature = "parity"))]
    pub fn export_png(&self) -> anyhow::Result<Vec<u8>> {
        let wallpaper = self.wallpaper.for_scale(self.native_scale);
        let canvas = crate::editor::export::compose_with_layers(
            self.base_image.as_deref(),
            self.image_width as u32,
            self.image_height as u32,
            self.history.current(),
            &wallpaper,
            &self.layers,
            self.native_scale,
        );
        crate::render::images::encode_srgb_png(&canvas)
    }

    fn pin_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::windows::pin::PinWindow;

        self.commit_text(cx);
        if self.pin_pending {
            return;
        }

        self.pin_pending = true;
        let composition = self.composition();
        let handle = window.window_handle();
        cx.spawn(async move |editor, cx| {
            let image = cx
                .background_executor()
                .spawn(async move { PinWindow::prepare_image(composition.render()) })
                .await;
            let Ok((path, state)) = editor.update(cx, |editor, cx| {
                editor.pin_pending = false;
                cx.notify();
                (
                    editor.file_path.clone(),
                    PersistedEditorState {
                        annotations: editor.history.current().to_vec(),
                        wallpaper: editor.wallpaper.clone(),
                        layers: editor.layers.clone(),
                    },
                )
            }) else {
                return;
            };
            cx.update(|cx| {
                if PinWindow::open_for_editor(cx, image, path, state) {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                } else {
                    crate::windows::toast::Toast::show(
                        cx,
                        "Pin failed",
                        "Could not open the pinned screenshot",
                    );
                }
            });
        })
        .detach();
    }

    pub fn copy_to_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text(cx);
        if self.copy_pending {
            return;
        }

        let generation = cx.default_global::<EditorCopyGeneration>();
        generation.0 = generation.0.wrapping_add(1);
        let generation = generation.0;
        let composition = self.composition();
        let close_on_copy = crate::state::state(cx)
            .config
            .get()
            .screenshot
            .close_on_copy;
        let handle = window.window_handle();
        self.copy_pending = true;
        self.is_copied = false;
        cx.notify();

        self.copy_task = Some(cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { composition.encode(false) })
                .await;
            let Ok(copied) = editor.update(cx, |editor, cx| {
                editor.copy_pending = false;
                cx.notify();
                if cx.global::<EditorCopyGeneration>().0 != generation {
                    return false;
                }
                match result {
                    Ok(png) => {
                        crate::system::clipboard::ClipboardService::write_png(cx, png);
                        editor.is_copied = true;
                        true
                    }
                    Err(error) => {
                        crate::windows::toast::Toast::show(cx, "Copy failed", error.to_string());
                        false
                    }
                }
            }) else {
                return;
            };
            if !copied {
                return;
            }
            if close_on_copy {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                return;
            }
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = editor.update(cx, |editor, cx| {
                editor.is_copied = false;
                cx.notify();
            });
        }));
    }

    /// Uploads the composed image to the user's own provider and copies the
    /// returned URL, mirroring `useCloudFileUpload`.
    pub fn upload_to_cloud(&mut self, cx: &mut Context<Self>) {
        use crate::cloud::UploadState;

        self.commit_text(cx);
        if self.cloud_upload == UploadState::Uploading {
            return;
        }
        let config = crate::state::state(cx).config.get().cloud;
        if !crate::cloud::is_configured(&config) {
            crate::intents::open_settings(crate::windows::settings::registry::Category::Cloud, cx);
            return;
        }
        let composition = self.composition();
        let name = crate::editor::filename::generate_screenshot_export_name("png");

        self.cloud_upload = UploadState::Uploading;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let uploaded = cx
                .background_executor()
                .spawn(async move {
                    let png = composition.encode(false)?;
                    crate::cloud::upload_bytes(&config, Path::new(&name), png)
                })
                .await;

            let state = if uploaded.is_ok() {
                UploadState::Success
            } else {
                UploadState::Error
            };

            cx.update(|cx| match uploaded {
                Ok(url) => {
                    crate::system::clipboard::ClipboardService::write_text(cx, url);
                    crate::windows::toast::Toast::show_transient(
                        cx,
                        "Image Uploaded",
                        "Link copied to clipboard",
                    );
                }
                Err(error) => {
                    crate::windows::toast::Toast::show(cx, "Upload failed", error.to_string());
                }
            });
            let _ = entity.update(cx, |editor, cx| {
                editor.cloud_upload = state;
                cx.notify();
            });

            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = entity.update(cx, |editor, cx| {
                if editor.cloud_upload == state {
                    editor.cloud_upload = UploadState::Idle;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Prints the composed image through the daemon's `print` module.
    pub fn print(&mut self, cx: &mut Context<Self>) {
        use base64::Engine as _;

        self.commit_text(cx);
        if !crate::system::capabilities::is_supported(crate::system::capabilities::Feature::Print) {
            crate::windows::toast::Toast::show(
                cx,
                "Printing unavailable",
                "This platform does not support printing",
            );
            return;
        }
        let composition = self.composition();
        let daemon = crate::state::state(cx).daemon;
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let png = composition.encode(false)?;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
                    daemon.print().image(encoded)
                })
                .await;
            if let Err(error) = result {
                cx.update(|cx| {
                    crate::windows::toast::Toast::show(cx, "Printing failed", error.to_string())
                });
            }
        })
        .detach();
    }

    /// Deletes the file this editor was opened from, after confirming.
    pub fn delete_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_path.is_empty() {
            return;
        }
        let confirmed = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Delete Screenshot")
            .set_description(format!("Delete {}?", self.file_path))
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                "Delete".into(),
                "Cancel".into(),
            ))
            .show();
        if confirmed != rfd::MessageDialogResult::Custom("Delete".into()) {
            return;
        }

        let path = std::path::PathBuf::from(&self.file_path);
        let (history_update_error, backup_warning) = match crate::history_store::delete_path(
            &path,
            crate::history_store::HistoryItemType::Screenshot,
        ) {
            crate::history_store::DeleteOutcome::Deleted => (None, None),
            crate::history_store::DeleteOutcome::DeletedWithBackupWarning(path) => {
                (None, Some(path))
            }
            crate::history_store::DeleteOutcome::HistoryPersistenceFailed(error) => {
                (Some(error), None)
            }
            crate::history_store::DeleteOutcome::SourceDeletionFailed
            | crate::history_store::DeleteOutcome::HistoryUnavailable
            | crate::history_store::DeleteOutcome::NotFound => {
                crate::windows::toast::Toast::show(
                    cx,
                    "Delete failed",
                    "The screenshot could not be deleted",
                );
                return;
            }
        };
        let notify = crate::state::state(cx)
            .config
            .get()
            .general
            .show_deletion_notifications;
        window.remove_window();
        if let Some(path) = backup_warning {
            crate::windows::toast::Toast::show(
                cx,
                "History recovery copy remains",
                format!("Could not remove the saved recovery copy at {path}"),
            );
        } else if let Some(error) = history_update_error {
            crate::windows::toast::Toast::show(
                cx,
                "History update failed",
                format!(
                    "The screenshot was deleted, but its history entry may return after restart: {error}"
                ),
            );
        } else if notify {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            cx.defer(move |cx| {
                crate::windows::toast::Toast::show(cx, "Screenshot deleted", name);
            });
        }
    }

    #[cfg(all(not(test), target_os = "macos", feature = "parity"))]
    pub(crate) fn parity_save_pending(&self) -> bool {
        self.save_dialog_open
    }

    #[cfg(all(not(test), target_os = "macos", feature = "parity"))]
    pub(crate) fn parity_save_to(
        &self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> gpui::Task<anyhow::Result<Option<String>>> {
        let composition = self.composition();
        cx.background_executor()
            .spawn(async move { composition.write(&path, false) })
    }

    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        eprintln!(
            "[diag-save] save_as entered save_dialog_open={}",
            self.save_dialog_open
        );
        self.commit_text(cx);
        if self.save_dialog_open {
            return;
        }

        let composition = self.composition();
        let settings = crate::state::state(cx).config.get();
        let jpeg = settings.screenshot.format == "jpeg";
        let extension = if jpeg { "jpg" } else { "png" };
        let default_name = crate::editor::filename::generate_screenshot_export_name(extension);
        let saved_directory = std::path::PathBuf::from(&settings.save_locations.screenshot);
        let directory = if saved_directory.is_dir() {
            saved_directory
        } else {
            dirs::picture_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
        };
        let dialog = rfd::AsyncFileDialog::new()
            .set_title("Save Screenshot")
            .set_parent(window)
            .set_file_name(&default_name)
            .add_filter(if jpeg { "JPEG Image" } else { "PNG Image" }, &[extension])
            .set_directory(&directory);
        self.save_dialog_open = true;
        let window_handle = window.window_handle();
        let config = crate::state::state(cx).config.clone();
        let close_on_save = settings.screenshot.close_on_save;

        cx.spawn(async move |editor, cx| {
            let Some(file) = dialog.save_file().await else {
                let _ = editor.update(cx, |editor, _| editor.save_dialog_open = false);
                return;
            };
            let path = file.path().to_path_buf();
            let failure_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    composition
                        .write(&path, jpeg)
                        .map(|warning| (path, warning))
                })
                .await;

            match result {
                Ok((path, cleanup_warning)) => {
                    if let Some(parent) = path.parent() {
                        let parent = parent.to_string_lossy().to_string();
                        config.update(move |config| config.save_locations.screenshot = parent);
                    }
                    let _ = editor.update(cx, |editor, cx| {
                        editor.save_dialog_open = false;
                        cx.notify();
                    });
                    if close_on_save {
                        let _ = window_handle.update(cx, |_, window, _| window.remove_window());
                    }
                    if let Some(warning) = cleanup_warning {
                        cx.update(|cx| {
                            crate::windows::toast::Toast::show(cx, "Screenshot saved", warning)
                        });
                    }
                }
                Err(error) => {
                    cx.update(|cx| report_screenshot_save_failure(cx, &failure_path, &error));
                    let _ = editor.update(cx, |editor, _| editor.save_dialog_open = false);
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herogpui::test::TestWindowExt;

    #[test]
    fn opaque_png_save_can_use_the_source_and_matches_the_composition() {
        for alpha in [255, 128] {
            let source = image::RgbaImage::from_fn(33, 25, |x, y| {
                image::Rgba([x as u8 * 7, y as u8 * 9, 140, alpha])
            });
            let snapshot = || ScreenshotComposition {
                base_image: Some(Arc::new(image::DynamicImage::ImageRgba8(source.clone()))),
                width: 33,
                height: 25,
                annotations: Vec::new(),
                wallpaper: Default::default(),
                layers: Vec::new(),
                scale: 2.0,
            };
            assert_eq!(snapshot().opaque_png_source().is_some(), alpha == 255);
            let composed = crate::render::images::encode_srgb_png(&snapshot().render()).unwrap();
            assert_eq!(snapshot().encode(false).unwrap(), composed);

            let mut framed = snapshot();
            framed.wallpaper.padding = 5.0;
            assert!(framed.opaque_png_source().is_none());
        }
    }

    #[test]
    fn jpeg_save_matches_the_png_based_encoding() {
        let base = Arc::new(image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(
            32,
            24,
            |x, y| image::Rgba([x as u8 * 8, y as u8 * 10, 140, (x + y) as u8 * 4]),
        )));
        let snapshot = || ScreenshotComposition {
            base_image: Some(base.clone()),
            width: 32,
            height: 24,
            annotations: Vec::new(),
            wallpaper: Default::default(),
            layers: Vec::new(),
            scale: 1.0,
        };
        let png = snapshot().encode(false).unwrap();
        let rgb = image::load_from_memory(&png).unwrap().into_rgb8();
        let mut expected = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut expected, 92)
            .encode_image(&rgb)
            .unwrap();
        assert_eq!(snapshot().encode(true).unwrap(), expected);
    }

    #[test]
    fn blackout_patch_covers_fractional_extents_and_clips_to_image() {
        let source = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            32,
            24,
            image::Rgba([255, 255, 255, 255]),
        ));
        for (rect, expected) in [
            ((8.8, 3.8, 12.3, 12.3), (8., 3., 14., 14.)),
            ((21.1, 16.1, -12.3, -12.3), (8., 3., 14., 14.)),
            ((-2.5, 3.8, 12.3, 12.3), (0., 3., 10., 14.)),
            ((28.8, 20.8, 12.3, 12.3), (28., 20., 4., 4.)),
        ] {
            let patch = render_redact_patch(&source, rect, "blackout", 5., 1.).unwrap();
            assert_eq!((patch.x, patch.y, patch.width, patch.height), expected);
            assert!(patch
                .image
                .as_bytes(0)
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [0, 0, 0, 255]));
        }
        assert!(render_redact_patch(&source, (8.8, 3.8, 0., 4.), "blackout", 5., 1.).is_none());
        assert!(render_redact_patch(&source, (8.8, 3.8, 4., 0.), "blackout", 5., 1.).is_none());
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn screenshot_save_failure_is_visible_to_the_user(cx: &mut gpui::TestAppContext) {
        let notifications = crate::system::notification::capture_for_test();
        let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected failure");
        cx.update(|cx| {
            report_screenshot_save_failure(cx, Path::new("capture.png"), &error);
        });
        assert_eq!(&*notifications.borrow(), &["Screenshot Save Failed"]);
    }

    #[herogpui::test]
    fn oversized_tiles_follow_edits_and_backdrop_retirement(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("long.png");
        image::RgbaImage::from_pixel(17_000, 4, image::Rgba([220, 40, 60, 128]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                let old: Vec<_> = editor
                    .image_tiles
                    .iter()
                    .map(|tile| tile.image.id)
                    .collect();
                assert_eq!(old.len(), 3);
                editor.crop = Some((100., 0., 5_000., 4.));
                editor.apply_crop(cx);
                assert!(editor.image_tiles.is_empty());
                assert!(old.iter().all(|id| editor
                    .retired_preview_edges
                    .iter()
                    .any(|image| image.id == *id)));
                assert_eq!(editor.image_width, 5_000.);
                editor.undo(cx);
                assert_eq!(editor.image_width, 17_000.);
                assert_eq!(editor.image_tiles.len(), 3);
                assert!(editor
                    .image_tiles
                    .iter()
                    .all(|tile| !old.contains(&tile.image.id)));
                assert_eq!(
                    editor
                        .base_image
                        .as_ref()
                        .unwrap()
                        .to_rgba8()
                        .get_pixel(0, 0)
                        .0,
                    [220, 40, 60, 128]
                );
                editor.backdrop_tiles =
                    crate::editor::image_tiles::create(editor.image.as_ref().unwrap());
                let backdrop_ids: Vec<_> = editor
                    .backdrop_tiles
                    .iter()
                    .map(|tile| tile.image.id)
                    .collect();
                assert_eq!(backdrop_ids.len(), 3);
                let matte = Arc::new(gpui::RenderImage::new(smallvec::smallvec![
                    image::Frame::new(image::RgbaImage::new(1, 1))
                ]));
                let matte_id = matte.id;
                editor.rounded_matte = Some(matte);
                editor.retire_backdrop();
                assert!(editor.backdrop_tiles.is_empty());
                assert!(editor.rounded_matte.is_none());
                assert!(editor
                    .retired_preview_edges
                    .iter()
                    .any(|image| image.id == matte_id));
                assert!(backdrop_ids.iter().all(|id| editor
                    .retired_preview_edges
                    .iter()
                    .any(|image| image.id == *id)));
            });
        });
    }

    #[herogpui::test]
    fn simple_backdrop_refreshes_when_source_changes_at_same_size(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.wallpaper.padding = 20.;
                editor.refresh_backdrop(cx);
            });
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.settle();
        let original = cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.backdrop_composite);
            editor.backdrop.as_ref().unwrap().id
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                let base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    8,
                    8,
                    image::Rgba([64, 80, 240, 255]),
                ));
                editor.image = Some(render_image_from_base(&base));
                editor.base_image = Some(Arc::new(base));
                editor.refresh_backdrop(cx);
            });
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.backdrop_composite);
            assert_ne!(editor.backdrop.as_ref().unwrap().id, original);
        });
    }

    #[herogpui::test]
    fn a_backdrop_render_coalesces_changes_that_arrive_while_it_runs(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.wallpaper.padding = 20.;
                editor.refresh_backdrop(cx);
                assert!(editor.backdrop_in_flight);
                assert!(!editor.backdrop_pending);
                editor.wallpaper.padding = 40.;
                editor.refresh_backdrop(cx);
                assert!(editor.backdrop_pending);
            });
        });
        for _ in 0..8 {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(16));
            cx.settle();
            if !cx.update(|_, cx| editor.read(cx).backdrop_in_flight) {
                break;
            }
        }
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(!editor.backdrop_in_flight);
            assert!(!editor.backdrop_pending);
            let expected = (8.0 + 40.0 * editor.native_scale * 2.0).round() as u32;
            let size = editor.backdrop.as_ref().unwrap().size(0);
            assert_eq!(u32::from(size.width), expected);
            assert_eq!(u32::from(size.height), expected);
        });
    }

    #[herogpui::test]
    fn backdrop_renders_immediately_without_debounce_timer(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(16, 12, image::Rgba([40, 120, 220, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .wallpaper
                    .set_gradient(crate::editor::wallpaper::preset("crimson-wave"));
                editor.wallpaper.padding = 24.;
                editor.refresh_backdrop(cx);
                editor.sync_snapshot(cx);
                assert!(matches!(
                    editor.snapshot.borrow().live_background,
                    LiveBackground::Image { cover: true, .. }
                ));
            });
        });
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(!editor.backdrop_in_flight);
            assert!(editor.backdrop.is_some());
        });
    }

    #[herogpui::test]
    fn geometry_and_inset_drag_keeps_live_background_and_inset_color_without_blank_frame(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(24, 18, image::Rgba([24, 180, 96, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .wallpaper
                    .set_gradient(crate::editor::wallpaper::preset("crimson-wave"));
                editor.wallpaper.padding = 30.;
                editor.refresh_backdrop(cx);
            });
        });
        cx.settle();
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.wallpaper.padding = 60.;
                editor.wallpaper.inset = 20.;
                editor.wallpaper.corners = 32.;
                editor.wallpaper.shadow = 80.;
                editor.refresh_backdrop(cx);
                editor.sync_snapshot(cx);
                let snapshot = editor.snapshot.borrow();
                assert!(snapshot.backdrop.is_none());
                assert!(matches!(
                    snapshot.live_background,
                    LiveBackground::Image { cover: true, .. }
                ));
                assert_eq!(snapshot.inset_color.as_deref(), Some("#18b460"));
            });
        });
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.backdrop.is_some());
            assert!(!editor.backdrop_in_flight);
        });
    }

    #[herogpui::test]
    fn noise_drag_preserves_backdrop_and_converges_without_clock_advance(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(16, 16, image::Rgba([80, 90, 100, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .wallpaper
                    .set_gradient(crate::editor::wallpaper::preset("ocean"));
                editor.wallpaper.padding = 20.;
                editor.refresh_backdrop(cx);
            });
        });
        cx.settle();
        let before = cx.update(|_, cx| editor.read(cx).backdrop.clone().unwrap());
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.wallpaper.noise = 35.;
                editor.refresh_backdrop(cx);
                assert!(editor
                    .backdrop
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &before)));
                editor.wallpaper.noise = 70.;
                editor.refresh_backdrop(cx);
                assert!(editor
                    .backdrop
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &before)));
                assert!(editor.backdrop_pending);
            });
        });
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(!editor.backdrop_in_flight);
            assert!(!editor.backdrop_pending);
            let final_backdrop = editor.backdrop.as_ref().unwrap();
            assert_ne!(final_backdrop.id, before.id);
            assert_eq!(
                editor.rendered_backdrop_key.as_ref().map(|key| key.0.noise),
                Some(70.)
            );
        });
    }

    #[herogpui::test]
    fn zoom_backdrop_replacement_drops_previous_atlas_image(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(1200, 900, image::Rgba([64, 112, 192, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            editor.update(cx, |editor, cx| {
                editor.set_zoom(2.0, cx);
                cx.notify();
            });
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let first_image = cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
            let image = editor.read(cx).zoom_backdrop.as_ref().unwrap().image();
            assert!(window.has_image_atlas_entry(&image));
            image
        });
        let second_image = cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.set_zoom(2.5, cx);
                cx.notify();
            });
            window.refresh();
            window.draw(cx).clear(cx);
            window.refresh();
            window.draw(cx).clear(cx);
            assert!(!window.has_image_atlas_entry(&first_image));
            let image = editor.read(cx).zoom_backdrop.as_ref().unwrap().image();
            assert!(window.has_image_atlas_entry(&image));
            image
        });
        assert!(!Arc::ptr_eq(&first_image, &second_image));
    }

    #[cfg(target_os = "macos")]
    #[herogpui::test]
    fn oversized_rounded_backdrop_refreshes_when_source_changes(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(17_000, 8, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.wallpaper.padding = 20.;
                editor.wallpaper.corners = 4.;
                editor.refresh_backdrop(cx);
            });
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.settle();
        let original = cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.rounded_matte.is_some());
            editor.backdrop.as_ref().unwrap().id
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                let base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    17_000,
                    8,
                    image::Rgba([64, 80, 240, 255]),
                ));
                editor.replace_image(render_image_from_base(&base));
                editor.base_image = Some(Arc::new(base));
                editor.refresh_backdrop(cx);
            });
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.rounded_matte.is_some());
            assert_ne!(editor.backdrop.as_ref().unwrap().id, original);
        });
    }

    #[herogpui::test]
    fn reset_zoom_button_accepts_a_pointer_press(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::new(320, 240).save(&source).unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor
        });
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.apply_option_for_test(EditorOption::Zoom(1.5), cx);
            });
            window.refresh();
        });
        cx.settle();
        assert_eq!(cx.update(|_, app| editor.read(app).zoom), 1.5);
        let bounds = cx.expect("zoom-reset-button");
        assert_eq!(bounds.size.width, px(chrome::ZOOM_RESET_MIN));
        let center_y = bounds.center().y;

        cx.click_at(gpui::point(bounds.left() + px(2.0), center_y));

        assert_eq!(cx.update(|_, app| editor.read(app).zoom), 1.0);
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.apply_option_for_test(EditorOption::Zoom(1.5), cx);
            });
            window.refresh();
        });
        cx.settle();
        cx.click_at(gpui::point(bounds.right() - px(2.0), center_y));
        assert_eq!(cx.update(|_, app| editor.read(app).zoom), 1.0);
    }

    #[herogpui::test]
    fn background_image_picker_ignores_cancel_and_superseded_drafts(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            EditorWindow::from_file(&source_path, window, cx)
        });
        editor.update(cx, |editor, cx| editor.open_background_editor(None, cx));

        let generation = cx.update(|_, app| editor.read(app).background_editor_generation);
        let preview = render_image_from_base(&image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255])),
        ));
        editor.update(cx, |editor, cx| {
            editor.background_picker_open = true;
            editor.finish_background_image_picker(generation, None, None, cx);
            assert!(!editor.background_picker_open);
            assert!(editor
                .background_editor
                .as_ref()
                .unwrap()
                .image_url
                .is_none());

            editor.background_picker_open = true;
            editor.open_background_editor(None, cx);
            editor.finish_background_image_picker(
                generation,
                Some(source_path.clone()),
                Some(preview.clone()),
                cx,
            );
            assert!(!editor.background_picker_open);
            assert!(editor
                .background_editor
                .as_ref()
                .unwrap()
                .image_url
                .is_none());

            let generation = editor.background_editor_generation;
            editor.background_picker_open = true;
            editor.finish_background_image_picker(
                generation,
                Some(source_path.clone()),
                Some(preview.clone()),
                cx,
            );
            let draft = editor.background_editor.as_ref().unwrap();
            assert_eq!(draft.image_url.as_deref(), Some(source_path.as_str()));
            assert!(draft.preview.is_some());
            assert!(!editor.background_picker_open);
        });
    }

    #[herogpui::test]
    fn background_image_picker_keeps_replacement_draft_until_preview_load_finishes(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store.clone()));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            EditorWindow::from_file(&source_path, window, cx)
        });
        editor.update(cx, |editor, cx| {
            let mut draft = BackgroundDraft::new(
                Some("existing".into()),
                BACKGROUND_DEFAULT_COLORS
                    .iter()
                    .map(|color| color.to_string())
                    .collect(),
                135.0,
                cx,
            );
            draft.draft_type = BackgroundDraftType::Image;
            draft.image_url = Some("old-image.png".into());
            editor.background_editor = Some(draft);
            editor.background_picker_open = true;
            let generation = editor.background_editor_generation;

            editor.save_background_editor(cx);

            assert!(editor.background_picker_open);
            assert_eq!(
                editor
                    .background_editor
                    .as_ref()
                    .and_then(|draft| draft.image_url.as_deref()),
                Some("old-image.png")
            );
            assert!(store.get().wallpaper.custom_backgrounds.is_empty());

            let preview = render_image_from_base(&image::DynamicImage::ImageRgba8(
                image::RgbaImage::from_pixel(8, 8, image::Rgba([240, 80, 64, 255])),
            ));
            editor.finish_background_image_picker(
                generation,
                Some(source_path.clone()),
                Some(preview),
                cx,
            );
            assert!(!editor.background_picker_open);

            editor.save_background_editor(cx);
            assert_eq!(
                store
                    .get()
                    .wallpaper
                    .custom_backgrounds
                    .iter()
                    .find(|background| background.id == "existing")
                    .and_then(|background| match &background.data {
                        crate::config::schema::CustomBackgroundData::Image { data } => {
                            Some(data.image_url.as_str())
                        }
                        _ => None,
                    }),
                Some(source_path.as_str())
            );
        });
    }

    #[herogpui::test]
    fn editor_copy_publishes_the_requested_composition(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(160, 100, image::Rgba([72, 104, 224, 255]))
            .save(&path)
            .unwrap();
        let path = path.to_str().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config.clone()));

        for close_on_copy in [false, true] {
            config.update(|config| config.screenshot.close_on_copy = close_on_copy);
            let (handle, expected) = cx.update(|cx| {
                use crate::system::clipboard::ClipboardService;
                ClipboardService::write_text(cx, String::new());
                assert!(crate::open_editor_with_state(
                    cx,
                    path,
                    Some(Default::default())
                ));
                let handle = cx
                    .windows()
                    .into_iter()
                    .find_map(|handle| handle.downcast::<EditorWindow>())
                    .unwrap();
                let expected = handle
                    .update(cx, |editor, window, cx| {
                        editor.wallpaper.padding = 20.;
                        let expected = editor.export_png().unwrap();
                        editor.copy_to_clipboard(window, cx);
                        assert!(editor.copy_pending);
                        assert!(ClipboardService::read_image(cx).is_none());
                        editor.wallpaper.padding = 40.;
                        expected
                    })
                    .unwrap();
                (handle, expected)
            });

            cx.run_until_parked();
            cx.update(|cx| {
                let copied = crate::system::clipboard::ClipboardService::read_image(cx).unwrap();
                assert_eq!(copied.bytes, expected);
                if close_on_copy {
                    assert!(handle.update(cx, |_, _, _| ()).is_err());
                } else {
                    handle
                        .update(cx, |editor, window, _| {
                            assert!(!editor.copy_pending);
                            assert!(editor.is_copied);
                            window.remove_window();
                        })
                        .unwrap();
                }
            });
        }
    }

    #[herogpui::test]
    fn cloud_upload_sends_the_requested_composition(cx: &mut gpui::TestAppContext) {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/screenshot.png");
        let response_url = url.clone();
        let receiver = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = BufReader::new(stream);
            let mut line = String::new();
            request.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /upload "));
            let mut length = None;
            loop {
                line.clear();
                assert!(request.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("Content-Length") {
                        length = Some(value.trim().parse().unwrap());
                    }
                }
            }
            let mut payload = vec![0; length.unwrap()];
            request.read_exact(&mut payload).unwrap();
            write!(
                request.get_mut(),
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_url}",
                response_url.len(),
            )
            .unwrap();
            payload
        });

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(160, 100, image::Rgba([72, 104, 224, 255]))
            .save(&path)
            .unwrap();
        let path = path.to_str().unwrap();
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json")).unwrap(),
        );
        config.update(|config| {
            config.cloud.enabled = true;
            config.cloud.active_provider = "rest".into();
            config.cloud.rest.url = format!("http://{address}/upload");
            config.cloud.rest.response_is_plain_text = true;
        });
        let (handle, expected) = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            assert!(crate::open_editor_with_state(
                cx,
                path,
                Some(Default::default())
            ));
            let handle = cx
                .windows()
                .into_iter()
                .find_map(|handle| handle.downcast::<EditorWindow>())
                .unwrap();
            let expected = handle
                .update(cx, |editor, _, cx| {
                    editor.wallpaper.padding = 20.;
                    let expected = editor.export_png().unwrap();
                    editor.upload_to_cloud(cx);
                    assert_eq!(editor.cloud_upload, crate::cloud::UploadState::Uploading);
                    editor.wallpaper.padding = 40.;
                    expected
                })
                .unwrap();
            (handle, expected)
        });
        cx.run_until_parked();
        handle
            .update(cx, |editor, _, _| {
                assert_eq!(editor.cloud_upload, crate::cloud::UploadState::Success);
            })
            .unwrap();
        cx.update(|cx| {
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some(url.as_str())
            );
        });
        let payload = receiver.join().unwrap();
        assert!(payload.windows(expected.len()).any(|part| part == expected));
    }

    #[herogpui::test]
    fn wallpaper_export_matches_polish_at_two_x(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(1600, 1000, image::Rgba([240, 80, 64, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            window.set_scale_factor(2.);
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor.wallpaper.padding = 50.;
            editor.wallpaper.corners = 12.;
            editor
        });
        let bytes = cx.update(|_, cx| editor.read(cx).export_png().unwrap());
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (1800, 1200)
        );
    }

    #[herogpui::test]
    fn reopening_framed_balanced_layers_restores_crop_and_inset(cx: &mut gpui::TestAppContext) {
        use crate::editor::layers::{Edge, ImageLayer};

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_fn(100, 80, |x, y| {
            if (30..80).contains(&x) && (25..65).contains(&y) {
                image::Rgba([64, 112, 192, 255])
            } else if (10..90).contains(&x) && (10..70).contains(&y) {
                image::Rgba([235, 235, 235, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .save(&source)
        .unwrap();
        let attached = directory.path().join("top.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([240, 80, 64, 255]))
            .save(&attached)
            .unwrap();
        let attached_right = directory.path().join("right.png");
        image::RgbaImage::from_pixel(30, 40, image::Rgba([72, 192, 112, 255]))
            .save(&attached_right)
            .unwrap();
        let mut wallpaper = crate::editor::wallpaper::WallpaperSettings {
            balance: true,
            inset: 4.0,
            spacing: 5.0,
            ..crate::editor::wallpaper::WallpaperSettings::default()
        };
        wallpaper.window_frame.style = "windows-dark".into();
        let persisted = PersistedEditorState {
            annotations: vec![Annotation::Rectangle {
                id: "balance-probe".into(),
                x: 35.0,
                y: 30.0,
                width: 10.0,
                height: 10.0,
                stroke: "#ff00ff".into(),
                stroke_width: 2.0,
                fill: None,
            }],
            wallpaper,
            layers: vec![
                ImageLayer {
                    id: "top".into(),
                    image_url: attached.to_string_lossy().into_owned(),
                    natural_width: 40.0,
                    natural_height: 20.0,
                    edge: Edge::Top,
                },
                ImageLayer {
                    id: "right".into(),
                    image_url: attached_right.to_string_lossy().into_owned(),
                    natural_width: 30.0,
                    natural_height: 40.0,
                    edge: Edge::Right,
                },
            ],
        };
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, move |window, cx| {
            window.set_scale_factor(1.0);
            let mut editor =
                EditorWindow::from_file_with_state(&source_path, persisted, window, cx);
            editor.file_path.clear();
            editor
        });
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.settle();

        let (crop, content_size, inset_color, exported, selected) = cx.update(|_, cx| {
            let editor = editor.read(cx);
            let bounds = editor.bounds.borrow().unwrap();
            let point = to_canvas_point(
                gpui::point(
                    bounds.origin.x + px(35.0 * editor.zoom),
                    bounds.origin.y + px(30.0 * editor.zoom),
                ),
                Some(bounds),
                editor.zoom,
            );
            (
                editor.balance_crop,
                editor.content_size(),
                editor.inset_color.clone(),
                editor.export_png().unwrap(),
                editor.annotation_at(point),
            )
        });
        assert_eq!(crop, Some((20.0, 15.0, 10.0, 5.0)));
        assert_eq!(inset_color.as_deref(), Some("#ebebeb"));
        assert_eq!(content_size, (131.0, 192.0));
        assert_eq!(selected.as_deref(), Some("balance-probe"));
        assert_eq!(
            image::load_from_memory(&exported)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (131, 192)
        );
    }

    #[herogpui::test]
    fn pointer_updates_schedule_a_frame_and_keep_every_stroke_sample(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::new(320, 240).save(&source).unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor.tool = Tool::Pen;
            editor
        });
        cx.settle();
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor.start_stroke(point(10., 10.), false, window, cx);
                editor.queue_pointer_update(point(20., 20.), false, window, cx);
                editor.queue_pointer_update(point(30., 30.), false, window, cx);
                assert!(editor.pointer_update_scheduled);
            })
        });
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        cx.update(|_, cx| editor.update(cx, |editor, cx| {
            assert!(!editor.pointer_update_scheduled);
            editor.finish_stroke(cx);
            assert!(matches!(&editor.history.current()[0], Annotation::Pen { points, .. } if points == &[10., 10., 20., 20., 30., 30.]));
        }));
    }

    #[herogpui::test]
    fn selection_and_resize_keep_redo_until_an_annotation_changes(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::new(320, 240).save(&source).unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor.tool = Tool::Select;
            editor
        });
        let original = Annotation::Rectangle {
            id: "rectangle".into(),
            x: 20.,
            y: 20.,
            width: 80.,
            height: 80.,
            stroke: "#ff00ff".into(),
            stroke_width: 2.,
            fill: Some("#ff00ff".into()),
        };
        let mut later = original.clone();
        later.translate(5., 0.);
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                let mut results = Vec::new();
                for resize in [false, true] {
                    editor.history = annotations::AnnotationHistory::new(vec![original.clone()]);
                    editor.history.push(vec![later.clone()]);
                    assert!(editor.history.undo());
                    editor.selected_annotations = if resize {
                        vec!["rectangle".into()]
                    } else {
                        Vec::new()
                    };
                    let point = if resize { point(100., 100.) } else { point(60., 60.) };
                    editor.start_stroke(point, false, window, cx);
                    assert_eq!(editor.selected_annotations, ["rectangle"]);
                    if resize {
                        assert!(editor.resize_state.is_some() && editor.move_origin.is_none());
                        editor.resize_selected(point);
                    } else {
                        assert!(editor.move_origin.is_some() && editor.resize_state.is_none());
                        editor.move_selected(point);
                    }
                    editor.finish_stroke(cx);
                    results.push((
                        editor.history.can_redo(),
                        editor.history.len(),
                        editor.history.current() == [original.clone()],
                    ));
                    if editor.history.can_redo() {
                        assert!(editor.history.redo());
                        assert_eq!(editor.history.current(), std::slice::from_ref(&later));
                        assert!(editor.history.undo());
                    }
                }
                assert_eq!(results, [(true, 2, true), (true, 2, true)]);

                for option in [
                    EditorOption::Color("#ff00ff".into()),
                    EditorOption::StrokeWidth(2.),
                    EditorOption::ShapeFillMode("filled".into()),
                ] {
                    editor.history = annotations::AnnotationHistory::new(vec![original.clone()]);
                    editor.history.push(vec![later.clone()]);
                    assert!(editor.history.undo());
                    editor.selected_annotations = vec!["rectangle".into()];
                    editor.apply_option_to_selection(&option, cx);
                    assert!(editor.history.can_redo());
                    assert_eq!(editor.history.current(), std::slice::from_ref(&original));
                    assert!(editor.history.redo());
                    assert_eq!(editor.history.current(), std::slice::from_ref(&later));
                }
                let mut other = original.clone();
                if let Annotation::Rectangle { id, x, stroke, fill, .. } = &mut other {
                    *id = "other".into();
                    *x = 120.;
                    *stroke = "#00ff00".into();
                    *fill = Some("#00ff00".into());
                }
                editor.history =
                    annotations::AnnotationHistory::new(vec![original.clone(), other.clone()]);
                editor.history.push(vec![later.clone(), other.clone()]);
                assert!(editor.history.undo());
                editor.selected_annotations = vec!["rectangle".into(), "other".into()];
                editor.apply_option_to_selection(&EditorOption::Color("#ff00ff".into()), cx);
                let mut recolored_other = other.clone();
                recolored_other.apply_option(&EditorOption::Color("#ff00ff".into()));
                assert_eq!(editor.history.current(), [original.clone(), recolored_other]);
                assert!(!editor.history.can_redo());
                assert!(editor.history.undo());
                assert_eq!(editor.history.current(), [original.clone(), other]);

                results.clear();
                for (resize, start, away) in [
                    (false, point(60., 60.), point(70., 70.)),
                    (true, point(20., 20.), point(30., 30.)),
                    (true, point(100., 20.), point(110., 30.)),
                    (true, point(20., 100.), point(30., 110.)),
                    (true, point(100., 100.), point(110., 110.)),
                ] {
                    editor.history = annotations::AnnotationHistory::new(vec![original.clone()]);
                    editor.history.push(vec![later.clone()]);
                    assert!(editor.history.undo());
                    editor.selected_annotations = if resize {
                        vec!["rectangle".into()]
                    } else {
                        Vec::new()
                    };
                    editor.start_stroke(start, false, window, cx);
                    assert_eq!(editor.selected_annotations, ["rectangle"]);
                    if resize {
                        assert!(editor.resize_state.is_some() && editor.move_origin.is_none());
                        editor.resize_selected(away);
                        editor.resize_selected(start);
                    } else {
                        assert!(editor.move_origin.is_some() && editor.resize_state.is_none());
                        editor.move_selected(away);
                        editor.move_selected(start);
                    }
                    editor.finish_stroke(cx);
                    results.push((
                        editor.history.can_redo(),
                        editor.history.len(),
                        editor.history.current() == [original.clone()],
                    ));
                    if editor.history.can_redo() {
                        assert!(editor.history.redo());
                        assert_eq!(editor.history.current(), std::slice::from_ref(&later));
                        assert!(editor.history.undo());
                    }
                }
                assert_eq!(results, [(true, 2, true); 5]);

                let limited = Annotation::Circle {
                    id: "small-circle".into(),
                    x: 100.,
                    y: 100.,
                    radius: 5.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 2.,
                    fill: None,
                };
                let mut larger = limited.clone();
                if let Annotation::Circle { radius, .. } = &mut larger {
                    *radius = 10.;
                }
                editor.history = annotations::AnnotationHistory::new(vec![limited.clone()]);
                editor.history.push(vec![larger.clone()]);
                assert!(editor.history.undo());
                editor.selected_annotations = vec!["small-circle".into()];
                editor.begin_resize(ResizeHandle::BottomRight, point(104., 104.));
                assert!(editor.resize_state.is_some());
                editor.resize_selected(point(103., 103.));
                editor.finish_stroke(cx);
                assert!(editor.history.can_redo());
                assert_eq!(editor.history.current(), std::slice::from_ref(&limited));
                editor.begin_resize(ResizeHandle::BottomRight, point(104., 104.));
                editor.resize_selected(point(103., 103.));
                editor.resize_selected(point(104., 104.));
                editor.finish_stroke(cx);
                assert!(editor.history.can_redo());
                assert_eq!(editor.history.current(), [limited]);
                assert!(editor.history.redo());
                assert_eq!(editor.history.current(), [larger]);

                editor.history = annotations::AnnotationHistory::new(vec![original.clone()]);
                editor.history.push(vec![later.clone()]);
                assert!(editor.history.undo());
                editor.selected_annotations = vec!["rectangle".into()];
                editor.editor_state_write_task = None;
                editor.start_stroke(point(60., 60.), false, window, cx);
                assert!(!editor.snapshot.borrow().moving);
                assert!(editor.editor_state_write_task.is_none());
                editor.move_selected(point(70., 65.));
                editor.sync_interactive_snapshot();
                assert!(editor.snapshot.borrow().moving);
                assert!(editor.editor_state_write_task.is_none());
                editor.move_selected(point(72., 70.));
                editor.sync_interactive_snapshot();
                assert!(editor.snapshot.borrow().moving);
                editor.finish_stroke(cx);
                assert!(!editor.snapshot.borrow().moving);
                let mut moved = original.clone();
                moved.translate(12., 10.);
                assert_eq!(editor.history.current(), [moved.clone()]);
                assert!(!editor.history.can_redo());
                assert!(editor.history.undo());
                assert_eq!(editor.history.current(), std::slice::from_ref(&original));
                assert!(editor.history.redo());
                assert_eq!(editor.history.current(), [moved.clone()]);

                editor.start_stroke(point(72., 70.), false, window, cx);
                editor.queue_pointer_update(point(77., 75.), false, window, cx);
                editor.undo(cx);
                assert!(editor.move_origin.is_none());
                assert!(!editor.pointer_update_scheduled);
                assert_eq!(editor.history.current(), [moved.clone()]);
                let mut moved_again = moved.clone();
                moved_again.translate(5., 5.);
                editor.redo(cx);
                assert_eq!(editor.history.current(), [moved_again]);
                editor.undo(cx);
                assert_eq!(editor.history.current(), [moved.clone()]);

                editor.start_stroke(point(112., 110.), false, window, cx);
                editor.resize_selected(point(122., 120.));
                editor.finish_stroke(cx);
                assert!(matches!(editor.history.current(), [Annotation::Rectangle { width, height, .. }] if *width == 90. && *height == 90.));
                assert!(editor.history.undo());
                assert_eq!(editor.history.current(), [moved.clone()]);
                assert!(editor.history.redo());
                assert!(matches!(editor.history.current(), [Annotation::Rectangle { width, height, .. }] if *width == 90. && *height == 90.));

                editor.history = annotations::AnnotationHistory::new(vec![original.clone()]);
                editor.selected_annotations.clear();
                editor.start_stroke(point(60., 60.), false, window, cx);
                editor.queue_pointer_update(point(66., 63.), false, window, cx);
                editor.apply_option_to_selection(&EditorOption::Color("#00ff00".into()), cx);
                let mut moved_for_option = original.clone();
                moved_for_option.translate(6., 3.);
                let mut recolored = moved_for_option.clone();
                recolored.apply_option(&EditorOption::Color("#00ff00".into()));
                assert_eq!(editor.history.current(), [recolored]);
                assert!(editor.history.undo());
                assert_eq!(editor.history.current(), [moved_for_option]);
                assert!(editor.history.undo());
                assert_eq!(editor.history.current(), std::slice::from_ref(&original));
            });
        });
    }

    #[herogpui::test]
    fn desktop_wallpaper_selection_reuses_cached_source_without_daemon_roundtrip(
        cx: &mut gpui::TestAppContext,
    ) {
        if !crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::DesktopWallpaper,
        ) {
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([40, 80, 120, 255]))
            .save(&source)
            .unwrap();
        let wallpaper_file = directory.path().join("desktop.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([200, 60, 40, 255]))
            .save(&wallpaper_file)
            .unwrap();
        let wallpaper_path = wallpaper_file.to_string_lossy().into_owned();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor
                .background_previews
                .set_desktop(Some(wallpaper_path.clone()), None);
            editor
        });
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.apply_option(EditorOption::WallpaperUseDesktop, cx);
                assert_eq!(
                    editor.wallpaper.background_image.as_deref(),
                    Some(wallpaper_path.as_str())
                );
                assert_eq!(editor.wallpaper.padding, 50.0);
                editor.refresh_backdrop(cx);
            });
        });
        cx.settle();
        cx.update(|_, cx| {
            let editor = editor.read(cx);
            assert!(editor.backdrop.is_some());
            assert!(editor.base_pixmap.is_some());
        });
    }

    #[herogpui::test]
    fn selecting_through_a_pen_gap_moves_the_visible_lower_annotation(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::new(160, 120).save(&source).unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor.tool = Tool::Select;
            editor.history = annotations::AnnotationHistory::new(vec![
                Annotation::Rectangle {
                    id: "lower-fill".into(),
                    x: 40.0,
                    y: 40.0,
                    width: 20.0,
                    height: 20.0,
                    stroke: "#ff0000".into(),
                    stroke_width: 2.0,
                    fill: Some("#ff0000".into()),
                },
                Annotation::Pen {
                    id: "top-u-stroke".into(),
                    points: vec![10.0, 10.0, 110.0, 10.0, 110.0, 110.0, 10.0, 110.0],
                    stroke: "#000000".into(),
                    stroke_width: 8.0,
                },
            ]);
            editor.sync_snapshot(cx);
            editor
        });
        cx.settle();

        let screen_point = |cx: &mut gpui::VisualTestContext, image_x: f32, image_y: f32| {
            cx.update(|_, app| {
                let editor = editor.read(app);
                let bounds = editor.bounds.borrow().unwrap();
                gpui::point(
                    bounds.origin.x + px(image_x * editor.zoom),
                    bounds.origin.y + px(image_y * editor.zoom),
                )
            })
        };
        let start = screen_point(cx, 50.0, 50.0);
        let end = screen_point(cx, 60.0, 55.0);
        cx.simulate_mouse_move(start, None, gpui::Modifiers::none());
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), gpui::Modifiers::none());
        cx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.settle();

        let (selected, lower_position, top_points) = cx.update(|_, app| {
            let editor = editor.read(app);
            let lower = editor
                .history
                .current()
                .iter()
                .find(|annotation| annotation.id() == "lower-fill")
                .unwrap();
            let top = editor
                .history
                .current()
                .iter()
                .find(|annotation| annotation.id() == "top-u-stroke")
                .unwrap();
            let Annotation::Rectangle { x, y, .. } = lower else {
                panic!("lower annotation changed type")
            };
            let Annotation::Pen { points, .. } = top else {
                panic!("top annotation changed type")
            };
            (
                editor.selected_annotations.clone(),
                (*x, *y),
                points.clone(),
            )
        });
        assert_eq!(selected, ["lower-fill"]);
        assert_eq!(lower_position, (50.0, 45.0));
        assert_eq!(
            top_points,
            [10.0, 10.0, 110.0, 10.0, 110.0, 110.0, 10.0, 110.0]
        );

        let top_stroke = screen_point(cx, 60.0, 10.0);
        cx.simulate_mouse_move(top_stroke, None, gpui::Modifiers::none());
        cx.simulate_click(top_stroke, gpui::Modifiers::none());
        cx.settle();
        assert_eq!(
            cx.update(|_, app| editor.read(app).selected_annotations.clone()),
            ["top-u-stroke"]
        );
    }

    #[herogpui::test]
    fn overlapping_highlight_selects_above_circle_without_mutating_saved_pixels(
        cx: &mut gpui::TestAppContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::new(160, 120).save(&source).unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(source.to_str().unwrap(), window, cx);
            editor.file_path.clear();
            editor.tool = Tool::Select;
            editor.history = annotations::AnnotationHistory::new(vec![
                Annotation::Circle {
                    id: "lower-circle".into(),
                    x: 80.0,
                    y: 60.0,
                    radius: 30.0,
                    stroke: "#ff0000".into(),
                    stroke_width: 2.0,
                    fill: Some("#ff0000".into()),
                },
                Annotation::Highlight {
                    id: "top-highlight".into(),
                    points: vec![30.0, 60.0, 130.0, 60.0],
                    fill: "#0000ff".into(),
                    opacity: 0.5,
                    stroke_width: 20.0,
                },
            ]);
            editor.sync_snapshot(cx);
            editor
        });
        cx.settle();

        let screen_point = |cx: &mut gpui::VisualTestContext, image_x: f32, image_y: f32| {
            cx.update(|_, app| {
                let editor = editor.read(app);
                let bounds = editor.bounds.borrow().unwrap();
                gpui::point(
                    bounds.origin.x + px(image_x * editor.zoom),
                    bounds.origin.y + px(image_y * editor.zoom),
                )
            })
        };
        let initial = cx.update(|_, app| {
            let editor = editor.read(app);
            (
                editor.history.current().to_vec(),
                editor.export_png().unwrap(),
            )
        });

        let overlap = screen_point(cx, 80.0, 60.0);
        cx.simulate_mouse_move(overlap, None, gpui::Modifiers::none());
        cx.simulate_click(overlap, gpui::Modifiers::none());
        cx.settle();
        assert_eq!(
            cx.update(|_, app| editor.read(app).selected_annotations.clone()),
            ["top-highlight"]
        );

        let circle_only = screen_point(cx, 80.0, 44.0);
        cx.simulate_mouse_move(circle_only, None, gpui::Modifiers::none());
        cx.simulate_click(circle_only, gpui::Modifiers::none());
        cx.settle();
        assert_eq!(
            cx.update(|_, app| editor.read(app).selected_annotations.clone()),
            ["lower-circle"]
        );

        let final_state = cx.update(|_, app| {
            let editor = editor.read(app);
            (
                editor.history.current().to_vec(),
                editor.export_png().unwrap(),
            )
        });
        assert_eq!(final_state.0, initial.0);
        assert_eq!(final_state.1, initial.1);
    }

    fn point(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn resize_state(handle: ResizeHandle) -> ResizeState {
        ResizeState {
            handle,
            start: point(0.0, 0.0),
            last: point(0.0, 0.0),
            original: None,
            anchor: None,
            initial_drag: None,
            initial_font_size: None,
            initial_size: None,
            center: None,
            initial_rotation: None,
            start_angle: None,
        }
    }

    #[test]
    fn corner_resize_keeps_the_anchor_and_signed_extent() {
        let mut rect = Annotation::Rectangle {
            id: "rect-1".into(),
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 50.0,
            stroke: "#000".into(),
            stroke_width: 2.0,
            fill: None,
        };
        let mut state = resize_state(ResizeHandle::BottomRight);
        state.anchor = Some((10.0, 10.0));
        state.initial_drag = Some((110.0, 60.0));
        apply_resize(
            &mut rect,
            &state,
            20.0,
            10.0,
            20.0,
            10.0,
            point(130.0, 70.0),
        );
        assert!(matches!(
            rect,
            Annotation::Rectangle { x, y, width, height, .. }
            if x == 10.0 && y == 10.0 && width == 120.0 && height == 60.0
        ));

        let mut state = resize_state(ResizeHandle::TopLeft);
        state.anchor = Some((110.0, 60.0));
        state.initial_drag = Some((10.0, 10.0));
        apply_resize(
            &mut rect,
            &state,
            150.0,
            0.0,
            150.0,
            0.0,
            point(160.0, 10.0),
        );
        assert!(matches!(
            rect,
            Annotation::Rectangle { x, width, .. } if x == 110.0 && width == 50.0
        ));
    }

    #[test]
    fn circle_resize_grows_the_radius_with_a_floor() {
        let mut circle = Annotation::Circle {
            id: "circle-1".into(),
            x: 50.0,
            y: 50.0,
            radius: 20.0,
            stroke: "#000".into(),
            stroke_width: 2.0,
            fill: None,
        };
        let state = resize_state(ResizeHandle::BottomRight);
        apply_resize(&mut circle, &state, 6.0, 4.0, 6.0, 4.0, point(0.0, 0.0));
        assert!(matches!(circle, Annotation::Circle { radius, .. } if radius == 25.0));
        apply_resize(
            &mut circle,
            &state,
            -100.0,
            -100.0,
            -100.0,
            -100.0,
            point(0.0, 0.0),
        );
        assert!(matches!(circle, Annotation::Circle { radius, .. } if radius == 5.0));
    }

    #[test]
    fn segment_endpoints_and_arrow_bend_follow_the_pointer() {
        let mut line = Annotation::Line {
            id: "line-1".into(),
            points: [0.0, 0.0, 10.0, 10.0],
            stroke: "#000".into(),
            stroke_width: 2.0,
        };
        let state = resize_state(ResizeHandle::End);
        apply_resize(&mut line, &state, 5.0, -5.0, 5.0, -5.0, point(0.0, 0.0));
        assert!(matches!(
            line,
            Annotation::Line { points, .. } if points == [0.0, 0.0, 15.0, 5.0]
        ));

        let mut arrow = Annotation::Arrow {
            id: "arrow-1".into(),
            points: [0.0, 0.0, 10.0, 0.0],
            stroke: "#000".into(),
            stroke_width: 2.0,
            arrow_style: None,
            bend_offset: None,
        };
        let state = resize_state(ResizeHandle::Bend);
        apply_resize(&mut arrow, &state, 3.0, 4.0, 3.0, 4.0, point(0.0, 0.0));
        assert!(matches!(
            arrow,
            Annotation::Arrow { bend_offset: Some(offset), .. }
            if offset.x == 3.0 && offset.y == 4.0
        ));
    }

    #[test]
    fn text_resize_scales_the_font_and_rotate_sets_the_angle() {
        let mut text = Annotation::Text {
            id: "text-1".into(),
            x: 0.0,
            y: 0.0,
            text: "hi".into(),
            font_size: 16.0,
            fill: "#000".into(),
            font_family: None,
            background_color: None,
            background_opacity: None,
            background_padding: None,
            background_radius: None,
            rotation: None,
        };
        let mut state = resize_state(ResizeHandle::BottomRight);
        state.initial_font_size = Some(16.0);
        state.initial_size = Some((100.0, 50.0));
        apply_resize(&mut text, &state, 50.0, 0.0, 50.0, 0.0, point(0.0, 0.0));
        assert!(matches!(text, Annotation::Text { font_size, .. } if font_size == 24.0));

        let mut state = resize_state(ResizeHandle::Rotate);
        state.center = Some((50.0, 25.0));
        state.initial_rotation = Some(0.0);
        state.start_angle = Some(-std::f64::consts::FRAC_PI_2);
        apply_resize(&mut text, &state, 0.0, 0.0, 0.0, 0.0, point(100.0, 25.0));
        assert!(matches!(
            text,
            Annotation::Text { rotation: Some(angle), .. } if (angle - 90.0).abs() < 1e-9
        ));
    }

    #[test]
    fn shape_builders_keep_the_selected_fill() {
        let filled = Some("#ff0000".to_string());
        let rectangle = build_shape(
            Tool::Rectangle,
            "rect".into(),
            point(0.0, 0.0),
            point(10.0, 20.0),
            "#ff0000",
            4.0,
            filled.clone(),
        );
        assert!(matches!(rectangle, Annotation::Rectangle { fill, .. } if fill == filled));
        let circle = build_shape(
            Tool::Circle,
            "circle".into(),
            point(0.0, 0.0),
            point(10.0, 0.0),
            "#ff0000",
            4.0,
            filled.clone(),
        );
        assert!(matches!(circle, Annotation::Circle { fill, .. } if fill == filled));
    }

    #[test]
    fn shape_builders_stay_unfilled_in_outline_mode() {
        let rectangle = build_shape(
            Tool::Rectangle,
            "rect".into(),
            point(0.0, 0.0),
            point(10.0, 20.0),
            "#ff0000",
            4.0,
            None,
        );
        assert!(matches!(
            rectangle,
            Annotation::Rectangle { fill: None, .. }
        ));
    }

    #[test]
    fn segment_builder_keeps_the_selected_arrow_style() {
        let arrow = build_segment(
            Tool::Arrow,
            "arrow".into(),
            point(0.0, 0.0),
            point(10.0, 10.0),
            "#00ff00",
            4.0,
            Some("double-curved".to_string()),
        );
        assert!(
            matches!(arrow, Annotation::Arrow { arrow_style, bend_offset: None, .. } if arrow_style.as_deref() == Some("double-curved"))
        );
    }

    #[test]
    fn crop_rect_normalizes_backwards_drags() {
        assert_eq!(
            normalize_crop_rect((120.0, 100.0, -40.0, -30.0)),
            (80.0, 70.0, 40.0, 30.0)
        );
        assert_eq!(
            normalize_crop_rect((10.0, 20.0, 30.0, 40.0)),
            (10.0, 20.0, 30.0, 40.0)
        );
    }

    #[test]
    fn crop_handles_hit_their_corners() {
        let rect = (10.0, 20.0, 100.0, 60.0);
        assert_eq!(
            crop_handle_at(rect, point(10.0, 20.0), 8.0),
            Some(CropCorner::TopLeft)
        );
        assert_eq!(
            crop_handle_at(rect, point(110.0, 80.0), 8.0),
            Some(CropCorner::BottomRight)
        );
        assert_eq!(crop_handle_at(rect, point(60.0, 50.0), 8.0), None);
        assert!(point_in_crop_rect(rect, point(60.0, 50.0)));
        assert!(!point_in_crop_rect(rect, point(200.0, 50.0)));
    }

    #[test]
    fn crop_resize_clamps_to_min_size_and_image_bounds() {
        let rect = resize_crop_rect(
            (10.0, 10.0, 100.0, 100.0),
            CropCorner::BottomRight,
            1000.0,
            1000.0,
            200.0,
            150.0,
        );
        assert_eq!(rect, (10.0, 10.0, 190.0, 140.0));

        let rect = resize_crop_rect(
            (10.0, 10.0, 100.0, 100.0),
            CropCorner::TopLeft,
            95.0,
            95.0,
            200.0,
            200.0,
        );
        assert_eq!(rect, (90.0, 90.0, 20.0, 20.0));
    }

    #[herogpui::test]
    fn crop_drag_creates_moves_and_resizes_via_pointer(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("crop-drag.png");
        image::RgbaImage::from_pixel(300, 200, image::Rgba([60, 80, 100, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, cx) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor.set_tool(Tool::Crop, cx);
            editor.set_zoom(1.0, cx);
            editor
        });
        cx.settle();

        let (origin, zoom) = cx.update(|_, cx| {
            let state = editor.read(cx);
            (
                state
                    .bounds
                    .borrow()
                    .expect("canvas bounds recorded")
                    .origin,
                state.zoom,
            )
        });
        let at = |x: f32, y: f32| gpui::point(origin.x + px(x * zoom), origin.y + px(y * zoom));

        cx.simulate_mouse_down(
            at(20.0, 20.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            at(120.0, 100.0),
            Some(gpui::MouseButton::Left),
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at(120.0, 100.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.settle();
        let created = cx.update(|_, cx| editor.read(cx).crop);
        assert_eq!(created, Some((20.0, 20.0, 100.0, 80.0)));

        cx.simulate_mouse_down(
            at(70.0, 60.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            at(100.0, 85.0),
            Some(gpui::MouseButton::Left),
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at(100.0, 85.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.settle();
        let moved = cx.update(|_, cx| editor.read(cx).crop);
        assert_eq!(moved, Some((50.0, 45.0, 100.0, 80.0)));

        cx.simulate_mouse_down(
            at(150.0, 125.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            at(200.0, 155.0),
            Some(gpui::MouseButton::Left),
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at(200.0, 155.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.settle();
        let resized = cx.update(|_, cx| editor.read(cx).crop);
        assert_eq!(resized, Some((50.0, 45.0, 150.0, 110.0)));

        cx.simulate_mouse_down(
            at(5.0, 5.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at(5.0, 5.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.settle();
        let dismissed = cx.update(|_, cx| editor.read(cx).crop);
        assert_eq!(dismissed, None);

        cx.simulate_mouse_down(
            at(30.0, 30.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            at(110.0, 90.0),
            Some(gpui::MouseButton::Left),
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            at(110.0, 90.0),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.settle();
        assert!(cx.update(|_, cx| editor.read(cx).crop.is_some()));
        cx.update(|_, cx| editor.update(cx, |e, cx| e.set_tool(Tool::Pen, cx)));
        assert_eq!(cx.update(|_, cx| editor.read(cx).crop), None);
    }

    #[test]
    fn dropped_images_attach_to_the_nearest_edge() {
        use crate::editor::layers::Edge;

        let bounds = gpui::Bounds {
            origin: gpui::point(px(0.0), px(0.0)),
            size: gpui::size(px(400.0), px(400.0)),
        };
        assert_eq!(
            drop_edge_for_point(gpui::point(px(200.0), px(10.0)), bounds),
            Edge::Top
        );
        assert_eq!(
            drop_edge_for_point(gpui::point(px(200.0), px(390.0)), bounds),
            Edge::Bottom
        );
        assert_eq!(
            drop_edge_for_point(gpui::point(px(10.0), px(200.0)), bounds),
            Edge::Left
        );
        assert_eq!(
            drop_edge_for_point(gpui::point(px(390.0), px(200.0)), bounds),
            Edge::Right
        );
    }

    #[test]
    fn only_decodable_images_light_up_the_drop_zone() {
        assert!(is_image_path(std::path::Path::new("shot.PNG")));
        assert!(is_image_path(std::path::Path::new("shot.jpeg")));
        assert!(is_image_path(std::path::Path::new("shot.gif")));
        assert!(!is_image_path(std::path::Path::new("shot.webp")));
        assert!(!is_image_path(std::path::Path::new("notes.txt")));
        assert!(!is_image_path(std::path::Path::new("no-extension")));
    }

    fn open_text_test_editor(
        cx: &mut gpui::TestAppContext,
    ) -> (
        tempfile::TempDir,
        gpui::Entity<EditorWindow>,
        &mut gpui::VisualTestContext,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        image::RgbaImage::from_pixel(400, 300, image::Rgba([40, 80, 140, 255]))
            .save(&source)
            .unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(directory.path().join("config.json"))
                .unwrap(),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let source_path = source.to_string_lossy().into_owned();
        let (editor, visual) = herogpui::test::open_window(cx, |window, cx| {
            let mut editor = EditorWindow::from_file(&source_path, window, cx);
            editor.file_path.clear();
            editor.tool = Tool::Text;
            editor.text_background = false;
            editor.text_font_size = 24.0;
            editor.color_hex = "#ff3300".into();
            editor.sync_snapshot(cx);
            editor
        });
        visual.settle();
        (directory, editor, visual)
    }

    fn canvas_screen_point(
        editor: &gpui::Entity<EditorWindow>,
        visual: &mut gpui::VisualTestContext,
        x: f32,
        y: f32,
    ) -> gpui::Point<Pixels> {
        visual.update(|_, app| {
            let editor = editor.read(app);
            let bounds = editor.bounds.borrow().expect("canvas bounds");
            gpui::point(
                bounds.left() + px(x * editor.zoom),
                bounds.top() + px(y * editor.zoom),
            )
        })
    }

    #[herogpui::test]
    fn text_tool_click_stays_open_after_mouse_up_grows_on_typing_and_commits_on_enter(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_dir, editor, visual) = open_text_test_editor(cx);

        let click = canvas_screen_point(&editor, visual, 60.0, 50.0);
        visual.simulate_mouse_move(click, None, gpui::Modifiers::none());
        visual.simulate_click(click, gpui::Modifiers::none());
        visual.settle();

        let empty_bounds = visual
            .debug_bounds("inline-text-editor")
            .expect("inline text editor must remain open after mouse up");
        assert!(visual.update(|_, app| {
            let ed = editor.read(app);
            ed.text_editor.is_some() && ed.draft.is_some()
        }));

        visual.simulate_keystrokes("h e l l o space w o r l d");
        visual.settle();

        let typed_bounds = visual
            .debug_bounds("inline-text-editor")
            .expect("inline text editor stays mounted while typing");
        assert!(typed_bounds.size.width > empty_bounds.size.width);

        visual.simulate_keystrokes("enter");
        visual.settle();

        assert!(visual.debug_bounds("inline-text-editor").is_none());
        visual.update(|_, app| {
            let ed = editor.read(app);
            assert!(ed.text_editor.is_none());
            assert!(ed.draft.is_none());
            let annotations = ed.history.current();
            assert_eq!(annotations.len(), 1);
            match &annotations[0] {
                Annotation::Text {
                    text,
                    font_size,
                    fill,
                    background_color,
                    ..
                } => {
                    assert_eq!(text, "hello world");
                    assert_eq!(*font_size, 24.0);
                    assert_eq!(fill, "#ff3300");
                    assert!(background_color.is_none());
                }
                other => panic!("expected text annotation, got {other:?}"),
            }
        });
    }

    #[herogpui::test]
    fn clicking_inside_keeps_editing_and_clicking_outside_commits_without_spawning_new_box(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_dir, editor, visual) = open_text_test_editor(cx);

        let start = canvas_screen_point(&editor, visual, 60.0, 50.0);
        visual.simulate_click(start, gpui::Modifiers::none());
        visual.settle();

        visual.simulate_keystrokes("a b c");
        visual.settle();

        let box_bounds = visual
            .debug_bounds("inline-text-editor")
            .expect("inline text editor bounds");
        let inside = box_bounds.center();
        visual.simulate_click(inside, gpui::Modifiers::none());
        visual.settle();
        assert!(visual.debug_bounds("inline-text-editor").is_some());
        assert!(visual.update(|_, app| editor.read(app).text_editor.is_some()));

        let outside = canvas_screen_point(&editor, visual, 300.0, 220.0);
        visual.simulate_click(outside, gpui::Modifiers::none());
        visual.settle();

        assert!(visual.debug_bounds("inline-text-editor").is_none());
        visual.update(|_, app| {
            let ed = editor.read(app);
            assert!(ed.text_editor.is_none());
            assert_eq!(ed.history.current().len(), 1);
            assert!(matches!(
                &ed.history.current()[0],
                Annotation::Text { text, .. } if text == "abc"
            ));
        });
    }

    #[herogpui::test]
    fn live_text_options_update_active_draft_and_scale_box_height(cx: &mut gpui::TestAppContext) {
        let (_dir, editor, visual) = open_text_test_editor(cx);

        let click = canvas_screen_point(&editor, visual, 40.0, 40.0);
        visual.simulate_click(click, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_keystrokes("b i g");
        visual.settle();

        let small_bounds = visual
            .debug_bounds("inline-text-editor")
            .expect("initial inline text bounds");

        visual.update(|_, app| {
            editor.update(app, |ed, cx| {
                ed.apply_option(EditorOption::TextFontSize(72.0), cx);
                ed.apply_option(EditorOption::Color("#00ff88".into()), cx);
                ed.apply_option(EditorOption::TextFontFamily("mono".into()), cx);
                ed.apply_option(EditorOption::TextBackground(true), cx);
                ed.sync_snapshot(cx);
                cx.notify();
            });
        });
        visual.settle();

        let big_bounds = visual
            .debug_bounds("inline-text-editor")
            .expect("scaled inline text bounds");
        assert!(big_bounds.size.height > small_bounds.size.height * 2.0);
        assert!(big_bounds.size.width > small_bounds.size.width * 2.0);

        visual.update(|_, app| {
            editor.update(app, |ed, cx| ed.commit_text(cx));
        });
        visual.settle();

        visual.update(|_, app| {
            let ed = editor.read(app);
            assert_eq!(ed.history.current().len(), 1);
            match &ed.history.current()[0] {
                Annotation::Text {
                    text,
                    font_size,
                    fill,
                    font_family,
                    background_color,
                    ..
                } => {
                    assert_eq!(text, "big");
                    assert_eq!(*font_size, 72.0);
                    assert_eq!(fill, "#00ff88");
                    assert_eq!(font_family.as_deref(), Some("mono"));
                    assert!(background_color.is_some());
                }
                other => panic!("expected text annotation, got {other:?}"),
            }
        });
    }

    #[herogpui::test]
    fn reediting_existing_text_does_not_duplicate_and_escape_restores_original(
        cx: &mut gpui::TestAppContext,
    ) {
        let (_dir, editor, visual) = open_text_test_editor(cx);

        let click = canvas_screen_point(&editor, visual, 80.0, 80.0);
        visual.simulate_click(click, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_keystrokes("o n e");
        visual.settle();
        visual.simulate_keystrokes("enter");
        visual.settle();

        let hit = canvas_screen_point(&editor, visual, 90.0, 90.0);
        visual.simulate_click(hit, gpui::Modifiers::none());
        visual.settle();
        assert!(visual.debug_bounds("inline-text-editor").is_some());
        assert_eq!(
            visual.update(|_, app| editor.read(app).history.current().len()),
            1
        );

        visual.simulate_keystrokes("t w o");
        visual.settle();
        visual.simulate_keystrokes("escape");
        visual.settle();

        assert!(visual.debug_bounds("inline-text-editor").is_none());
        visual.update(|_, app| {
            let ed = editor.read(app);
            assert_eq!(ed.history.current().len(), 1);
            assert!(matches!(
                &ed.history.current()[0],
                Annotation::Text { text, .. } if text == "one"
            ));
        });

        visual.simulate_click(hit, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_keystrokes("enter");
        visual.settle();
        visual.update(|window, app| {
            let ed = editor.read(app);
            assert!(ed.focus_handle.is_focused(window));
            assert_eq!(ed.history.current().len(), 1);
            assert!(ed.history.can_undo());
        });
        visual.update(|_, app| {
            editor.update(app, |ed, cx| ed.undo(cx));
        });
        visual.settle();
        assert_eq!(
            visual.update(|_, app| editor.read(app).history.current().len()),
            0
        );
        visual.update(|_, app| {
            editor.update(app, |ed, cx| ed.redo(cx));
        });
        visual.settle();

        visual.simulate_click(hit, gpui::Modifiers::none());
        visual.settle();
        visual.simulate_keystrokes("space t w o");
        visual.settle();
        visual.update(|_, app| {
            editor.update(app, |ed, cx| ed.set_tool(Tool::Select, cx));
        });
        visual.settle();

        visual.update(|_, app| {
            let ed = editor.read(app);
            assert!(ed.text_editor.is_none());
            assert_eq!(ed.history.current().len(), 1);
            assert!(matches!(
                &ed.history.current()[0],
                Annotation::Text { text, .. } if text == "one two"
            ));
        });
    }
}
