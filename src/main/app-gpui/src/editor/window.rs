//! Editor window entity — owns editor state, wires the title bar and canvas
//! together, and hosts the color palette popover.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, prelude::*, px, App, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Render, ScrollWheelEvent, Styled, Subscription, Window,
};

use crate::editor::actions;
use crate::editor::annotations::{self, Annotation, AnnotationHistory, Point};
use crate::editor::canvas::{CanvasSnapshot, EditorCanvas};
use crate::editor::options::{
    EditorAction, EditorHandlers, EditorOption, MAX_ZOOM, MIN_ZOOM, ZOOM_STEP,
};
use crate::editor::title_bar::TitleBar;
use crate::editor::tool_options::ToolOptionsState;
use crate::theme::vars::active_theme;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::colors::Tool;
use crate::ui::menu::MenuHandle;

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedEditorState {
    #[serde(default)]
    annotations: Vec<Annotation>,
    #[serde(default)]
    wallpaper: crate::editor::wallpaper::WallpaperSettings,
    #[serde(default)]
    layers: Vec<crate::editor::layers::ImageLayer>,
}

pub struct EditorWindow {
    pub image: Option<Arc<gpui::RenderImage>>,
    pub base_image: Option<Arc<image::DynamicImage>>,
    pub is_copied: bool,
    pub image_width: f32,
    pub image_height: f32,
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
    pointer_update_scheduled: bool,
    pub menu: MenuHandle,
    /// The inline field shown while a text annotation is being typed.
    text_editor: Option<(String, gpui::Entity<crate::ui::text_field::TextField>)>,
    /// The pending crop rectangle in image coordinates.
    crop: Option<(f64, f64, f64, f64)>,
    /// The zoom control's measured rect, so its `backdrop-blur-md` can sample
    /// the capture underneath it. Measured rather than computed because the
    /// bar's width follows its label.
    pub(crate) zoom_bar_bounds: std::rc::Rc<std::cell::RefCell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// The blurred crop behind the zoom control, rebuilt only when the sampled
    /// region moves.
    zoom_backdrop: Option<crate::editor::zoom_backdrop::ZoomBackdrop>,
    /// The viewport and sheet state the current fit-to-window zoom was computed
    /// for, so it is recomputed on a resize or a sheet toggle and not otherwise.
    fit_for: Option<(gpui::Size<Pixels>, bool)>,
    pub wallpaper: crate::editor::wallpaper::WallpaperSettings,
    wallpaper_preset_id: String,
    pub cloud_upload: crate::cloud::UploadState,
    redact_patches: std::collections::HashMap<String, Arc<gpui::RenderImage>>,
    pub snapshot: SnapshotCell,
    pub bounds: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
    /// Images attached to the capture's edges.
    layers: Vec<crate::editor::layers::ImageLayer>,
    /// Whether the edge overlay for attaching a capture is showing.
    capture_mode: bool,
    /// The margin the balance option trims, recomputed when it is turned on.
    balance_crop: Option<(f32, f32, f32, f32)>,
    /// The annotation the select tool has picked, and the point a move drag
    /// started from. One history entry is pushed when the drag ends.
    selected_annotation: Option<String>,
    move_origin: Option<Point>,
    /// Annotations on the editor's own clipboard, from copy or cut.
    annotation_clipboard: Vec<Annotation>,
    /// The rasterized wallpaper backdrop and the settings it was made for,
    /// so it is only re-rendered when the wallpaper actually changes.
    backdrop: Option<Arc<gpui::RenderImage>>,
    backdrop_key: Option<(
        crate::editor::wallpaper::WallpaperSettings,
        u32,
        u32,
        Vec<crate::editor::layers::ImageLayer>,
    )>,
    backdrop_generation: u64,
    backdrop_task: Option<gpui::Task<()>>,
    inset_color: Option<String>,
    /// The stage's scroll position, which ctrl-drag pans.
    stage_scroll: gpui::ScrollHandle,
    /// Pointer position and scroll offset a pan started from.
    pan_origin: Option<(gpui::Point<Pixels>, gpui::Point<Pixels>)>,
    next_id: u32,
    persisted_editor_state: RefCell<Option<serde_json::Value>>,
    last_editor_state_write: Cell<std::time::Instant>,
    pub focus_handle: gpui::FocusHandle,
    bounds_sub: Option<Subscription>,
}

pub type SnapshotCell = Rc<RefCell<CanvasSnapshot>>;

impl EditorWindow {
    pub fn from_file(path: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (image, base, width, height) = match load_image(path) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("[editor] failed to load {path}: {error}");
                (None, None, 0.0, 0.0)
            }
        };

        let persisted = crate::history_store::editor_state_for_path(std::path::Path::new(path))
            .and_then(|state| serde_json::from_value::<PersistedEditorState>(state).ok())
            .unwrap_or_default();
        let persisted_value = serde_json::to_value(&persisted).ok();
        let next_id = persisted
            .annotations
            .iter()
            .filter_map(|annotation| annotation.id().strip_prefix("ann-"))
            .filter_map(|id| id.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let mut editor = Self {
            image,
            base_image: base.map(Arc::new),
            layers: persisted.layers,
            capture_mode: false,
            balance_crop: None,
            selected_annotation: None,
            move_origin: None,
            annotation_clipboard: Vec::new(),
            backdrop: None,
            backdrop_key: None,
            backdrop_generation: 0,
            backdrop_task: None,
            inset_color: None,
            stage_scroll: gpui::ScrollHandle::new(),
            pan_origin: None,
            is_copied: false,
            image_width: width,
            image_height: height,
            file_path: path.to_string(),
            tool: Tool::Select,
            color_hex: "#FF3B30".to_string(),
            stroke_width: 3.0,
            arrow_style: "standard".to_string(),
            highlight_color: "#FFFF00".to_string(),
            highlight_opacity: 0.4,
            number_style: "numeric".to_string(),
            number_size: "medium".to_string(),
            number_start_value: 1.0,
            text_background: true,
            text_font_size: 20.0,
            text_font_family: "sans".to_string(),
            redact_style: "pixelate".to_string(),
            redact_intensity: 5.0,
            shape_fill_mode: "outline".to_string(),
            zoom: 1.0,
            history: AnnotationHistory::new(persisted.annotations),
            draft: None,
            drag_start: None,
            pending_pointer_update: None,
            pending_stroke_points: Vec::new(),
            pointer_update_scheduled: false,
            menu: MenuHandle::new(),
            text_editor: None,
            crop: None,
            zoom_bar_bounds: Rc::new(RefCell::new(None)),
            fit_for: None,
            zoom_backdrop: None,
            wallpaper: persisted.wallpaper,
            wallpaper_preset_id: String::new(),
            cloud_upload: crate::cloud::UploadState::Idle,
            redact_patches: std::collections::HashMap::new(),
            snapshot: Rc::new(RefCell::new(CanvasSnapshot {
                image: None,
                redact_patches: std::collections::HashMap::new(),
                image_width: width,
                image_height: height,
                zoom: 1.0,
                annotations: Vec::new(),
                draft: None,
                tool: Tool::Select,
                color_hex: "#FF3B30".to_string(),
                stroke_width: 3.0,
                crop: None,
                wallpaper: crate::editor::wallpaper::WallpaperSettings::default(),
                backdrop: None,
                selected: None,
                balance_crop: None,
                layers: Vec::new(),
                spacing: 0.0,
            })),
            bounds: Rc::new(RefCell::new(None)),
            next_id,
            persisted_editor_state: RefCell::new(persisted_value),
            last_editor_state_write: Cell::new(std::time::Instant::now()),
            focus_handle: cx.focus_handle(),
            bounds_sub: None,
        };
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
        editor.sync_snapshot();
        editor
    }

    /// Rasterizes the redacted pixels for every committed redaction once, so
    /// the canvas can show the exported result instead of a placeholder.
    fn refresh_redact_patches(&mut self) {
        let Some(base) = self.base_image.as_ref() else {
            return;
        };
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
            let key = format!("{id}:{x}:{y}:{width}:{height}:{style}:{intensity}");
            if let Some(existing) = self.redact_patches.get(&key) {
                live.insert(key, existing.clone());
                continue;
            }
            if let Some(patch) =
                render_redact_patch(base, *x, *y, *width, *height, style, *intensity)
            {
                live.insert(key, patch);
            }
        }
        self.redact_patches = live;
    }

    fn redact_patches_by_id(&self) -> std::collections::HashMap<String, Arc<gpui::RenderImage>> {
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
                let key = format!("{id}:{x}:{y}:{width}:{height}:{style}:{intensity}");
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
            self.balance_crop = None;
            return;
        }
        if self.balance_crop.is_some() {
            return;
        }
        let Some(base) = self.base_image.as_ref() else {
            return;
        };
        self.balance_crop = crate::editor::export::balance_bounds(&base.to_rgba8()).map(|bounds| {
            (
                bounds.left as f32,
                bounds.top as f32,
                bounds.right as f32,
                bounds.bottom as f32,
            )
        });
    }

    /// Renders the wallpaper backdrop when the settings or the image size have
    /// changed since the last one.
    fn refresh_backdrop(&mut self, cx: &mut Context<Self>) {
        if !self
            .wallpaper
            .is_active_with_layers(!self.layers.is_empty())
        {
            if self.backdrop.is_some()
                || self.backdrop_key.is_some()
                || self.backdrop_task.is_some()
            {
                self.backdrop_generation = self.backdrop_generation.wrapping_add(1);
            }
            self.backdrop = None;
            self.backdrop_key = None;
            self.backdrop_task = None;
            return;
        }
        // The frame surrounds the whole group, so the layout is computed from
        // the attached layers' bounds rather than the capture alone.
        let group = crate::editor::layers::compute(
            self.image_width as f64,
            self.image_height as f64,
            &self.layers,
            self.wallpaper.spacing.max(0.0),
        );
        let ((width, height), _) =
            crate::editor::wallpaper::layout(&self.wallpaper, group.width, group.height);
        let size = (
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
        );
        let key = (self.wallpaper.clone(), size.0, size.1, self.layers.clone());
        if self.backdrop_key.as_ref() == Some(&key) {
            return;
        }
        self.backdrop_key = Some(key);
        self.backdrop_generation = self.backdrop_generation.wrapping_add(1);
        let generation = self.backdrop_generation;
        let wallpaper = self.wallpaper.clone();
        let image_width = self.image_width as f64;
        let image_height = self.image_height as f64;
        let layers = self.layers.clone();
        let inset_color = self.inset_color.clone();
        let base_image = self.base_image.clone();
        let background = cx.background_executor().clone();
        let task = cx.background_executor().spawn(async move {
            background.timer(std::time::Duration::from_millis(16)).await;
            let inset_color = if wallpaper.inset > 0.0 && inset_color.is_none() {
                base_image.as_ref().and_then(|base| {
                    let base = base.to_rgba8();
                    let pixmap = crate::editor::export::from_rgba(&base)?;
                    crate::render::color_detection::dominant_inset_color(
                        pixmap.as_ref(),
                        crate::render::color_detection::ContentBounds::default(),
                    )
                })
            } else {
                inset_color
            };
            let rendered = crate::editor::export::render_backdrop(
                &wallpaper,
                size.0,
                size.1,
                image_width,
                image_height,
                inset_color.clone(),
                &layers,
            );
            (rendered, inset_color)
        });
        self.backdrop_task = Some(cx.spawn(async move |entity, cx| {
            let (rendered, inset_color) = task.await;
            let _ = entity.update(cx, |editor, cx| {
                if editor.backdrop_generation != generation {
                    return;
                }
                editor.backdrop = rendered;
                editor.inset_color = inset_color;
                editor.backdrop_task = None;
                cx.notify();
            });
        }));
    }

    fn sync_snapshot(&self) {
        {
            let mut snap = self.snapshot.borrow_mut();
            snap.redact_patches = self.redact_patches_by_id();
            snap.crop = self.crop;
            snap.wallpaper = self.wallpaper.clone();
            snap.backdrop = self.backdrop.clone();
            snap.selected = self.selected_annotation.clone();
            snap.balance_crop = self.balance_crop;
            snap.layers = self.layers.clone();
            snap.spacing = self.wallpaper.spacing;
            snap.image = self.image.clone();
            snap.image_width = self.image_width;
            snap.image_height = self.image_height;
            snap.zoom = self.zoom;
            snap.annotations = self.history.current().to_vec();
            snap.draft = self.draft.clone();
            snap.tool = self.tool;
            snap.color_hex = self.color_hex.clone();
            snap.stroke_width = self.stroke_width;
        }
        self.persist_editor_state(false);
    }

    fn persist_editor_state(&self, force: bool) {
        if self.file_path.is_empty() || self.draft.is_some() {
            return;
        }
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
        let state = match serde_json::to_value(PersistedEditorState {
            annotations: self.history.current().to_vec(),
            wallpaper,
            layers: self.layers.clone(),
        }) {
            Ok(state) => state,
            Err(_) => return,
        };
        if self.persisted_editor_state.borrow().as_ref() == Some(&state) {
            return;
        }
        if !force
            && self.last_editor_state_write.get().elapsed() < std::time::Duration::from_millis(500)
        {
            return;
        }
        if crate::history_store::update_editor_state_by_path(
            std::path::Path::new(&self.file_path),
            state.clone(),
        ) {
            *self.persisted_editor_state.borrow_mut() = Some(state);
            self.last_editor_state_write.set(std::time::Instant::now());
        }
    }

    fn annotation_id(&mut self) -> String {
        self.next_id += 1;
        format!("ann-{}", self.next_id)
    }

    /// The topmost annotation under `point`, which is the last one drawn.
    fn annotation_at(&self, point: Point) -> Option<String> {
        const TOLERANCE: f64 = 4.0;
        self.history
            .current()
            .iter()
            .rev()
            .find(|annotation| {
                let (left, top, right, bottom) = annotation.bounds();
                point.x as f64 >= left - TOLERANCE
                    && point.x as f64 <= right + TOLERANCE
                    && point.y as f64 >= top - TOLERANCE
                    && point.y as f64 <= bottom + TOLERANCE
            })
            .map(|annotation| annotation.id().to_string())
    }

    pub fn toggle_capture_mode(&mut self, cx: &mut Context<Self>) {
        self.capture_mode = !self.capture_mode;
        cx.notify();
    }

    /// Attaches an image to one of the capture's edges — the picker
    /// counterpart of the renderer's capture-and-attach.
    pub fn attach_layer(&mut self, edge: crate::editor::layers::Edge, cx: &mut Context<Self>) {
        self.capture_mode = false;
        let Some(path) = crate::editor::background::pick_image() else {
            cx.notify();
            return;
        };
        let Some(size) = image::image_dimensions(&path).ok() else {
            crate::windows::toast::Toast::show(
                cx,
                "Could not attach",
                "That file could not be read as an image.",
            );
            return;
        };
        self.layers.push(crate::editor::layers::ImageLayer {
            id: format!("layer-{}", self.layers.len() + 1),
            image_url: path,
            natural_width: size.0 as f64,
            natural_height: size.1 as f64,
            edge,
        });
        self.refresh_backdrop(cx);
        self.sync_snapshot();
        cx.notify();
    }

    pub fn clear_layers(&mut self, cx: &mut Context<Self>) {
        if self.layers.is_empty() {
            return;
        }
        self.layers.clear();
        self.refresh_backdrop(cx);
        self.sync_snapshot();
        cx.notify();
    }

    /// Moves the selected annotation without touching history; the gesture is
    /// committed when the pointer is released.
    fn move_selected(&mut self, point: Point) {
        let (Some(id), Some(origin)) = (self.selected_annotation.clone(), self.move_origin) else {
            return;
        };
        let (dx, dy) = ((point.x - origin.x) as f64, (point.y - origin.y) as f64);
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let mut annotations = self.history.current().to_vec();
        if let Some(annotation) = annotations
            .iter_mut()
            .find(|annotation| annotation.id() == id)
        {
            annotation.translate(dx, dy);
        }
        // The move replaces the current revision rather than stacking one entry
        // per pointer sample; `finish_stroke` records the final position.
        self.history.replace_current(annotations);
        self.move_origin = Some(point);
    }

    pub fn delete_selected_annotation(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_annotation.take() else {
            return;
        };
        let annotations: Vec<Annotation> = self
            .history
            .current()
            .iter()
            .filter(|annotation| annotation.id() != id)
            .cloned()
            .collect();
        self.history.push(annotations);
        self.refresh_redact_patches();
        self.sync_snapshot();
        cx.notify();
    }

    /// `handleCopyAnnotations` — returns false when nothing was selected, so
    /// the caller can fall back to copying the image.
    pub fn copy_selected_annotation(&mut self) -> bool {
        let Some(id) = self.selected_annotation.clone() else {
            return false;
        };
        let Some(annotation) = self
            .history
            .current()
            .iter()
            .find(|annotation| annotation.id() == id)
        else {
            return false;
        };
        self.annotation_clipboard = vec![annotation.clone()];
        true
    }

    pub fn cut_selected_annotation(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.copy_selected_annotation() {
            return false;
        }
        self.delete_selected_annotation(cx);
        true
    }

    /// `handlePasteAnnotations` — a pasted copy is offset so it does not hide
    /// the original.
    pub fn paste_annotations(&mut self, cx: &mut Context<Self>) {
        if self.annotation_clipboard.is_empty() {
            return;
        }
        const PASTE_OFFSET: f64 = 12.0;
        let mut annotations = self.history.current().to_vec();
        let mut last_id = None;
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
            last_id = Some(id);
            annotations.push(copy);
        }
        self.history.push(annotations);
        self.selected_annotation = last_id;
        self.refresh_redact_patches();
        self.sync_snapshot();
        cx.notify();
    }

    fn start_stroke(&mut self, point: Point, window: &mut Window, cx: &mut Context<Self>) {
        let tool = self.tool;
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
                self.history.push(annotations);
            }
            Tool::Text => {
                self.drag_start = None;
                self.begin_text(point, color, cx);
                self.focus_text_editor(window, cx);
            }
            Tool::Crop => {
                self.drag_start = Some(point);
                self.crop = Some((
                    point.x.clamp(0.0, self.image_width) as f64,
                    point.y.clamp(0.0, self.image_height) as f64,
                    0.0,
                    0.0,
                ));
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
                self.draft = Some(build_shape(
                    tool,
                    self.annotation_id(),
                    point,
                    point,
                    &color,
                    stroke_width,
                ));
            }
            Tool::Line | Tool::Arrow => {
                self.drag_start = Some(point);
                self.draft = Some(build_segment(
                    tool,
                    self.annotation_id(),
                    point,
                    point,
                    &color,
                    stroke_width,
                ));
            }
            Tool::Select => {
                self.selected_annotation = self.annotation_at(point);
                self.move_origin = self.selected_annotation.as_ref().map(|_| point);
                if self.selected_annotation.is_some() {
                    // A move is one undo step, so the pre-move revision is kept
                    // by pushing a copy the drag then edits in place.
                    let annotations = self.history.current().to_vec();
                    self.history.push(annotations);
                }
            }
            _ => {}
        }
        self.sync_snapshot();
    }

    fn extend_stroke(&mut self, point: Point) {
        let Some(start) = self.drag_start else {
            return;
        };
        let tool = self.tool;
        let color = self.color_hex.clone();
        let stroke_width = self.stroke_width;
        if tool == Tool::Crop {
            if let Some((x, y, width, height)) = &mut self.crop {
                let clamped_x = point.x.clamp(0.0, self.image_width) as f64;
                let clamped_y = point.y.clamp(0.0, self.image_height) as f64;
                *width = clamped_x - *x;
                *height = clamped_y - *y;
            }
            self.sync_snapshot();
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
                );
            }
            _ => {}
        }
        self.sync_snapshot();
    }

    fn queue_pointer_update(
        &mut self,
        point: Point,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.draft,
            Some(Annotation::Pen { .. } | Annotation::Highlight { .. })
        ) {
            self.pending_stroke_points.push(point);
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
                editor.apply_pending_pointer_update();
                cx.notify();
            });
        });
        window.request_animation_frame();
    }

    fn apply_pending_pointer_update(&mut self) {
        self.pointer_update_scheduled = false;
        if let Some(point) = self.pending_pointer_update.take() {
            if self.move_origin.is_some() {
                self.move_selected(point);
                self.sync_snapshot();
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
        let Some(draft @ (Annotation::Pen { .. } | Annotation::Highlight { .. })) = &mut self.draft
        else {
            return;
        };
        let mut changed = false;
        for point in points {
            changed |= draft.push_point(point);
        }
        if changed {
            self.sync_snapshot();
        }
    }

    /// Port of `applyCrop`: crops the base image and shifts every annotation
    /// into the new coordinate space.
    pub fn apply_crop(&mut self, cx: &mut Context<Self>) {
        let Some((x, y, width, height)) = self.crop.take() else {
            return;
        };
        let (left, top) = (
            if width < 0.0 { x + width } else { x },
            if height < 0.0 { y + height } else { y },
        );
        let (crop_width, crop_height) = (width.abs(), height.abs());
        if crop_width < 1.0 || crop_height < 1.0 {
            self.sync_snapshot();
            cx.notify();
            return;
        }

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

            let mut bgra = cropped.clone();
            for pixel in bgra.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            self.image = Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
                image::Frame::new(bgra)
            ])));
            self.base_image = Some(Arc::new(image::DynamicImage::ImageRgba8(cropped)));
            self.inset_color = None;
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

        self.refresh_redact_patches();
        self.sync_snapshot();
        cx.notify();
    }

    pub fn cancel_crop(&mut self, cx: &mut Context<Self>) {
        self.crop = None;
        self.sync_snapshot();
        cx.notify();
    }

    fn begin_text(&mut self, point: Point, color: String, cx: &mut Context<Self>) {
        self.commit_text(cx);
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

        let owner = cx.entity().downgrade();
        let cancel_owner = owner.clone();
        let field = cx.new(|cx| {
            crate::ui::text_field::TextField::new("", cx)
                .placeholder("Type\u{2026}")
                .on_change({
                    let owner = owner.clone();
                    move |value, _window, app| {
                        let value = value.to_string();
                        if let Some(owner) = owner.upgrade() {
                            owner.update(app, |editor, cx| {
                                if let Some(Annotation::Text { text, .. }) = &mut editor.draft {
                                    *text = value.clone();
                                }
                                editor.sync_snapshot();
                                cx.notify();
                            });
                        }
                    }
                })
                .on_submit(move |_value, _window, app| {
                    if let Some(owner) = owner.upgrade() {
                        owner.update(app, |editor, cx| editor.commit_text(cx));
                    }
                })
                .on_cancel(move |_value, _window, app| {
                    if let Some(owner) = cancel_owner.upgrade() {
                        owner.update(app, |editor, cx| {
                            editor.text_editor = None;
                            editor.draft = None;
                            editor.sync_snapshot();
                            cx.notify();
                        });
                    }
                })
        });
        self.text_editor = Some((id, field));
        self.sync_snapshot();
        cx.notify();
    }

    /// Commits the in-progress text annotation, dropping it when empty.
    pub fn commit_text(&mut self, cx: &mut Context<Self>) {
        let Some((_, _)) = self.text_editor.take() else {
            return;
        };
        let Some(draft) = self.draft.take() else {
            self.sync_snapshot();
            cx.notify();
            return;
        };
        if let Annotation::Text { text, .. } = &draft {
            if !text.trim().is_empty() {
                let mut annotations = self.history.current().to_vec();
                annotations.push(draft);
                self.history.push(annotations);
            }
        }
        self.sync_snapshot();
        cx.notify();
    }

    fn text_editor_overlay(&self) -> Option<gpui::AnyElement> {
        let (_, field) = self.text_editor.as_ref()?;
        let Some(Annotation::Text {
            x, y, font_size, ..
        }) = &self.draft
        else {
            return None;
        };
        let bounds = (*self.bounds.borrow())?;
        Some(
            div()
                .absolute()
                .left(bounds.left() + px(*x as f32 * self.zoom))
                .top(bounds.top() + px(*y as f32 * self.zoom))
                .w(px((*font_size as f32 * self.zoom * 12.0).max(160.0)))
                .child(field.clone())
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

    fn finish_stroke(&mut self) {
        if self.move_origin.take().is_some() {
            self.refresh_redact_patches();
            self.sync_snapshot();
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
                self.history.push(annotations);
                self.refresh_redact_patches();
            }
        }
        self.drag_start = None;
        self.sync_snapshot();
    }
}

fn render_redact_patch(
    base: &image::DynamicImage,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    style: &str,
    intensity: f64,
) -> Option<Arc<gpui::RenderImage>> {
    let x0 = x.max(0.0).round() as u32;
    let y0 = y.max(0.0).round() as u32;
    let w = width.round().max(1.0) as u32;
    let h = height.round().max(1.0) as u32;
    if x0 >= base.width() || y0 >= base.height() {
        return None;
    }
    let w = w.min(base.width() - x0);
    let h = h.min(base.height() - y0);

    let mut patch = image::imageops::crop_imm(&base.to_rgba8(), x0, y0, w, h).to_image();
    crate::editor::export::redact_region(&mut patch, 0, 0, w as i64, h as i64, style, intensity);

    // GPUI composites in BGRA.
    for pixel in patch.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(patch)
    ])))
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
            fill: None,
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
        fill: None,
    }
}

fn build_segment(
    tool: Tool,
    id: String,
    start: Point,
    end: Point,
    color: &str,
    stroke_width: f64,
) -> Annotation {
    let stroke = color.to_string();
    let points = [start.x as f64, start.y as f64, end.x as f64, end.y as f64];
    if tool == Tool::Arrow {
        Annotation::Arrow {
            id,
            points,
            stroke,
            stroke_width,
            arrow_style: None,
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
        let group = crate::editor::layers::compute(
            self.image_width as f64,
            self.image_height as f64,
            &self.layers,
            self.wallpaper.spacing.max(0.0),
        );
        if self
            .wallpaper
            .is_active_with_layers(!self.layers.is_empty())
        {
            let ((width, height), _) =
                crate::editor::wallpaper::layout(&self.wallpaper, group.width, group.height);
            (width as f32, height as f32)
        } else {
            (group.width as f32, group.height as f32)
        }
    }

    /// `calculateOptimalZoom` runs on the first frame, on every resize, and on
    /// every wallpaper-sheet toggle -- so a manual zoom is deliberately
    /// discarded when any of those change, exactly as in Electron.
    fn fit_to_window(&mut self, window: &mut Window) {
        let sheet_open = self.tool == Tool::Wallpaper;
        let viewport = window.viewport_size();
        if self.fit_for == Some((viewport, sheet_open)) {
            return;
        }
        self.fit_for = Some((viewport, sheet_open));

        let (width, height) = self.content_size();
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.set_zoom(crate::editor::zoom_fit::optimal_zoom(
            viewport, width, height, sheet_open,
        ));
    }

    /// Rebuilds the zoom control's `backdrop-blur-md` from the previous frame's
    /// measurements, and asks for one more frame when it changed so the new crop
    /// is actually shown.
    fn refresh_zoom_backdrop(&mut self, cx: &mut Context<Self>) {
        use crate::editor::zoom_backdrop;

        let previous = self.zoom_backdrop.as_ref().map(|backdrop| backdrop.key());
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

        if self.zoom_backdrop.as_ref().map(|b| b.key()) != previous {
            cx.notify();
        }
    }
}

impl Render for EditorWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let theme = active_theme(cx);
        self.fit_to_window(window);
        self.sync_snapshot();
        self.refresh_backdrop(cx);
        self.refresh_zoom_backdrop(cx);

        let handlers = build_handlers(&weak);
        let shortcuts = crate::state::state(cx)
            .config
            .read(|config| config.shortcuts.clone());

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
            .text_color(theme.foreground)
            .track_focus(&self.focus_handle);
        let root = if self.text_editor.is_none() {
            root.key_context("Editor")
        } else {
            root
        };

        root.on_action(
            cx.listener(|this, _: &actions::ToolSelect, _, _cx| this.set_tool(Tool::Select)),
        )
        .on_action(cx.listener(|this, _: &actions::ToolPen, _, _cx| this.set_tool(Tool::Pen)))
        .on_action(
            cx.listener(|this, _: &actions::ToolHighlight, _, _cx| this.set_tool(Tool::Highlight)),
        )
        .on_action(
            cx.listener(|this, _: &actions::ToolRectangle, _, _cx| this.set_tool(Tool::Rectangle)),
        )
        .on_action(cx.listener(|this, _: &actions::ToolCircle, _, _cx| this.set_tool(Tool::Circle)))
        .on_action(cx.listener(|this, _: &actions::ToolLine, _, _cx| this.set_tool(Tool::Line)))
        .on_action(cx.listener(|this, _: &actions::ToolArrow, _, _cx| this.set_tool(Tool::Arrow)))
        .on_action(cx.listener(|this, _: &actions::ToolText, _, _cx| this.set_tool(Tool::Text)))
        .on_action(cx.listener(|this, _: &actions::ToolNumber, _, _cx| this.set_tool(Tool::Number)))
        .on_action(cx.listener(|this, _: &actions::ToolRedact, _, _cx| this.set_tool(Tool::Redact)))
        .on_action(cx.listener(|this, _: &actions::ToolCrop, _, _cx| this.set_tool(Tool::Crop)))
        .on_action(
            cx.listener(|this, _: &actions::ToolWallpaper, _, _cx| this.set_tool(Tool::Wallpaper)),
        )
        .on_action(cx.listener(|this, _: &actions::Undo, _, _cx| this.undo()))
        .on_action(cx.listener(|this, _: &actions::Redo, _, _cx| this.redo()))
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
            } else {
                this.text_editor = None;
                this.draft = None;
                this.sync_snapshot();
                cx.notify();
            }
        }))
        .on_action(cx.listener(|this, _: &actions::ToggleCaptureMode, _, cx| {
            this.toggle_capture_mode(cx);
        }))
        .on_action(
            cx.listener(|this, _: &actions::CopyAnnotation, window, cx| {
                // A selected annotation is copied; otherwise the whole image
                // is, which is what the renderer's Ctrl+C falls back to.
                if !this.copy_selected_annotation() {
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
        .on_action(cx.listener(|this, _: &actions::DeleteAnnotation, _, cx| {
            this.delete_selected_annotation(cx);
        }))
        .on_action(cx.listener(|this, _: &actions::ZoomIn, _, cx| {
            this.set_zoom(this.zoom + ZOOM_STEP);
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &actions::ZoomOut, _, cx| {
            this.set_zoom(this.zoom - ZOOM_STEP);
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &actions::ZoomReset, _, cx| {
            this.set_zoom(1.0);
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
            is_uploading: self.cloud_upload == crate::cloud::UploadState::Uploading,
            is_upload_done: self.cloud_upload == crate::cloud::UploadState::Success,
            is_capture_mode: self.capture_mode,
            menu: self.menu.clone(),
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
                .when(self.tool == Tool::Wallpaper, |el| {
                    el.child(crate::editor::wallpaper_sheet::render(
                        &self.wallpaper,
                        !self.layers.is_empty(),
                        &self.wallpaper_preset_id,
                        &self.menu,
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
                        .cursor_default()
                        .child(EditorCanvas::new(
                            snapshot_cell,
                            bounds_cell,
                            self.stage_scroll.clone(),
                        ))
                        .when(self.capture_mode, |el| {
                            let entity = cx.entity().downgrade();
                            el.child(crate::editor::canvas::capture_edge_overlay(
                                &theme,
                                std::rc::Rc::new(move |edge, _window, cx| {
                                    if let Some(entity) = entity.upgrade() {
                                        entity
                                            .update(cx, |editor, cx| editor.attach_layer(edge, cx));
                                    }
                                }),
                            ))
                        })
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let entity = down_entity;
                            move |event: &MouseDownEvent, window, cx| {
                                entity
                                    .update(cx, |editor, cx| {
                                        // Ctrl/Cmd-drag pans the stage instead of
                                        // drawing, matching `usePanOnDrag`.
                                        if event.modifiers.secondary() {
                                            editor.pan_origin = Some((
                                                event.position,
                                                editor.stage_scroll.offset(),
                                            ));
                                            return;
                                        }
                                        let bounds = *editor.bounds.borrow();
                                        let point =
                                            to_canvas_point(event.position, bounds, editor.zoom);
                                        editor.start_stroke(point, window, cx);
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
                                        if editor.move_origin.is_some()
                                            || editor.drag_start.is_some()
                                        {
                                            let bounds = *editor.bounds.borrow();
                                            let point =
                                                to_canvas_point(position, bounds, editor.zoom);
                                            editor.queue_pointer_update(point, window, cx);
                                        }
                                    })
                                    .ok();
                            }
                        })
                        .on_mouse_up(gpui::MouseButton::Left, {
                            let entity = up_entity;
                            move |_event: &MouseUpEvent, _window, cx| {
                                entity
                                    .update(cx, |editor, cx| {
                                        editor.pan_origin = None;
                                        editor.apply_pending_pointer_update();
                                        editor.finish_stroke();
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        })
                        .on_scroll_wheel({
                            let entity = cx.entity().downgrade();
                            move |event: &ScrollWheelEvent, _window, cx| {
                                if !event.modifiers.secondary() {
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
                                        editor.set_zoom(editor.zoom + step);
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
        )
        .children(self.text_editor_overlay())
    }
}

impl Drop for EditorWindow {
    fn drop(&mut self) {
        self.persist_editor_state(true);
    }
}

fn build_handlers(weak: &gpui::WeakEntity<EditorWindow>) -> EditorHandlers {
    let option_entity = weak.clone();
    let action_entity = weak.clone();

    EditorHandlers {
        on_option: Rc::new(move |option, _window, cx: &mut App| {
            let Some(entity) = option_entity.upgrade() else {
                return;
            };
            entity.update(cx, |editor, cx| {
                editor.apply_option(option, cx);
                editor.refresh_balance_crop();
                editor.refresh_backdrop(cx);
                editor.sync_snapshot();
                cx.notify();
            });
        }),
        on_action: Rc::new(move |action, window, cx: &mut App| {
            let Some(entity) = action_entity.upgrade() else {
                return;
            };
            if action == EditorAction::Pin {
                let png = entity.read(cx).export_png().ok();
                if let Some(bytes) = png {
                    crate::windows::pin::PinWindow::open(cx, bytes);
                }
                return;
            }
            entity.update(cx, |editor, cx| {
                editor.apply_action(action, window, cx);
                cx.notify();
            });
        }),
    }
}

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
        .child(
            Button::new("zoom-out")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("minus")
                .tooltip("Zoom Out")
                .disabled(zoom <= MIN_ZOOM)
                .on_click(move |_event, window, cx| zoom_out(window, cx)),
        )
        .child(
            div().min_w(px(chrome::ZOOM_RESET_MIN)).child(
                Button::new("zoom-reset")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Xs)
                    .foreground(theme.muted_foreground)
                    .label(format!("{}%", (zoom * 100.0).round() as i32))
                    .tooltip("Reset Zoom")
                    .on_click(move |_event, window, cx| zoom_reset(window, cx)),
            ),
        )
        .child(
            Button::new("zoom-in")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("plus")
                .tooltip("Zoom In")
                .disabled(zoom >= MAX_ZOOM)
                .on_click(move |_event, window, cx| zoom_in(window, cx)),
        )
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

    fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.sync_snapshot();
    }

    fn apply_option(&mut self, option: EditorOption, cx: &mut Context<Self>) {
        match option {
            EditorOption::Tool(tool) => self.set_tool(tool),
            EditorOption::Color(value) => self.color_hex = value.to_string(),
            EditorOption::StrokeWidth(value) => self.stroke_width = value,
            EditorOption::ArrowStyle(value) => self.arrow_style = value.to_string(),
            EditorOption::HighlightOpacity(value) => self.highlight_opacity = value,
            EditorOption::HighlightColor(value) => self.highlight_color = value.to_string(),
            EditorOption::NumberStyle(value) => self.number_style = value.to_string(),
            EditorOption::NumberSize(value) => self.number_size = value.to_string(),
            EditorOption::NumberStartValue(value) => self.number_start_value = value,
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
                let daemon = crate::state::state(cx).daemon;
                let background = cx.background_executor().clone();
                cx.spawn(async move |entity, cx| {
                    let source = background
                        .spawn(async move { crate::editor::background::desktop_wallpaper(&daemon) })
                        .await;
                    let _ = entity.update(cx, |editor, cx| match source {
                        Some(source) => {
                            editor.wallpaper.set_background_image(Some(source));
                            editor.refresh_backdrop(cx);
                            editor.sync_snapshot();
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
            EditorOption::WallpaperPickImage => {
                if let Some(path) = crate::editor::background::pick_image() {
                    let id = format!(
                        "custom-{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_millis())
                            .unwrap_or(0)
                    );
                    let background = crate::config::schema::CustomBackground {
                        id: id.clone(),
                        data: crate::config::schema::CustomBackgroundData::Image {
                            data: crate::config::schema::ImageBackgroundData {
                                image_url: path.clone(),
                            },
                        },
                    };
                    crate::state::state(cx).config.update(|settings| {
                        settings.wallpaper.custom_backgrounds.push(background);
                    });
                    self.wallpaper.set_background_image(Some(path));
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
            EditorOption::WallpaperSavePreset => {
                let count = crate::state::state(cx).config.get().wallpaper.presets.len();
                let id = format!(
                    "preset-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis())
                        .unwrap_or(0)
                );
                let name = format!("Preset {}", count + 1);
                let preset =
                    crate::editor::wallpaper::to_schema_preset(&self.wallpaper, id.clone(), name);
                crate::state::state(cx).config.update(|settings| {
                    settings.wallpaper.presets.push(preset);
                });
                self.wallpaper_preset_id = id;
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
            EditorOption::Zoom(value) => self.set_zoom(value),
        }
        self.sync_snapshot();
    }

    fn apply_action(&mut self, action: EditorAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            EditorAction::Undo => self.undo(),
            EditorAction::Redo => self.redo(),
            EditorAction::Copy => self.copy_to_clipboard(window, cx),
            EditorAction::Save => self.save_as(window, cx),
            EditorAction::CloudUpload => self.upload_to_cloud(cx),
            EditorAction::CaptureToggle => self.toggle_capture_mode(cx),
            EditorAction::Pin => {}
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.sync_snapshot();
    }

    fn focus_text_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, field)) = &self.text_editor else {
            return;
        };
        window.focus(&field.read(cx).focus_handle());
    }

    fn undo(&mut self) {
        self.history.undo();
        self.refresh_redact_patches();
        self.sync_snapshot();
    }

    fn redo(&mut self) {
        self.history.redo();
        self.refresh_redact_patches();
        self.sync_snapshot();
    }
}
fn load_image(
    path: &str,
) -> anyhow::Result<(
    Option<Arc<gpui::RenderImage>>,
    Option<image::DynamicImage>,
    f32,
    f32,
)> {
    let bytes = std::fs::read(path)?;
    let decoded = image::load_from_memory(&bytes)?;
    let (width, height) = (decoded.width() as f32, decoded.height() as f32);

    let base = image::DynamicImage::ImageRgba8(decoded.to_rgba8());

    // GPUI composites in BGRA; swap channels once at decode time.
    let mut buffer = decoded.to_rgba8();
    for pixel in buffer.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let frame = image::Frame::new(buffer);
    let render_image = gpui::RenderImage::new(smallvec::smallvec![frame]);

    Ok((Some(Arc::new(render_image)), Some(base), width, height))
}

impl EditorWindow {
    /// Renders the current composition (base + annotations) to PNG bytes at
    /// natural size.
    pub fn export_png(&self) -> anyhow::Result<Vec<u8>> {
        let canvas = crate::editor::export::compose_with_layers(
            self.base_image.as_deref(),
            self.image_width as u32,
            self.image_height as u32,
            self.history.current(),
            &self.wallpaper,
            &self.layers,
        );
        crate::editor::export::encode_png(&canvas)
    }

    pub fn copy_to_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.export_png() {
            Ok(png) => {
                crate::system::clipboard::ClipboardService::write_png(cx, png);
                self.is_copied = true;
                if crate::state::state(cx)
                    .config
                    .get()
                    .screenshot
                    .close_on_copy
                {
                    window.remove_window();
                    return;
                }
                cx.notify();
                let entity = cx.entity().downgrade();
                cx.spawn(async move |_entity, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    if let Some(entity) = entity.upgrade() {
                        let _ = entity.update(cx, |editor, cx| {
                            editor.is_copied = false;
                            cx.notify();
                        });
                    }
                })
                .detach();
            }
            Err(error) => eprintln!("[editor] copy failed: {error}"),
        }
    }

    /// Uploads the composed image to the user's own provider and copies the
    /// returned URL, mirroring `useCloudFileUpload`.
    pub fn upload_to_cloud(&mut self, cx: &mut Context<Self>) {
        use crate::cloud::UploadState;

        if self.cloud_upload == UploadState::Uploading {
            return;
        }
        let config = crate::state::state(cx).config.get().cloud;
        if !crate::cloud::is_configured(&config) {
            crate::intents::open_settings(crate::windows::settings::registry::Category::Cloud, cx);
            return;
        }
        let png = match self.export_png() {
            Ok(png) => png,
            Err(error) => {
                eprintln!("[cloud] export failed: {error}");
                return;
            }
        };
        let name = crate::editor::filename::generate_screenshot_export_name("png");
        let path = std::env::temp_dir().join(&name);

        self.cloud_upload = UploadState::Uploading;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let uploaded = cx
                .background_executor()
                .spawn(async move {
                    std::fs::write(&path, png)?;
                    let url = crate::cloud::upload(&config, &path);
                    let _ = std::fs::remove_file(&path);
                    url
                })
                .await;

            let (state, title, body) = match uploaded {
                Ok(url) => (UploadState::Success, "Link copied", url),
                Err(error) => (UploadState::Error, "Upload failed", error.to_string()),
            };

            let _ = cx.update(|cx| {
                if state == UploadState::Success {
                    crate::system::clipboard::ClipboardService::write_text(cx, body.clone());
                }
                crate::windows::toast::Toast::show(cx, title, body)
            });
            let _ = entity.update(cx, |editor, cx| {
                editor.cloud_upload = state;
                cx.notify();
            });

            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = entity.update(cx, |editor, cx| {
                editor.cloud_upload = UploadState::Idle;
                cx.notify();
            });
        })
        .detach();
    }

    /// Prints the composed image through the daemon's `print` module.
    pub fn print(&mut self, cx: &mut Context<Self>) {
        use base64::Engine as _;

        if !crate::system::capabilities::is_supported(crate::system::capabilities::Feature::Print) {
            crate::windows::toast::Toast::show(
                cx,
                "Printing unavailable",
                "This platform does not support printing",
            );
            return;
        }
        let png = match self.export_png() {
            Ok(png) => png,
            Err(error) => {
                eprintln!("[print] export failed: {error}");
                return;
            }
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        let daemon = crate::state::state(cx).daemon;
        let background = cx.background_executor().clone();
        cx.spawn(async move |_, cx| {
            let result = background
                .spawn(async move { daemon.print().image(encoded) })
                .await;
            if let Err(error) = result {
                let _ = cx.update(|cx| {
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
        if !crate::history_store::delete_path(
            &path,
            crate::history_store::HistoryItemType::Screenshot,
        ) {
            crate::windows::toast::Toast::show(
                cx,
                "Delete failed",
                "The screenshot could not be deleted",
            );
            return;
        }
        let notify = crate::state::state(cx)
            .config
            .get()
            .general
            .show_deletion_notifications;
        window.remove_window();
        if notify {
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

    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let png = match self.export_png() {
            Ok(png) => png,
            Err(error) => {
                eprintln!("[editor] export failed: {error}");
                return;
            }
        };

        let settings = crate::state::state(cx).config.get();
        let jpeg = settings.screenshot.format == "jpeg";
        let extension = if jpeg { "jpg" } else { "png" };
        let bytes = if jpeg {
            let decoded = match image::load_from_memory(&png) {
                Ok(image) => image.to_rgb8(),
                Err(error) => {
                    eprintln!("[editor] jpeg conversion failed: {error}");
                    return;
                }
            };
            let mut bytes = Vec::new();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92);
            if let Err(error) = encoder.encode_image(&decoded) {
                eprintln!("[editor] jpeg encoding failed: {error}");
                return;
            }
            bytes
        } else {
            png
        };
        let default_name = crate::editor::filename::generate_screenshot_export_name(extension);
        let saved_directory = std::path::PathBuf::from(&settings.save_locations.screenshot);
        let directory = if saved_directory.is_dir() {
            saved_directory
        } else {
            dirs::picture_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
        };
        let path = rfd::FileDialog::new()
            .set_title("Save Screenshot")
            .set_file_name(&default_name)
            .add_filter(if jpeg { "JPEG Image" } else { "PNG Image" }, &[extension])
            .set_directory(&directory)
            .save_file();

        if let Some(path) = path {
            match std::fs::write(&path, bytes) {
                Ok(()) => {
                    if let Some(parent) = path.parent() {
                        let parent = parent.to_string_lossy().to_string();
                        crate::state::state(cx)
                            .config
                            .update(move |config| config.save_locations.screenshot = parent);
                    }
                    if settings.screenshot.close_on_save {
                        window.remove_window();
                    }
                }
                Err(error) => eprintln!("[editor] save failed: {error}"),
            }
        }
    }
}
