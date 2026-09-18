pub mod data_editor;
pub mod drawing_overlay;
pub mod drawing_paint;
pub mod model;
pub mod panel_kit;
pub mod panels;
pub mod preview;
pub mod sidebar;
pub mod styles;
pub mod timeline;
pub mod title_bar;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
const EXPORT_COMPLETION_MS: u64 = 3000;
const COPY_FEEDBACK_MS: u64 = 2000;
const PROGRESS_TWEEN_MS: u64 = 300;
const PREVIEW_INSET: f32 = 16.0;
const UNDO_HISTORY_LIMIT: usize = 100;
const FRAME_STEP: f64 = 1.0 / 30.0;
const LAST_FRAME_EPSILON: f64 = 0.01;
const ACTIVE_PREVIEW_SIZES: [(u32, u32); 7] = [
    (384, 216),
    (432, 243),
    (480, 270),
    (640, 360),
    (854, 480),
    (960, 540),
    (1280, 720),
];
const INITIAL_PREVIEW_QUALITY: usize = 3;
const PREVIEW_QUALITY_RAISE_FRAMES: u16 = 120;
const SIDEBAR_ANIMATION_DURATION: Duration = Duration::from_millis(180);
/// Playback pulls one composed frame per tick; the composition is software
/// rasterized, so this is a preview rate rather than the export frame rate.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, img, prelude::*, px, size, App, Bounds, Context, DispatchPhase, Entity,
    FocusHandle, Focusable, KeyDownEvent, Render, ScrollHandle, SharedString, Styled, Window,
};
use herogpui::gpui;

use crate::editor::annotations::Annotation;
use crate::editor::options::EditorOption;
use crate::system::desktop;
use crate::theme::vars::active_theme;
use crate::ui::icon::icon_element;
use crate::ui::menu::MenuHandle;
use crate::video::composition::segments as timeline_segments;
use crate::video::project;
use crate::windows::registry::{self, WindowKind as RegistryKind};
use crate::windows::video_editor::model::VideoEditorState;
use crate::windows::video_editor::sidebar::SidebarTab;
use crate::windows::video_editor::timeline::edit;
use crate::windows::video_editor::timeline::tracks::{Track, TrackKind};

/// What a clip drag is doing — the body moves it, the edges resize it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMode {
    Move,
    ResizeStart,
    ResizeEnd,
}

/// The gesture in progress on a timeline clip. One history entry is pushed when
/// it ends, so a drag is a single undo step.
#[derive(Clone, Debug)]
pub struct ClipDrag {
    pub kind: TrackKind,
    pub id: SharedString,
    pub mode: DragMode,
    /// Where inside the clip the pointer grabbed it.
    pub grab_offset: f64,
}

#[derive(Clone, Debug)]
pub struct ReorderDrag {
    pub id: SharedString,
    pub drop_index: usize,
    pub origin_x: f32,
    pub dragging: bool,
}

#[derive(Clone, Debug)]
pub struct DrawDrag {
    pub kind: TrackKind,
    pub start: f64,
    pub end: f64,
}

/// Where the preview pipeline is: a project has to be decoded before the first
/// composed frame can appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewStatus {
    Idle,
    Loading,
    Ready,
    Unavailable,
}

/// The subtitle panel's generation state — the `downloadStatus` /
/// `generationStatus` pair in `subtitle-settings-panel.tsx` merged into one.
/// The readiness check is synchronous here (two file-exists probes), so the
/// Electron `checking` flash has no equivalent: the panel renders the model
/// metadata and the button jumps straight to `Downloading` or `Generating`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TranscriptionStatus {
    #[default]
    Idle,
    Downloading(u8),
    Generating(u8),
    Failed(String),
}

/// One step of a generation run, pumped from the background worker to the
/// window in order — the `Finished` event always lands last, so the terminal
/// state can never be overwritten by a late percent.
enum TranscriptionEvent {
    Download(u8),
    Generate(u8),
    Finished(Result<usize, String>),
}

pub struct VideoEditorWindow {
    path: Option<PathBuf>,
    state: VideoEditorState,
    history: Vec<VideoEditorState>,
    future: Vec<VideoEditorState>,
    pixels_per_second: f32,
    playhead: f64,
    is_playing: bool,
    is_scrubbing: bool,
    is_cut_tool_active: bool,
    is_exporting: bool,
    export_progress: f32,
    selected_clip: Option<SharedString>,
    menu: MenuHandle,
    save_generation: u64,
    ruler_scroll: ScrollHandle,
    tracks_scroll: ScrollHandle,
    pane_scroll: ScrollHandle,
    panel_scroll: ScrollHandle,
    export_scroll: ScrollHandle,
    timeline_height: f32,
    timeline_resize: Option<(f32, f32)>,
    focus_handle: FocusHandle,
    source: Option<preview::Handle>,
    source_frame_rate: f64,
    preview_status: PreviewStatus,
    preview_image: Option<Arc<gpui::RenderImage>>,
    /// One compose runs at a time; a request that arrives while one is in
    /// flight replaces the queued position rather than piling up.
    compose_in_flight: bool,
    queued_frame: Option<f64>,
    preview_quality: usize,
    preview_fast_frames: u16,
    preview_state_in_flight: bool,
    preview_state_queued: bool,
    playback_generation: u64,
    preview_audio: crate::video::preview_audio::PreviewAudio,
    audio_generation: u64,
    scrub_audio_generation: u64,
    /// The export runs on the background executor; progress is published in
    /// permille and cancellation is a flag the render loop checks per frame.
    export_progress_permille: Arc<AtomicU32>,
    export_cancelled: Arc<AtomicBool>,
    /// When the running export started, and the smoothed ETA the progress
    /// footer shows — port of `useExportProgress`.
    export_started_at: Option<std::time::Instant>,
    export_remaining: Option<f64>,
    clip_drag: Option<ClipDrag>,
    reorder_drag: Option<ReorderDrag>,
    draw_drag: Option<DrawDrag>,
    preview_playhead: Option<f64>,
    /// The transcript's segment count, refreshed whenever it is written.
    subtitle_count: usize,
    cursor_summary: Option<SharedString>,
    subtitle_summary: Option<SharedString>,
    /// The open JSON data editor, if any.
    data_editor: Option<data_editor::DataEditor>,
    transcription: TranscriptionStatus,
    transcription_model: String,
    transcription_prompt: String,
    prompt_field: Entity<herogpui::components::InputState>,
    rename_field: Entity<herogpui::components::InputState>,
    rename_error: Option<SharedString>,
    path_copied_at: Option<std::time::Instant>,
    url_copied_at: Option<std::time::Instant>,
    export_error: Option<SharedString>,
    export_completed_at: Option<std::time::Instant>,
    displayed_progress: f32,
    upload_to_cloud: bool,
    cloud_upload: crate::cloud::UploadState,
    uploaded_url: Option<String>,
    drawing_tools: styles::DrawingToolSettings,
    drawing_stroke: Option<drawing_overlay::Stroke>,
    drawing_id_seed: u64,
    drawing_text_field: Entity<herogpui::components::InputState>,
    preview_bounds: drawing_overlay::BoundsCell,
    composition_size: Option<(f64, f64)>,
    sidebar_width: f32,
    sidebar_resize: Option<(f32, f32)>,
    sidebar_animation_generation: u64,
    sidebar_closing: bool,
    desktop_wallpaper_source: Option<String>,
    desktop_wallpaper_preview: Option<Arc<gpui::RenderImage>>,
    keyboard_demo: Option<std::sync::Arc<AtomicBool>>,
    /// The state a drag started from, pushed onto the undo stack when it ends.
    gesture_snapshot: Option<VideoEditorState>,
}

impl VideoEditorWindow {
    pub fn open(cx: &mut App, path: Option<String>) {
        registry::open_or_activate(RegistryKind::VideoEditor, cx, move |cx| {
            let available = cx
                .displays()
                .first()
                .map(|display| display.bounds().size)
                .unwrap_or_else(|| size(px(1280.0), px(800.0)));
            let bounds = Bounds::centered(
                None,
                size(
                    px(1280.0).min(available.width - px(100.0)),
                    px(800.0).min(available.height - px(100.0)),
                ),
                cx,
            );
            cx.open_window(
                crate::windows::app_window_options(bounds, Some(size(px(1200.0), px(750.0)))),
                |window, cx| {
                    let view = cx.new(|cx| {
                        let mut editor = Self::new(path.map(PathBuf::from), cx);
                        editor.load_preview(cx);
                        editor.load_desktop_wallpaper(cx);
                        editor.request_audio_sync(cx);
                        editor
                    });
                    let focus = view.read(cx).focus_handle.clone();
                    window.focus(&focus, cx);
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
    }

    fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let has_saved_state = path
            .as_deref()
            .is_some_and(|path| crate::video::project::editor_state_path(path).is_file());
        let mut state = path.as_deref().map(model::load_state).unwrap_or_default();
        if !has_saved_state {
            let recording = crate::state::state(cx).config.get().recording;
            state.camera_style.mirrored = recording.camera.flipped;
            if recording.auto_zoom {
                if let Some(cursor_data) = path
                    .as_deref()
                    .and_then(crate::video::sidecars::load_cursor)
                {
                    state.zoom_segments = crate::video::auto_zoom::generate(
                        &cursor_data,
                        chrono::Utc::now().timestamp_millis(),
                    );
                    if !state.zoom_segments.is_empty() {
                        state.ui.sidebar_open = true;
                        state.ui.sidebar_tab = "zoom".to_string();
                    }
                }
            }
        }
        // A recording whose camera was toggled on and off records its
        // on-periods in `camera.json`; the first open turns them into timeline
        // segments, the same way `useCameraSegments` seeds itself.
        if state.camera_segments.is_empty() {
            if let Some(meta) = path
                .as_deref()
                .and_then(crate::video::sidecars::load_camera_meta)
            {
                let total = model::total_duration(
                    &state.segments,
                    state.source_duration.unwrap_or(meta.duration),
                );
                state.camera_segments = crate::video::sidecars::map_visible_ranges_to_segments(
                    meta.visible_ranges.as_deref(),
                    &state.segments,
                    total,
                );
            }
        }
        styles::normalize_export_settings(&mut state.export_settings);
        let pixels_per_second = state
            .timeline_zoom
            .map(|value| timeline::clamp_zoom(value as f32))
            .unwrap_or(timeline::DEFAULT_PIXELS_PER_SECOND);
        let timeline_height = state
            .ui
            .timeline_height
            .map(|value| timeline::clamp_pane_height(value as f32))
            .unwrap_or_else(timeline::default_pane_height);
        if let Some(project) = path.as_deref() {
            materialize_builtin_audio_tracks(&mut state, project);
        }
        let initial_name = path
            .as_deref()
            .map(model::project_display_name)
            .unwrap_or_else(|| "Untitled".to_string());
        let rename_field =
            cx.new(|cx| herogpui::components::InputState::with_value(cx, initial_name));
        Self {
            path,
            state,
            history: Vec::new(),
            future: Vec::new(),
            pixels_per_second,
            playhead: 0.0,
            is_playing: false,
            is_scrubbing: false,
            is_cut_tool_active: false,
            is_exporting: false,
            export_progress: 0.0,
            selected_clip: None,
            menu: MenuHandle::new(),
            save_generation: 0,
            ruler_scroll: ScrollHandle::new(),
            tracks_scroll: ScrollHandle::new(),
            pane_scroll: ScrollHandle::new(),
            panel_scroll: ScrollHandle::new(),
            export_scroll: ScrollHandle::new(),
            timeline_height,
            timeline_resize: None,
            focus_handle: cx.focus_handle(),
            source: None,
            source_frame_rate: 60.0,
            preview_status: PreviewStatus::Idle,
            preview_image: None,
            compose_in_flight: false,
            queued_frame: None,
            preview_quality: INITIAL_PREVIEW_QUALITY,
            preview_fast_frames: 0,
            preview_state_in_flight: false,
            preview_state_queued: false,
            playback_generation: 0,
            preview_audio: crate::video::preview_audio::PreviewAudio::new(),
            audio_generation: 0,
            scrub_audio_generation: 0,
            export_progress_permille: Arc::new(AtomicU32::new(0)),
            export_cancelled: Arc::new(AtomicBool::new(false)),
            export_started_at: None,
            export_remaining: None,
            clip_drag: None,
            reorder_drag: None,
            draw_drag: None,
            preview_playhead: None,
            gesture_snapshot: None,
            subtitle_count: 0,
            cursor_summary: None,
            subtitle_summary: None,
            data_editor: None,
            transcription: TranscriptionStatus::Idle,
            transcription_model: "base".to_string(),
            transcription_prompt: String::new(),
            prompt_field: cx.new(|cx| herogpui::components::InputState::new(cx)),
            rename_field,
            rename_error: None,
            path_copied_at: None,
            url_copied_at: None,
            export_error: None,
            export_completed_at: None,
            displayed_progress: 0.0,
            upload_to_cloud: false,
            cloud_upload: crate::cloud::UploadState::Idle,
            uploaded_url: None,
            drawing_tools: styles::DrawingToolSettings::default(),
            drawing_stroke: None,
            drawing_id_seed: 0,
            drawing_text_field: cx.new(|cx| herogpui::components::InputState::new(cx)),
            preview_bounds: Rc::new(RefCell::new(None)),
            composition_size: None,
            sidebar_width: crate::ui::chrome::VIDEO_SIDEBAR_WIDTH,
            sidebar_resize: None,
            sidebar_animation_generation: 0,
            sidebar_closing: false,
            desktop_wallpaper_source: None,
            desktop_wallpaper_preview: None,
            keyboard_demo: None,
        }
    }

    /// Asks for a destination and renders the timeline to it. Every frame goes
    /// through the composition engine the preview uses.
    pub fn start_export(&mut self, cx: &mut Context<Self>) {
        if self.is_exporting {
            return;
        }
        let Some(project) = self.path.clone() else {
            return;
        };
        let state = self.state.clone();
        let suggested = crate::video::export::default_output_path(&project, &state);
        let is_gif = state.export_settings.format == "gif";
        let (filter_name, extensions): (&str, &[&str]) = if is_gif {
            ("GIF Image", &["gif"])
        } else {
            ("MP4 Video", &["mp4"])
        };
        let saved_directory = PathBuf::from(
            crate::state::state(cx)
                .config
                .get()
                .save_locations
                .video
                .clone(),
        );
        let directory = match saved_directory.is_dir() {
            true => saved_directory,
            false => suggested.parent().unwrap_or(&project).to_path_buf(),
        };
        let Some(output) = rfd::FileDialog::new()
            .add_filter(filter_name, extensions)
            .set_file_name(
                suggested
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(if is_gif { "export.gif" } else { "export.mp4" }),
            )
            .set_directory(&directory)
            .save_file()
        else {
            return;
        };
        if let Some(parent) = output.parent() {
            let parent = parent.to_string_lossy().to_string();
            crate::state::state(cx)
                .config
                .update(move |config| config.save_locations.video = parent);
        }
        let output = match is_gif
            && !output
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("gif"))
        {
            true => output.with_extension("gif"),
            false => output,
        };

        self.is_exporting = true;
        self.export_error = None;
        self.export_completed_at = None;
        self.export_progress = 0.0;
        self.displayed_progress = 0.0;
        self.export_progress_permille = Arc::new(AtomicU32::new(0));
        self.export_cancelled = Arc::new(AtomicBool::new(false));
        self.export_started_at = Some(cx.background_executor().now());
        self.export_remaining = None;
        cx.notify();

        let progress = self.export_progress_permille.clone();
        let cancelled = self.export_cancelled.clone();
        let was_cancelled = self.export_cancelled.clone();
        let reveal = state.export_settings.open_in_finder;
        let started = cx.background_executor().now();

        self.poll_export_progress(cx);
        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::video::export::run(
                        crate::video::export::Request {
                            project,
                            output,
                            state,
                        },
                        &mut |fraction| {
                            progress.store((fraction * 1000.0) as u32, Ordering::Relaxed);
                        },
                        &|| cancelled.load(Ordering::Relaxed),
                    )
                })
                .await;

            let user_cancelled = was_cancelled.load(Ordering::Relaxed);
            let failure = match (&result, user_cancelled) {
                (Err(error), false) => Some(error.clone()),
                _ => None,
            };
            if let Ok(path) = &result {
                crate::system::notification::show(
                    "Export Complete",
                    &format!(
                        "Video exported successfully in {}",
                        format_export_duration(
                            cx.background_executor()
                                .now()
                                .saturating_duration_since(started)
                                .as_secs_f64()
                        )
                    ),
                );
                if reveal {
                    crate::system::desktop::reveal_in_file_manager(path);
                }
            }
            let output = result.ok();
            let _ = entity.update(cx, |this, cx| {
                this.is_exporting = false;
                this.export_progress = 0.0;
                this.displayed_progress = 0.0;
                this.export_started_at = None;
                this.export_remaining = None;
                this.export_error = failure.map(SharedString::from);
                if output.is_some() {
                    this.export_completed_at = Some(cx.background_executor().now());
                    this.schedule_export_completion_clear(cx);
                }
                if this.upload_to_cloud {
                    if let Some(path) = output {
                        this.upload_export(path, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn upload_export(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let config = crate::state::state(cx).config.get().cloud;
        self.cloud_upload = crate::cloud::UploadState::Uploading;
        self.uploaded_url = None;
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let uploaded = cx
                .background_executor()
                .spawn(async move { crate::cloud::upload(&config, &path) })
                .await;
            let _ = entity.update(cx, |this, cx| {
                match uploaded {
                    Ok(url) => {
                        this.cloud_upload = crate::cloud::UploadState::Success;
                        this.uploaded_url = Some(url);
                    }
                    Err(_) => {
                        this.cloud_upload = crate::cloud::UploadState::Error;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Mirrors the background export's progress into the view.
    fn poll_export_progress(&mut self, cx: &mut Context<Self>) {
        let progress = self.export_progress_permille.clone();
        cx.spawn(async move |entity, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let running = entity.update(cx, |this, cx| {
                if !this.is_exporting {
                    return false;
                }
                this.export_progress = progress.load(Ordering::Relaxed) as f32 / 1000.0;
                this.displayed_progress =
                    tween_progress(this.displayed_progress, this.export_progress, 100);
                this.update_export_eta(cx);
                cx.notify();
                true
            });
            if !matches!(running, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    /// Port of the ETA half of `useExportProgress`: below 5% the estimate
    /// is withheld, above it the raw projection is smoothed at 0.1.
    fn update_export_eta(&mut self, cx: &App) {
        let Some(started) = self.export_started_at else {
            self.export_remaining = None;
            return;
        };
        if self.export_progress <= 0.05 {
            self.export_remaining = None;
            return;
        }
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(started)
            .as_secs_f64();
        let raw = (elapsed / f64::from(self.export_progress) - elapsed).max(0.0);
        self.export_remaining = Some(match self.export_remaining {
            Some(previous) => previous + 0.1 * (raw - previous),
            None => raw,
        });
    }

    pub fn export_elapsed_secs(&self, cx: &App) -> u64 {
        let now = cx.background_executor().now();
        self.export_started_at
            .map(|started| now.saturating_duration_since(started).as_secs())
            .unwrap_or(0)
    }

    pub fn export_remaining_secs(&self) -> Option<u64> {
        self.export_remaining
            .map(|remaining| remaining.round() as u64)
    }

    /// Opens the recording and its sidecars off the UI thread, then composes
    /// the frame under the playhead.
    fn load_preview(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let state = self.state.clone();
        self.preview_status = PreviewStatus::Loading;
        self.refresh_subtitle_count();
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let opened = cx
                .background_executor()
                .spawn(async move { preview::Source::open(&path, state) })
                .await;
            let _ = entity.update(cx, |this, cx| {
                match opened {
                    Some(source) => {
                        // `handleBootstrapMetadata` takes the duration off the
                        // `<video>` element's `loadedmetadata`. Nothing did that
                        // here, so a recording with no saved project sat at a
                        // total duration of zero: the player read `0:00 / 0:00`
                        // and the timeline had nothing to lay out.
                        let info = source.info();
                        let duration = info.duration;
                        this.source_frame_rate = info.frame_rate();
                        this.composition_size = Some(source.composition_size());
                        this.source = Some(Arc::new(parking_lot::Mutex::new(source)));
                        this.adopt_source_duration(duration, cx);
                        this.preview_status = PreviewStatus::Ready;
                        this.request_frame(cx);
                    }
                    None => this.preview_status = PreviewStatus::Unavailable,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_desktop_wallpaper(&mut self, cx: &mut Context<Self>) {
        if !crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::DesktopWallpaper,
        ) {
            return;
        }
        let daemon = crate::state::state(cx).daemon.clone();
        cx.spawn(async move |entity, cx| {
            let wallpaper = cx
                .background_executor()
                .spawn(async move {
                    let source = crate::editor::background::desktop_wallpaper(&daemon)?;
                    let image = crate::render::gradient::load_image(&source)
                        .as_ref()
                        .and_then(preview::to_render_image)?;
                    Some((source, image))
                })
                .await;
            let _ = entity.update(cx, |this, cx| {
                if let Some((source, image)) = wallpaper {
                    this.desktop_wallpaper_source = Some(source);
                    this.desktop_wallpaper_preview = Some(image);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Queues a compose at the current playhead.
    fn request_frame(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let time = self.preview_playhead.unwrap_or(self.playhead);
        let max_dimensions =
            (self.is_playing || self.is_scrubbing || self.gesture_snapshot.is_some())
                .then_some(ACTIVE_PREVIEW_SIZES[self.preview_quality]);
        if self.compose_in_flight {
            self.queued_frame = Some(time);
            return;
        }
        self.compose_in_flight = true;

        cx.spawn(async move |entity, cx| {
            let (composed, elapsed) = cx
                .background_executor()
                .spawn(async move {
                    let started = std::time::Instant::now();
                    let mut guard = source.lock();
                    let image = guard
                        .compose(time, max_dimensions)
                        .as_ref()
                        .and_then(preview::to_render_image);
                    (image, started.elapsed())
                })
                .await;

            let _ = entity.update(cx, |this, cx| {
                this.compose_in_flight = false;
                if max_dimensions.is_some() {
                    (this.preview_quality, this.preview_fast_frames) = adjust_preview_quality(
                        this.preview_quality,
                        this.preview_fast_frames,
                        elapsed,
                        active_preview_budget(this.source_frame_rate),
                    );
                }
                if let Some(image) = composed {
                    // Every composed frame is a new texture, so the previous
                    // one has to leave the sprite atlas with it.
                    if let Some(previous) = this.preview_image.replace(image) {
                        cx.drop_image(previous, None);
                    }
                }
                cx.notify();
                // A position queued while this compose ran is already the
                // playhead's, so the follow-up only has to be scheduled.
                if this.queued_frame.take().is_some() {
                    this.request_frame(cx);
                }
            });
        })
        .detach();
    }

    /// Pushes edited state into the composition engine and recomposes.
    fn sync_preview_state(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if self.preview_state_in_flight {
            self.preview_state_queued = true;
            return;
        }
        self.preview_state_in_flight = true;
        let state = self.state.clone();
        cx.spawn(async move |entity, cx| {
            let composition = cx
                .background_executor()
                .spawn(async move {
                    let mut guard = source.lock();
                    guard.set_state(state);
                    guard.composition_size()
                })
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.preview_state_in_flight = false;
                this.composition_size = Some(composition);
                if std::mem::take(&mut this.preview_state_queued) {
                    this.sync_preview_state(cx);
                    return;
                }
                this.request_frame(cx);
            });
        })
        .detach();
    }

    /// Advances the playhead in wall-clock time while playing. Each run is
    /// tagged so a pause or a second play stops the previous loop.
    fn drive_playback(&mut self, cx: &mut Context<Self>) {
        self.playback_generation += 1;
        let generation = self.playback_generation;
        let tick = playback_tick(self.source_frame_rate);
        let started = cx.background_executor().now();
        let initial_playhead = self.playhead;

        cx.spawn(async move |entity, cx| loop {
            cx.background_executor().timer(tick).await;
            let advanced = entity.update(cx, |this, cx| {
                if !this.is_playing || this.playback_generation != generation {
                    return false;
                }
                let total = this.total_duration();
                let next = initial_playhead
                    + cx.background_executor()
                        .now()
                        .saturating_duration_since(started)
                        .as_secs_f64();
                if next >= total {
                    this.playhead = total;
                    this.is_playing = false;
                    this.preview_audio.pause_all();
                    this.request_frame(cx);
                    cx.notify();
                    return false;
                }
                this.playhead = next;
                this.sync_audio_transport();
                this.autoscroll_timeline();
                this.request_frame(cx);
                cx.notify();
                true
            });
            if !matches!(advanced, Ok(true)) {
                break;
            }
        })
        .detach();
    }

    /// Records the decoded duration, unless the project already carries one --
    /// a saved project's value is authoritative because trims and speed changes
    /// are expressed against it.
    fn adopt_source_duration(&mut self, duration: f64, cx: &mut Context<Self>) {
        if !duration.is_finite() || duration <= 0.0 {
            return;
        }
        if self.state.source_duration.is_some() {
            return;
        }
        self.state.source_duration = Some(duration);
        // `initializeDocument({ segments: defaultSegments })` when there is no
        // saved project: one segment covering the whole recording. Without it
        // the ruler is drawn over an empty lane.
        if self.state.segments.is_empty() {
            self.state.segments = vec![model::Segment::spanning(duration)];
        }
        if let Some(project) = self.path.clone() {
            if materialize_builtin_audio_tracks(&mut self.state, &project) {
                self.persist(cx);
            }
        }
        self.request_audio_sync(cx);
        cx.notify();
    }

    fn total_duration(&self) -> f64 {
        first_frame_duration(&self.state)
            + model::total_duration(
                &self.state.segments,
                self.state.source_duration.unwrap_or(0.0),
            )
    }

    fn active_tab(&self) -> SidebarTab {
        SidebarTab::parse(&self.state.ui.sidebar_tab)
    }

    fn commit<F>(&mut self, cx: &mut Context<Self>, mutate: F)
    where
        F: FnOnce(&mut VideoEditorState),
    {
        let before = self.state.clone();
        mutate(&mut self.state);
        if self.state == before {
            return;
        }
        if self.gesture_snapshot.is_some() {
            self.sync_preview_state(cx);
            self.request_audio_sync(cx);
            cx.notify();
            return;
        }
        if self.history.len() >= UNDO_HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(before);
        self.future.clear();
        self.persist(cx);
        self.sync_preview_state(cx);
        self.request_audio_sync(cx);
        cx.notify();
    }

    /// Takes the snapshot a gesture will be undone to. Repeated calls during
    /// one gesture keep the first snapshot.
    fn begin_gesture(&mut self) {
        if self.gesture_snapshot.is_none() {
            self.gesture_snapshot = Some(self.state.clone());
        }
    }

    pub fn begin_slider_gesture(&mut self) {
        self.begin_gesture();
    }

    pub fn end_slider_gesture(&mut self, cx: &mut Context<Self>) {
        self.end_gesture(cx);
    }

    /// Closes a gesture, pushing one history entry for everything it changed.
    fn end_gesture(&mut self, cx: &mut Context<Self>) {
        let Some(before) = self.gesture_snapshot.take() else {
            return;
        };
        if before == self.state {
            self.request_frame(cx);
            return;
        }
        if self.history.len() >= UNDO_HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(before);
        self.future.clear();
        self.persist(cx);
        self.sync_preview_state(cx);
        self.request_audio_sync(cx);
        cx.notify();
    }

    /// Slider drags land many changes per second, so writes are debounced the
    /// same way `use-editor-state-persistence.ts` does.
    fn persist(&mut self, cx: &mut Context<Self>) {
        if self.path.is_none() {
            return;
        }
        self.save_generation = self.save_generation.wrapping_add(1);
        let generation = self.save_generation;
        cx.spawn(async move |entity, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            let _ = entity.update(cx, |this, _cx| {
                if this.save_generation != generation {
                    return;
                }
                let Some(path) = this.path.clone() else {
                    return;
                };
                let mut written = this.state.clone();
                written.saved_at = chrono::Utc::now().to_rfc3339();
                model::save_state(&path, &written);
                this.state.saved_at = written.saved_at;
            });
        })
        .detach();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(mut previous) = self.history.pop() else {
            return;
        };
        previous.export_settings = self.state.export_settings.clone();
        self.future
            .push(std::mem::replace(&mut self.state, previous));
        self.persist(cx);
        self.sync_preview_state(cx);
        self.request_audio_sync(cx);
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        let Some(mut next) = self.future.pop() else {
            return;
        };
        next.export_settings = self.state.export_settings.clone();
        self.history.push(std::mem::replace(&mut self.state, next));
        self.persist(cx);
        self.sync_preview_state(cx);
        self.request_audio_sync(cx);
        cx.notify();
    }

    pub fn reset_state(&mut self, cx: &mut Context<Self>) {
        let source_duration = self.state.source_duration;
        let recording_type = self.state.recording_type.clone();
        let recording = crate::state::state(cx).config.get().recording;
        let mut state = VideoEditorState {
            source_duration,
            recording_type,
            ..VideoEditorState::default()
        };
        state.camera_style.mirrored = recording.camera.flipped;
        if let Some(duration) = source_duration.filter(|duration| *duration > 0.0) {
            state.segments = vec![model::Segment::spanning(duration)];
        }
        if recording.auto_zoom {
            if let Some(cursor_data) = self
                .path
                .as_deref()
                .and_then(crate::video::sidecars::load_cursor)
            {
                state.zoom_segments = crate::video::auto_zoom::generate(
                    &cursor_data,
                    chrono::Utc::now().timestamp_millis(),
                );
                if !state.zoom_segments.is_empty() {
                    state.ui.sidebar_open = true;
                    state.ui.sidebar_tab = "zoom".to_string();
                }
            }
        }
        self.state = state;
        self.history.clear();
        self.future.clear();
        self.selected_clip = None;
        self.persist(cx);
        self.sync_preview_state(cx);
        self.request_audio_sync(cx);
        cx.notify();
    }

    pub fn confirm_reset(&mut self, cx: &mut Context<Self>) {
        let confirmed = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Reset Video Editor")
            .set_description("Reset all video editor settings to their defaults?")
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                "Reset".into(),
                "Cancel".into(),
            ))
            .show();
        if confirmed == rfd::MessageDialogResult::Custom("Reset".into()) {
            self.reset_state(cx);
        }
    }

    /// Builds the window without opening one, for the headless render tests.
    #[cfg(test)]
    pub fn new_for_test(path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        Self::new(path, cx)
    }

    /// Opens `tab` unconditionally. `select_tab` toggles, which would close the
    /// sidebar on a repeat and skip the panel the test means to draw.
    #[cfg(test)]
    pub fn open_tab_for_test(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.ui.sidebar_open = true;
        self.state.ui.sidebar_tab = tab.id().to_string();
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.ui.sidebar_open = !self.state.ui.sidebar_open;
        self.animate_sidebar(cx);
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.is_playing = !self.is_playing;
        if self.is_playing {
            if self.playhead >= self.total_duration() {
                self.playhead = 0.0;
            }
            self.scrub_audio_generation += 1;
            self.sync_audio_transport();
            self.drive_playback(cx);
        } else {
            self.preview_audio.pause_all();
            self.request_frame(cx);
        }
        cx.notify();
    }

    pub fn activate_tab(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        if !self.state.ui.sidebar_open {
            self.state.ui.sidebar_open = true;
            self.animate_sidebar(cx);
        }
        self.state.ui.sidebar_tab = tab.id().to_string();
        self.persist(cx);
        cx.notify();
    }

    fn animate_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_animation_generation += 1;
        self.sidebar_closing = !self.state.ui.sidebar_open;
        if !self.sidebar_closing {
            return;
        }
        let generation = self.sidebar_animation_generation;
        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(SIDEBAR_ANIMATION_DURATION)
                .await;
            let _ = entity.update(cx, |this, cx| {
                if this.sidebar_animation_generation != generation || this.state.ui.sidebar_open {
                    return;
                }
                this.sidebar_closing = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Moves the playhead to an absolute position — the timeline click and
    /// scrub path.
    pub fn set_playhead(&mut self, time: f64, cx: &mut Context<Self>) {
        let total = self.total_duration();
        self.playhead = time.clamp(0.0, total);
        self.preview_playhead = None;
        self.autoscroll_timeline();
        self.request_frame(cx);
        if self.is_playing {
            self.sync_audio_transport();
            self.drive_playback(cx);
        } else if self.state.ui.scrub_audio_enabled {
            self.scrub_audio_to(self.playhead - first_frame_duration(&self.state), cx);
        } else {
            self.preview_audio.stop_scrub();
        }
        cx.notify();
    }

    pub fn begin_scrub(&mut self) {
        self.is_scrubbing = true;
    }

    fn end_scrub(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.is_scrubbing) {
            return;
        }
        self.request_frame(cx);
    }

    /// Anchors the preview audio to the playhead. The stems are video-time,
    /// so the silent first-frame section maps to a negative offset.
    fn sync_audio_transport(&mut self) {
        let offset = self.playhead - first_frame_duration(&self.state);
        self.preview_audio.transport(self.is_playing, offset);
    }

    /// Scrub audio while paused: the program stems play from the scrub
    /// position and stop 120ms after the pointer goes quiet, the way the
    /// player's scrub loop does.
    fn scrub_audio_to(&mut self, offset: f64, cx: &mut Context<Self>) {
        if offset < 0.0 {
            self.preview_audio.stop_scrub();
            return;
        }
        self.preview_audio.scrub_to(offset);
        self.scrub_audio_generation += 1;
        let generation = self.scrub_audio_generation;
        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let _ = entity.update(cx, |this, _| {
                if this.scrub_audio_generation == generation {
                    this.preview_audio.stop_scrub();
                }
            });
        })
        .detach();
    }

    /// Applies volumes live and rebuilds the stems on the background executor
    /// when the structure changed. Called from every state mutation; the
    /// signature check keeps volume-only commits free.
    fn request_audio_sync(&mut self, cx: &mut Context<Self>) {
        self.preview_audio.apply_volumes(&self.state);
        let Some(project) = self.path.clone() else {
            return;
        };
        if !self.preview_audio.needs_rebuild(&self.state) {
            return;
        }
        self.audio_generation += 1;
        let generation = self.audio_generation;
        let inputs = crate::video::preview_audio::RebuildInputs::capture(
            &project,
            &self.state,
            self.preview_audio.cache(),
        );
        let task = cx
            .background_executor()
            .spawn(async move { crate::video::preview_audio::build_stems(inputs) });
        cx.spawn(async move |entity, cx| {
            let built = task.await;
            let _ = entity.update(cx, |this, _| {
                if this.audio_generation != generation {
                    return;
                }
                let offset = this.playhead - first_frame_duration(&this.state);
                this.preview_audio.install(built, &this.state, offset);
            });
        })
        .detach();
    }

    pub fn toggle_cut_tool(&mut self, cx: &mut Context<Self>) {
        self.is_cut_tool_active = !self.is_cut_tool_active;
        self.selected_clip = None;
        cx.notify();
    }

    pub fn set_scrub_audio(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.ui.scrub_audio_enabled = enabled;
        if !enabled {
            self.scrub_audio_generation += 1;
            self.preview_audio.stop_scrub();
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn select_clip(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.selected_clip = if self.selected_clip.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
        self.sync_drawing_text_field(cx);
        cx.notify();
    }

    fn selected_segment_index(&self) -> Option<usize> {
        let id = self.selected_clip.as_ref()?;
        self.state
            .segments
            .iter()
            .position(|segment| segment.id.as_str() == id.as_ref())
    }

    fn selected_clip_kind(&self) -> Option<TrackKind> {
        let id: &str = self.selected_clip.as_ref()?.as_ref();
        if self.state.segments.iter().any(|segment| segment.id == id) {
            return Some(TrackKind::Video);
        }
        if self
            .state
            .zoom_segments
            .iter()
            .any(|segment| segment.id == id)
        {
            return Some(TrackKind::Zoom);
        }
        if self
            .state
            .camera_segments
            .iter()
            .any(|segment| segment.id == id)
        {
            return Some(TrackKind::Camera);
        }
        if self
            .state
            .drawing_segments
            .iter()
            .any(|segment| segment.id == id)
        {
            return Some(TrackKind::Drawing);
        }
        self.state
            .music_tracks
            .iter()
            .any(|segment| segment.id == id)
            .then_some(TrackKind::Music)
    }

    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        let Some(kind) = self.selected_clip_kind() else {
            return;
        };
        let Some(id) = self.selected_clip.clone() else {
            return;
        };
        self.delete_clip(kind, id, cx);
    }

    fn reorder_selected_segment(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(index) = self.selected_segment_index() else {
            return;
        };
        self.move_segment(index, index as isize + delta, cx);
    }

    fn move_segment(&mut self, index: usize, next: isize, cx: &mut Context<Self>) {
        if next < 0 || next >= self.state.segments.len() as isize || next == index as isize {
            return;
        }
        self.commit(cx, move |state| {
            let segment = state.segments.remove(index);
            state.segments.insert(next as usize, segment);
        });
    }

    pub fn delete_selected_segment(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_segment_index() else {
            return;
        };
        if self.state.segments.len() <= 1 {
            return;
        }
        self.selected_clip = None;
        self.commit(cx, move |state| {
            state.segments.remove(index);
        });
    }

    pub fn set_selected_segment_speed(&mut self, speed: f64, cx: &mut Context<Self>) {
        let Some(index) = self.selected_segment_index() else {
            return;
        };
        self.commit(cx, move |state| {
            if let Some(segment) = state.segments.get_mut(index) {
                segment.speed = Some(speed);
            }
        });
    }

    /// The video time a timeline position maps to, used by the cut tool.
    fn video_time_at(&self, timeline_time: f64) -> f64 {
        let segments = timeline_segments::to_video_segments(&self.state.segments);
        timeline_segments::map_timeline_to_video_time(timeline_time, &segments)
            .or_else(|| segments.last().map(|segment| segment.end_time))
            .unwrap_or(timeline_time)
    }

    /// The cut tool. Without a track it splits every track at once, which is
    /// what clicking a lane does; with one it splits only that track, which is
    /// the shift-click and the context menu's own cut.
    pub fn cut_at(&mut self, time: f64, only: Option<TrackKind>, cx: &mut Context<Self>) {
        let video_time = self.video_time_at(time);
        self.commit(cx, |state| {
            match only {
                None => {
                    edit::split_all(state, time, video_time);
                }
                Some(TrackKind::Video) => {
                    edit::split_video(&mut state.segments, video_time);
                }
                Some(TrackKind::Zoom) => {
                    edit::split_ranges(&mut state.zoom_segments, time);
                }
                Some(TrackKind::Camera) => {
                    edit::split_ranges(&mut state.camera_segments, time);
                }
                Some(TrackKind::Drawing) => {
                    edit::split_ranges(&mut state.drawing_segments, time);
                }
                Some(TrackKind::Music) => {
                    edit::split_music(&mut state.music_tracks, time);
                }
            };
        });
        self.set_playhead(time, cx);
    }

    pub fn delete_clip(&mut self, kind: TrackKind, id: SharedString, cx: &mut Context<Self>) {
        if self.selected_clip.as_ref() == Some(&id) {
            self.selected_clip = None;
        }
        self.commit(cx, move |state| {
            match kind {
                TrackKind::Video => {
                    if state.segments.len() > 1 {
                        state.segments.retain(|segment| segment.id != id.as_ref());
                    }
                }
                TrackKind::Zoom => {
                    edit::remove_range(&mut state.zoom_segments, &id);
                }
                TrackKind::Camera => {
                    edit::remove_range(&mut state.camera_segments, &id);
                }
                TrackKind::Drawing => {
                    edit::remove_range(&mut state.drawing_segments, &id);
                }
                TrackKind::Music => {
                    edit::remove_range(&mut state.music_tracks, &id);
                }
            };
        });
    }

    /// "Delete Others" in the track context menus.
    pub fn delete_other_clips(
        &mut self,
        kind: TrackKind,
        id: SharedString,
        cx: &mut Context<Self>,
    ) {
        self.selected_clip = Some(id.clone());
        self.commit(cx, move |state| match kind {
            TrackKind::Video => state.segments.retain(|segment| segment.id == id.as_ref()),
            TrackKind::Zoom => state
                .zoom_segments
                .retain(|segment| segment.id == id.as_ref()),
            TrackKind::Camera => state
                .camera_segments
                .retain(|segment| segment.id == id.as_ref()),
            TrackKind::Drawing => state
                .drawing_segments
                .retain(|segment| segment.id == id.as_ref()),
            TrackKind::Music => state.music_tracks.retain(|track| track.id == id.as_ref()),
        });
    }

    /// The zoom segment the timeline selection points at, if any.
    pub fn selected_zoom_segment(&self) -> Option<&model::ZoomSegment> {
        let id = self.selected_clip.as_ref()?;
        self.state
            .zoom_segments
            .iter()
            .find(|segment| segment.id.as_str() == id.as_ref())
    }

    /// Applies a change to the selected zoom segment.
    pub fn update_selected_zoom(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut model::ZoomSegment),
    ) {
        let Some(id) = self.selected_clip.clone() else {
            return;
        };
        self.commit(cx, move |state| {
            if let Some(segment) = state
                .zoom_segments
                .iter_mut()
                .find(|segment| segment.id.as_str() == id.as_ref())
            {
                mutate(segment);
            }
        });
    }

    /// Switching a segment to manual framing seeds a centre focus point, the
    /// way `handleTargetModeChange` does.
    pub fn set_zoom_target_mode(&mut self, mode: String, cx: &mut Context<Self>) {
        let needs_focus = self
            .selected_zoom_segment()
            .is_some_and(|segment| segment.focus_point.is_none());
        self.update_selected_zoom(cx, move |segment| {
            if mode == "manual" && needs_focus {
                segment.focus_point = Some(model::FocusPoint { x: 0.5, y: 0.5 });
            }
            segment.target_mode = Some(mode);
        });
    }

    pub fn set_zoom_focus_point(&mut self, x: f64, y: f64, cx: &mut Context<Self>) {
        self.update_selected_zoom(cx, move |segment| {
            segment.focus_point = Some(model::FocusPoint {
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            });
        });
    }

    /// The composed frame under the playhead, which the focus picker draws.
    pub fn preview_image(&self) -> Option<Arc<gpui::RenderImage>> {
        self.preview_image.clone()
    }

    pub fn set_zoom_segment_level(&mut self, id: SharedString, level: f64, cx: &mut Context<Self>) {
        self.commit(cx, move |state| {
            if let Some(segment) = state
                .zoom_segments
                .iter_mut()
                .find(|segment| segment.id == id.as_ref())
            {
                segment.zoom_level = level;
            }
        });
    }

    /// "Apply zoom to All" — every zoom segment takes this one's level.
    pub fn apply_zoom_level_to_all(&mut self, id: SharedString, cx: &mut Context<Self>) {
        let Some(level) = self
            .state
            .zoom_segments
            .iter()
            .find(|segment| segment.id == id.as_ref())
            .map(|segment| segment.zoom_level)
        else {
            return;
        };
        self.commit(cx, move |state| {
            for segment in &mut state.zoom_segments {
                segment.zoom_level = level;
            }
        });
    }

    /// Generates zoom segments from the recorded pointer activity and merges
    /// them with the hand-placed ones.
    pub fn generate_auto_zoom(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let Some(cursor_data) = crate::video::sidecars::load_cursor(&path) else {
            crate::windows::toast::Toast::show(
                cx,
                "No cursor data",
                "This recording has no pointer track to zoom from.",
            );
            return;
        };
        let stamp = chrono::Utc::now().timestamp_millis();
        let generated = crate::video::auto_zoom::generate(&cursor_data, stamp);
        if generated.is_empty() {
            crate::windows::toast::Toast::show(
                cx,
                "Nothing to zoom",
                "No activity in this recording was worth a zoom.",
            );
            return;
        }
        self.commit(cx, move |state| {
            state.zoom_segments = crate::video::auto_zoom::merge(&state.zoom_segments, &generated);
        });
    }

    /// Adds a clip to a track, which is what dragging on an empty lane does.
    pub fn add_clip(&mut self, kind: TrackKind, start: f64, end: f64, cx: &mut Context<Self>) {
        let total = self.total_duration();
        let start = start.clamp(0.0, total);
        let end = end.clamp(
            start + edit::MIN_SPLIT_DURATION,
            total.max(start + edit::MIN_SPLIT_DURATION),
        );
        let id = format!("{}-{}", kind.id(), (start * 1000.0).round() as i64);
        let composition = self.composition_size.unwrap_or_default();
        self.selected_clip = Some(SharedString::from(id.clone()));
        self.commit(cx, move |state| match kind {
            TrackKind::Zoom => state.zoom_segments.push(model::ZoomSegment {
                id,
                start_time: start,
                end_time: end,
                zoom_level: 1.5,
                ..model::ZoomSegment::default()
            }),
            TrackKind::Camera => state.camera_segments.push(model::CameraSegment {
                id,
                start_time: start,
                end_time: end,
            }),
            TrackKind::Drawing => state.drawing_segments.push(model::DrawingSegment {
                id,
                start_time: start,
                end_time: end,
                canvas_width: composition.0,
                canvas_height: composition.1,
                annotations: Vec::new(),
            }),
            TrackKind::Video | TrackKind::Music => {}
        });
    }

    /// Whether this project has a transcript on disk.
    pub fn has_subtitles(&self) -> bool {
        self.subtitle_count > 0
    }

    pub fn subtitle_count(&self) -> usize {
        self.subtitle_count
    }

    pub fn is_transcribing(&self) -> bool {
        matches!(
            self.transcription,
            TranscriptionStatus::Downloading(_) | TranscriptionStatus::Generating(_)
        )
    }

    /// The generate button's label — the same `Downloading model (N%)` /
    /// `Generating (N%)` strings the Electron panel renders.
    pub fn transcription_label(&self) -> String {
        match &self.transcription {
            TranscriptionStatus::Downloading(percent) => {
                format!("Downloading model ({percent}%)")
            }
            TranscriptionStatus::Generating(percent) => format!("Generating ({percent}%)"),
            TranscriptionStatus::Idle | TranscriptionStatus::Failed(_) => {
                "Generate Subtitles".to_string()
            }
        }
    }

    pub fn transcription_error(&self) -> Option<&str> {
        match &self.transcription {
            TranscriptionStatus::Failed(error) => Some(error),
            _ => None,
        }
    }

    pub fn transcription_model(&self) -> &str {
        &self.transcription_model
    }

    pub fn set_transcription_model(&mut self, model: String, cx: &mut Context<Self>) {
        if self.is_transcribing() {
            return;
        }
        self.transcription_model = model;
        self.transcription = TranscriptionStatus::Idle;
        cx.notify();
    }

    fn refresh_subtitle_count(&mut self) {
        let subtitle = self
            .path
            .as_deref()
            .and_then(crate::video::sidecars::load_subtitle);
        self.subtitle_count = subtitle
            .as_ref()
            .map(|data| data.segments.len())
            .unwrap_or(0);
        self.subtitle_summary = subtitle.map(|data| SharedString::from(subtitle_summary(&data)));
        self.cursor_summary = Some(SharedString::from(
            match self
                .path
                .as_deref()
                .and_then(crate::video::sidecars::load_cursor)
            {
                Some(data) => format!(
                    "{} events, {:.1}s duration",
                    data.events.len(),
                    data.meta.duration
                ),
                None => "Edit or replace cursor movement data".to_string(),
            },
        ));
    }

    pub fn cursor_summary(&self) -> Option<SharedString> {
        self.cursor_summary.clone()
    }

    pub fn subtitle_summary(&self) -> Option<SharedString> {
        self.subtitle_summary.clone()
    }

    /// Downloads the model if it is missing, transcribes the recording and
    /// writes `subtitle.json`, then reloads the composition so the captions
    /// show in the preview. Progress streams through one ordered channel, so
    /// the button shows the same download/generate percents as Electron.
    pub fn generate_subtitles(&mut self, cx: &mut Context<Self>) {
        if self.is_transcribing() {
            return;
        }
        let Some(project) = self.path.clone() else {
            return;
        };
        if !crate::video::transcription::is_binary_available() {
            crate::windows::toast::Toast::show(
                cx,
                "Whisper not installed",
                "The transcription engine is not bundled with this build.",
            );
            return;
        }

        let model = self.transcription_model.clone();
        let prompt = self.prompt_field.read(cx).value().trim().to_string();
        self.transcription_prompt = prompt.clone();
        self.transcription = TranscriptionStatus::Generating(0);
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let (tx, rx) = smol::channel::unbounded::<TranscriptionEvent>();
            let work = cx.background_executor().spawn(async move {
                let download_tx = tx.clone();
                if let Err(error) = crate::video::transcription::download_model(&model, |percent| {
                    let _ = download_tx.try_send(TranscriptionEvent::Download(percent));
                }) {
                    let _ = tx.try_send(TranscriptionEvent::Finished(Err(error)));
                    return;
                }
                let generate_tx = tx.clone();
                let result = crate::video::transcription::transcribe(
                    &project,
                    &crate::video::transcription::Options {
                        model,
                        prompt: if prompt.is_empty() {
                            None
                        } else {
                            Some(prompt)
                        },
                        ..crate::video::transcription::Options::default()
                    },
                    |percent| {
                        let _ = generate_tx.try_send(TranscriptionEvent::Generate(percent));
                    },
                );
                let _ = tx.try_send(TranscriptionEvent::Finished(
                    result.map(|data| data.segments.len()),
                ));
            });
            while let Ok(event) = rx.recv().await {
                let finished = matches!(event, TranscriptionEvent::Finished(_));
                if let TranscriptionEvent::Finished(result) = &event {
                    let (title, body) = match result {
                        Ok(count) => ("Subtitles ready", format!("{count} segments transcribed.")),
                        Err(error) => ("Transcription failed", error.clone()),
                    };
                    cx.update(|cx| crate::windows::toast::Toast::show(cx, title, &body));
                }
                let applied = entity.update(cx, |this, cx| {
                    match event {
                        TranscriptionEvent::Download(percent) => {
                            this.transcription = TranscriptionStatus::Downloading(percent);
                        }
                        TranscriptionEvent::Generate(percent) => {
                            this.transcription = TranscriptionStatus::Generating(percent);
                        }
                        TranscriptionEvent::Finished(result) => {
                            this.transcription = match result {
                                Ok(_) => TranscriptionStatus::Idle,
                                Err(error) => TranscriptionStatus::Failed(error),
                            };
                            this.refresh_subtitle_count();
                            this.reload_preview(cx);
                        }
                    }
                    cx.notify();
                });
                if finished || applied.is_err() {
                    break;
                }
            }
            work.await;
        })
        .detach();
    }

    /// Imports a `.json` or `.srt` transcript.
    pub fn import_subtitles(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        let Some(source) = rfd::FileDialog::new()
            .set_title("Import Subtitle Data")
            .add_filter("Transcripts", &["json", "srt"])
            .pick_file()
        else {
            return;
        };
        match crate::video::transcription::import_from(&project, &source) {
            Ok(data) => {
                crate::windows::toast::Toast::show(
                    cx,
                    "Subtitles imported",
                    format!("{} segments loaded.", data.segments.len()),
                );
                self.refresh_subtitle_count();
                self.reload_preview(cx);
            }
            Err(error) => {
                crate::windows::toast::Toast::show(cx, "Import failed", &error);
            }
        }
        cx.notify();
    }

    pub fn delete_subtitles(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        crate::video::transcription::delete(&project);
        self.refresh_subtitle_count();
        self.reload_preview(cx);
        cx.notify();
    }

    /// Rebuilds the composition source, which is how a newly written sidecar
    /// reaches the preview.
    fn reload_preview(&mut self, cx: &mut Context<Self>) {
        self.source = None;
        self.load_preview(cx);
    }

    /// Copies a picked audio file into the project and lays it on the music
    /// track at the playhead.
    pub fn add_music_track(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        let Some(source) = crate::video::music::pick_file() else {
            return;
        };
        match crate::video::music::add(&project, &source, self.playhead) {
            Ok(track) => {
                self.selected_clip = Some(SharedString::from(track.id.clone()));
                self.commit(cx, move |state| state.music_tracks.push(track));
            }
            Err(error) => {
                crate::windows::toast::Toast::show(cx, "Could not add music", &error);
            }
        }
    }

    /// Removes a music track, deleting its file once nothing else uses it.
    pub fn remove_music_track(&mut self, id: SharedString, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        let Some(file_name) = self
            .state
            .music_tracks
            .iter()
            .find(|track| track.id.as_str() == id.as_ref())
            .map(|track| track.file_name.clone())
        else {
            return;
        };

        self.commit(cx, {
            let id = id.clone();
            move |state| {
                state
                    .music_tracks
                    .retain(|track| track.id.as_str() != id.as_ref());
            }
        });

        let still_referenced = self
            .state
            .music_tracks
            .iter()
            .any(|track| track.file_name == file_name);
        crate::video::music::remove(&project, &file_name, still_referenced);
    }

    pub fn set_music_volume(&mut self, id: SharedString, volume: f64, cx: &mut Context<Self>) {
        self.commit(cx, move |state| {
            if let Some(track) = state
                .music_tracks
                .iter_mut()
                .find(|track| track.id.as_str() == id.as_ref())
            {
                track.volume = volume.clamp(0.0, 1.0);
            }
        });
    }

    pub fn set_music_speed(&mut self, id: SharedString, speed: f64, cx: &mut Context<Self>) {
        self.commit(cx, move |state| {
            if let Some(track) = state
                .music_tracks
                .iter_mut()
                .find(|track| track.id.as_str() == id.as_ref())
            {
                track.speed = speed.max(0.01);
            }
        });
    }

    pub fn set_music_enabled(&mut self, id: SharedString, enabled: bool, cx: &mut Context<Self>) {
        self.commit(cx, move |state| {
            if let Some(track) = state
                .music_tracks
                .iter_mut()
                .find(|track| track.id.as_str() == id.as_ref())
            {
                track.enabled = enabled;
            }
        });
    }

    pub fn pick_custom_cursor(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = crate::editor::background::pick_image() {
            self.update_cursor(cx, move |style| style.custom_cursor_image = Some(path));
        }
    }

    pub fn clear_custom_cursor(&mut self, cx: &mut Context<Self>) {
        self.update_cursor(cx, |style| style.custom_cursor_image = None);
    }

    pub fn pick_first_frame(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = crate::editor::background::pick_image() {
            self.update_first_frame(cx, move |settings| {
                settings.image_data = Some(path);
                settings.enabled = true;
            });
        }
    }

    pub fn clear_first_frame(&mut self, cx: &mut Context<Self>) {
        self.update_first_frame(cx, |settings| {
            settings.image_data = None;
            settings.enabled = false;
        });
    }

    pub fn import_cursor_data(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        let Some(source) = rfd::FileDialog::new()
            .set_title("Import Cursor Data")
            .add_filter("Cursor data", &["json"])
            .pick_file()
        else {
            return;
        };
        let dest = crate::video::project::cursor_path(&project);
        match std::fs::copy(&source, &dest) {
            Ok(_) => {
                crate::windows::toast::Toast::show(
                    cx,
                    "Cursor data imported",
                    "The cursor track was replaced.",
                );
                self.reload_preview(cx);
            }
            Err(error) => {
                crate::windows::toast::Toast::show(cx, "Import failed", error.to_string());
            }
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn toggle_music_track(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.commit(cx, move |state| {
            if let Some(track) = state
                .music_tracks
                .iter_mut()
                .find(|track| track.id.as_str() == id.as_ref())
            {
                track.enabled = !track.enabled;
            }
        });
    }

    /// Opens the JSON editor for one of the project's sidecars.
    pub fn open_data_editor(&mut self, kind: data_editor::DataKind, cx: &mut Context<Self>) {
        let Some(project) = self.path.clone() else {
            return;
        };
        let (width, height) = self
            .source
            .as_ref()
            .map(|source| {
                let info = source.lock().info();
                (info.width as f64, info.height as f64)
            })
            .unwrap_or((1920.0, 1080.0));
        let duration = self.total_duration();
        self.data_editor = Some(data_editor::DataEditor::open(
            kind, &project, width, height, duration, cx,
        ));
        cx.notify();
    }

    pub fn close_data_editor(&mut self, cx: &mut Context<Self>) {
        self.data_editor = None;
        cx.notify();
    }

    pub fn load_data_editor_template(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.data_editor.as_ref() else {
            return;
        };
        let (width, height) = self
            .source
            .as_ref()
            .map(|source| {
                let info = source.lock().info();
                (info.width as f64, info.height as f64)
            })
            .unwrap_or((1920.0, 1080.0));
        let value = editor.kind.template(width, height, self.total_duration());
        let field = editor.field.clone();
        field.update(cx, |field, cx| {
            field.set_value(value);
            cx.notify();
        });
        if let Some(editor) = self.data_editor.as_mut() {
            editor.error = None;
        }
        cx.notify();
    }

    pub fn load_data_editor_example(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.data_editor.as_ref() else {
            return;
        };
        let value = editor.kind.example();
        let field = editor.field.clone();
        field.update(cx, |field, cx| {
            field.set_value(value);
            cx.notify();
        });
        if let Some(editor) = self.data_editor.as_mut() {
            editor.error = None;
        }
        cx.notify();
    }

    /// Validates the document and writes it to the sidecar, then reloads the
    /// composition so the change is visible in the preview.
    pub fn save_data_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.data_editor.as_ref() else {
            return;
        };
        let Some(project) = self.path.clone() else {
            return;
        };
        let kind = editor.kind;
        let value = editor.field.read(cx).value().to_string();

        match kind.validate(&value) {
            Ok(normalized) => {
                let path = kind.path(&project);
                if let Err(error) = std::fs::write(&path, normalized) {
                    if let Some(editor) = self.data_editor.as_mut() {
                        editor.error = Some(SharedString::from(format!("Could not save: {error}")));
                    }
                    cx.notify();
                    return;
                }
                self.data_editor = None;
                self.refresh_subtitle_count();
                self.reload_preview(cx);
            }
            Err(error) => {
                if let Some(editor) = self.data_editor.as_mut() {
                    editor.error = Some(SharedString::from(error));
                }
            }
        }
        cx.notify();
    }

    pub fn begin_clip_drag(&mut self, drag: ClipDrag, cx: &mut Context<Self>) {
        self.selected_clip = Some(drag.id.clone());
        self.clip_drag = Some(drag);
        self.begin_gesture();
        cx.notify();
    }

    /// Applies the pointer position to the clip being dragged. History is not
    /// touched until the gesture ends.
    pub fn update_clip_drag(&mut self, time: f64, cx: &mut Context<Self>) {
        let Some(drag) = self.clip_drag.clone() else {
            return;
        };
        let total = self.total_duration();
        let id = drag.id.to_string();
        let state = &mut self.state;

        macro_rules! apply {
            ($items:expr) => {{
                match drag.mode {
                    DragMode::Move => {
                        edit::move_range($items, &id, time - drag.grab_offset, total);
                    }
                    DragMode::ResizeStart => {
                        let end = $items
                            .iter()
                            .find(|item| edit::TimelineRange::id(*item) == id)
                            .map(edit::TimelineRange::end)
                            .unwrap_or(time);
                        edit::resize_range($items, &id, time, end, total);
                    }
                    DragMode::ResizeEnd => {
                        let start = $items
                            .iter()
                            .find(|item| edit::TimelineRange::id(*item) == id)
                            .map(edit::TimelineRange::start)
                            .unwrap_or(time);
                        edit::resize_range($items, &id, start, time, total);
                    }
                }
            }};
        }

        match drag.kind {
            TrackKind::Zoom => apply!(&mut state.zoom_segments[..]),
            TrackKind::Camera => apply!(&mut state.camera_segments[..]),
            TrackKind::Drawing => apply!(&mut state.drawing_segments[..]),
            TrackKind::Music => apply!(&mut state.music_tracks[..]),
            TrackKind::Video => match drag.mode {
                DragMode::ResizeStart => {
                    edit::trim_video(&mut state.segments, &id, time, true);
                }
                DragMode::ResizeEnd => {
                    edit::trim_video(&mut state.segments, &id, time, false);
                }
                DragMode::Move => {}
            },
        }
        self.sync_preview_state(cx);
        cx.notify();
    }

    pub fn begin_reorder(&mut self, id: SharedString, x: f32, cx: &mut Context<Self>) {
        self.reorder_drag = Some(ReorderDrag {
            drop_index: self
                .state
                .segments
                .iter()
                .position(|segment| segment.id.as_str() == id.as_ref())
                .unwrap_or_default(),
            id,
            origin_x: x,
            dragging: false,
        });
        cx.notify();
    }

    pub fn begin_draw(&mut self, kind: TrackKind, time: f64, cx: &mut Context<Self>) {
        self.selected_clip = None;
        self.draw_drag = Some(DrawDrag {
            kind,
            start: time,
            end: time,
        });
        cx.notify();
    }

    pub fn update_timeline_drag(&mut self, time: f64, x: f32, cx: &mut Context<Self>) -> bool {
        if self.clip_drag.is_some() {
            self.update_clip_drag(time, cx);
            return true;
        }
        if let Some(drag) = self.draw_drag.as_mut() {
            drag.end = time;
            cx.notify();
            return true;
        }
        let Some(drag) = self.reorder_drag.as_ref() else {
            return false;
        };
        if !drag.dragging && (x - drag.origin_x).abs() < timeline::reorder::DRAG_THRESHOLD {
            return true;
        }
        let durations: Vec<f64> = self
            .state
            .segments
            .iter()
            .map(model::Segment::timeline_duration)
            .collect();
        let dragged = self
            .state
            .segments
            .iter()
            .position(|segment| segment.id.as_str() == drag.id.as_ref())
            .unwrap_or_default();
        let drop_index = timeline::reorder::drop_index(&durations, dragged, time);
        if let Some(drag) = self.reorder_drag.as_mut() {
            drag.dragging = true;
            drag.drop_index = drop_index;
        }
        cx.notify();
        true
    }

    pub fn end_clip_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.draw_drag.take() {
            let total = self.total_duration();
            if let Some((start, end)) = edit::drawn_range(drag.start, drag.end, total) {
                self.add_clip(drag.kind, start, end, cx);
            }
            cx.notify();
            return;
        }
        if let Some(drag) = self.reorder_drag.take() {
            if drag.dragging {
                let index = self
                    .state
                    .segments
                    .iter()
                    .position(|segment| segment.id.as_str() == drag.id.as_ref());
                if let Some(index) = index {
                    self.move_segment(index, drag.drop_index as isize, cx);
                }
            }
            cx.notify();
            return;
        }
        let Some(drag) = self.clip_drag.take() else {
            return;
        };
        if drag.kind == TrackKind::Video {
            if let Some(index) = self
                .state
                .segments
                .iter()
                .position(|segment| segment.id.as_str() == drag.id.as_ref())
            {
                let start: f64 = self.state.segments[..index]
                    .iter()
                    .map(|segment| segment.timeline_duration())
                    .sum();
                self.set_playhead(start, cx);
            }
        }
        self.end_gesture(cx);
    }

    pub fn is_dragging_clip(&self) -> bool {
        self.clip_drag.is_some() || self.reorder_drag.is_some() || self.draw_drag.is_some()
    }

    pub fn preview_seek(&mut self, time: Option<f64>, cx: &mut Context<Self>) {
        if time.is_some() && (self.is_playing || self.is_scrubbing || self.is_dragging_clip()) {
            return;
        }
        let time = time.map(|time| time.clamp(0.0, self.total_duration()));
        if self.preview_playhead == time {
            return;
        }
        self.preview_playhead = time;
        self.request_frame(cx);
        cx.notify();
    }

    fn set_timeline_scroll(&self, left: f32) {
        let left = px(-left);
        self.tracks_scroll
            .set_offset(gpui::point(left, self.tracks_scroll.offset().y));
        self.mirror_timeline_scroll();
    }

    fn mirror_timeline_scroll(&self) {
        self.ruler_scroll.set_offset(gpui::point(
            self.tracks_scroll.offset().x,
            self.ruler_scroll.offset().y,
        ));
    }

    fn timeline_scroll_left(&self) -> f32 {
        -f32::from(self.tracks_scroll.offset().x)
    }

    pub fn zoom_timeline_at(&mut self, delta_y: f32, pointer: f32, cx: &mut Context<Self>) {
        let (next, left) = timeline::wheel_zoom(
            self.pixels_per_second,
            delta_y,
            pointer,
            self.timeline_scroll_left(),
        );
        self.set_timeline_zoom(next, cx);
        self.set_timeline_scroll(left);
    }

    pub fn scroll_timeline_by(&mut self, delta: f32, content_width: f32, cx: &mut Context<Self>) {
        let viewport = f32::from(self.tracks_scroll.bounds().size.width);
        let left = timeline::clamp_scroll_left(
            self.timeline_scroll_left() + delta,
            content_width,
            viewport,
        );
        self.set_timeline_scroll(left);
        cx.notify();
    }

    fn autoscroll_timeline(&self) {
        let viewport = f32::from(self.tracks_scroll.bounds().size.width);
        let playhead = self.playhead as f32 * self.pixels_per_second;
        let Some(left) = timeline::autoscroll_left(playhead, self.timeline_scroll_left(), viewport)
        else {
            return;
        };
        self.set_timeline_scroll(left);
    }

    pub fn begin_timeline_resize(&mut self, y: f32, cx: &mut Context<Self>) {
        self.timeline_resize = Some((y, self.timeline_height));
        cx.notify();
    }

    pub fn update_timeline_resize(&mut self, y: f32, cx: &mut Context<Self>) {
        let Some((origin, height)) = self.timeline_resize else {
            return;
        };
        self.timeline_height = timeline::clamp_pane_height(height + (origin - y));
        cx.notify();
    }

    pub fn end_timeline_resize(&mut self, cx: &mut Context<Self>) {
        if self.timeline_resize.take().is_none() {
            return;
        }
        self.state.ui.timeline_height = Some(self.timeline_height as f64);
        self.persist(cx);
        cx.notify();
    }

    pub fn is_resizing_timeline(&self) -> bool {
        self.timeline_resize.is_some()
    }

    pub fn set_music_group_speed(
        &mut self,
        group: SharedString,
        speed: f64,
        cx: &mut Context<Self>,
    ) {
        self.commit(cx, move |state| {
            for track in state
                .music_tracks
                .iter_mut()
                .filter(|track| track.group_id == group.as_ref())
            {
                track.speed = speed;
            }
        });
    }

    pub fn remove_music_group(&mut self, group: SharedString, cx: &mut Context<Self>) {
        self.selected_clip = None;
        let ids: Vec<SharedString> = self
            .state
            .music_tracks
            .iter()
            .filter(|track| model::group_key(track) == group.as_ref())
            .map(|track| SharedString::from(track.id.clone()))
            .collect();
        for id in ids {
            self.remove_music_track(id, cx);
        }
    }

    pub fn step_selected_segment_speed(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(index) = self.selected_segment_index() else {
            return;
        };
        let current = self
            .state
            .segments
            .get(index)
            .and_then(|segment| segment.speed)
            .unwrap_or(1.0);
        self.set_selected_segment_speed(timeline::step_speed(current, delta), cx);
    }

    fn display_duration(&self) -> f64 {
        self.total_duration()
            .max(model::music_display_end(&self.state.music_tracks))
    }

    pub fn set_timeline_zoom(&mut self, pixels_per_second: f32, cx: &mut Context<Self>) {
        self.pixels_per_second = timeline::clamp_zoom(pixels_per_second);
        self.state.timeline_zoom = Some(self.pixels_per_second as f64);
        self.persist(cx);
        cx.notify();
    }

    pub fn zoom_timeline_in(&mut self, cx: &mut Context<Self>) {
        let next = self.pixels_per_second * timeline::ZOOM_STEP;
        self.set_timeline_zoom(next, cx);
    }

    pub fn zoom_timeline_out(&mut self, cx: &mut Context<Self>) {
        let next = self.pixels_per_second / timeline::ZOOM_STEP;
        self.set_timeline_zoom(next, cx);
    }

    pub fn fit_timeline_to_view(&mut self, cx: &mut Context<Self>) {
        let width = f32::from(self.tracks_scroll.bounds().size.width);
        let next = timeline::fit_zoom(self.total_duration(), width);
        self.set_timeline_zoom(next, cx);
    }

    pub fn reveal_project(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.path {
            desktop::reveal_in_file_manager(path);
        }
        cx.notify();
    }

    pub fn toggle_project_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.path.as_deref() {
            let name = model::project_display_name(path);
            self.rename_field.update(cx, |field, cx| {
                field.set_value(&name);
                cx.notify();
            });
        }
        self.rename_error = None;
        self.path_copied_at = None;
        self.open_title_popover(title_bar::PROJECT_POPOVER_ID, window, cx);
    }

    pub fn toggle_export_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_title_popover(title_bar::EXPORT_POPOVER_ID, window, cx);
    }

    fn open_title_popover(
        &mut self,
        owner: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        let theme = active_theme(cx);
        self.menu.toggle_with(
            crate::ui::menu::MenuPlacement::below(owner).aligned_right(),
            move |_dismiss, cx| {
                let view = cx.new(move |_cx| TitlePopover {
                    owner,
                    editor: entity,
                    theme,
                });
                (view.into(), None)
            },
            window,
            cx,
        );
        cx.notify();
    }

    pub fn copy_project_path(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        crate::system::clipboard::ClipboardService::write_text(
            cx,
            path.to_string_lossy().to_string(),
        );
        self.path_copied_at = Some(cx.background_executor().now());
        cx.notify();
    }

    pub fn path_recently_copied(&self, cx: &App) -> bool {
        let now = cx.background_executor().now();
        self.path_copied_at.is_some_and(|at| {
            now.saturating_duration_since(at) < Duration::from_millis(COPY_FEEDBACK_MS)
        })
    }

    pub fn rename_project(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(old_path) = self.path.clone() else {
            return;
        };
        if value.trim().is_empty() || model::project_display_name(&old_path) == value.trim() {
            return;
        }
        self.source = None;
        let new_project = match project::rename_project(&old_path, value) {
            Ok(path) => path,
            Err(error) => {
                self.load_preview(cx);
                self.rename_error = Some(SharedString::from(error.to_string()));
                cx.notify();
                return;
            }
        };
        let old_project = project::project_folder(&old_path).unwrap_or_else(|| old_path.clone());
        let new_path = if old_path == old_project {
            new_project.clone()
        } else {
            new_project.join(old_path.file_name().unwrap_or_default())
        };
        if let Some(background) = self.state.wallpaper.background_image.as_mut() {
            *background = background.replace(
                old_project.to_string_lossy().as_ref(),
                new_project.to_string_lossy().as_ref(),
            );
        }
        crate::thumbnails::rekey(&old_path, &new_path);
        crate::history_store::update_item_path(&old_path, &new_path);
        self.path = Some(new_path);
        self.rename_error = None;
        self.menu.close(window, cx);
        self.persist(cx);
        self.load_preview(cx);
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub fn cancel_export(&mut self, cx: &mut Context<Self>) {
        self.export_cancelled.store(true, Ordering::Relaxed);
        self.is_exporting = false;
        self.export_progress = 0.0;
        self.displayed_progress = 0.0;
        self.export_started_at = None;
        self.export_remaining = None;
        self.export_error = None;
        self.export_completed_at = None;
        cx.notify();
    }

    fn schedule_export_completion_clear(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(EXPORT_COMPLETION_MS))
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.export_completed_at = None;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn is_export_complete(&self, cx: &App) -> bool {
        let now = cx.background_executor().now();
        self.export_completed_at.is_some_and(|at| {
            now.saturating_duration_since(at) < Duration::from_millis(EXPORT_COMPLETION_MS)
        })
    }

    pub fn export_error(&self) -> Option<SharedString> {
        self.export_error.clone()
    }

    pub fn url_recently_copied(&self, cx: &App) -> bool {
        let now = cx.background_executor().now();
        self.url_copied_at.is_some_and(|at| {
            now.saturating_duration_since(at) < Duration::from_millis(COPY_FEEDBACK_MS)
        })
    }

    pub fn set_upload_to_cloud(&mut self, value: bool, cx: &mut Context<Self>) {
        if value && !crate::cloud::is_configured(&crate::state::state(cx).config.get().cloud) {
            return;
        }
        self.upload_to_cloud = value;
        cx.notify();
    }

    pub fn cancel_cloud_upload(&mut self, cx: &mut Context<Self>) {
        self.cloud_upload = crate::cloud::UploadState::Idle;
        cx.notify();
    }

    pub fn copy_uploaded_url(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.uploaded_url.clone() else {
            return;
        };
        crate::system::clipboard::ClipboardService::write_text(cx, url);
        self.url_copied_at = Some(cx.background_executor().now());
        cx.notify();
    }

    pub fn open_uploaded_url(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = &self.uploaded_url {
            crate::system::desktop::open_url(url);
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn set_transcription_prompt(&mut self, value: String, cx: &mut Context<Self>) {
        self.transcription_prompt = value;
        cx.notify();
    }

    fn next_drawing_id(&mut self, prefix: &str) -> String {
        self.drawing_id_seed += 1;
        format!("{prefix}-{}", self.drawing_id_seed)
    }

    fn preview_content_rect(&self) -> Option<drawing_overlay::Rect> {
        let composition = self.composition_size?;
        let bounds = (*self.preview_bounds.borrow())?;
        drawing_overlay::contain_rect(drawing_overlay::Rect::from_bounds(bounds), composition)
    }

    fn composition_point(
        &self,
        position: gpui::Point<gpui::Pixels>,
        inside_only: bool,
    ) -> Option<crate::editor::annotations::Point> {
        let composition = self.composition_size?;
        let content = self.preview_content_rect()?;
        let (x, y) = (f32::from(position.x), f32::from(position.y));
        if inside_only && !content.contains(x, y) {
            return None;
        }
        drawing_overlay::to_composition((x, y), content, composition)
    }

    pub fn begin_drawing_stroke(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(point) = self.composition_point(position, true) else {
            return;
        };
        let tools = self.drawing_tools.clone();
        let id = self.next_drawing_id("annotation");
        if matches!(tools.active_tool.as_str(), "text" | "number") {
            let value = drawing_overlay::next_number_value(&self.state, &tools);
            let Some(annotation) = drawing_overlay::place(&tools, point, id, value) else {
                return;
            };
            let is_text = tools.active_tool == "text";
            self.add_drawing_annotation(annotation, cx);
            if is_text {
                let focus = self.drawing_text_field.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            }
            return;
        }
        self.drawing_stroke = drawing_overlay::Stroke::begin(&tools, point, id);
        cx.notify();
    }

    fn update_drawing_stroke(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        if self.drawing_stroke.is_none() {
            return;
        }
        let Some(point) = self.composition_point(position, false) else {
            return;
        };
        let tools = self.drawing_tools.clone();
        if let Some(stroke) = self.drawing_stroke.as_mut() {
            stroke.update(&tools, point, shift);
        }
        cx.notify();
    }

    fn end_drawing_stroke(&mut self, cx: &mut Context<Self>) {
        let Some(stroke) = self.drawing_stroke.take() else {
            return;
        };
        let Some(annotation) = stroke.finish() else {
            cx.notify();
            return;
        };
        self.add_drawing_annotation(annotation, cx);
    }

    fn add_drawing_annotation(&mut self, annotation: Annotation, cx: &mut Context<Self>) {
        let Some(composition) = self.composition_size else {
            return;
        };
        let total = self.total_duration();
        let position = self.playhead;
        let segment_id = self.next_drawing_id("drawing");
        let selected = self.selected_clip.as_ref().map(|id| id.to_string());
        let keeps_tool = matches!(annotation.kind(), "pen" | "highlight");
        let mut assigned = None;
        self.commit(cx, |state| {
            assigned = drawing_overlay::attach(
                state,
                selected.as_deref(),
                annotation,
                position,
                total,
                composition,
                &segment_id,
            );
        });
        if let Some(id) = assigned {
            self.selected_clip = Some(SharedString::from(id));
        }
        if !keeps_tool {
            self.drawing_tools.active_tool = "select".to_string();
        }
        self.sync_drawing_text_field(cx);
        cx.notify();
    }

    pub fn selected_drawing_segment(&self) -> Option<&model::DrawingSegment> {
        let id = self.selected_clip.as_ref()?;
        self.state
            .drawing_segments
            .iter()
            .find(|segment| segment.id.as_str() == id.as_ref())
    }

    pub fn update_selected_annotation(&mut self, option: EditorOption, cx: &mut Context<Self>) {
        let Some(id) = self.selected_clip.clone() else {
            return;
        };
        self.commit(cx, move |state| {
            let Some(segment) = state
                .drawing_segments
                .iter_mut()
                .find(|segment| segment.id.as_str() == id.as_ref())
            else {
                return;
            };
            for annotation in segment.annotations.iter_mut() {
                drawing_overlay::apply_option(annotation, &option);
            }
        });
    }

    pub fn set_selected_annotation_text(&mut self, value: String, cx: &mut Context<Self>) {
        let Some(id) = self.selected_clip.clone() else {
            return;
        };
        self.commit(cx, move |state| {
            let Some(segment) = state
                .drawing_segments
                .iter_mut()
                .find(|segment| segment.id.as_str() == id.as_ref())
            else {
                return;
            };
            for annotation in segment.annotations.iter_mut() {
                if let Annotation::Text { text, .. } = annotation {
                    *text = value.clone();
                }
            }
        });
    }

    fn sync_drawing_text_field(&mut self, cx: &mut Context<Self>) {
        let text = self
            .selected_drawing_segment()
            .and_then(|segment| segment.annotations.first())
            .and_then(|annotation| match annotation {
                Annotation::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let field = self.drawing_text_field.clone();
        if field.read(cx).value() == text {
            return;
        }
        field.update(cx, |field, cx| {
            field.set_value(&text);
            cx.notify();
        });
    }

    pub fn update_drawing_tools(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::DrawingToolSettings),
    ) {
        mutate(&mut self.drawing_tools);
        cx.notify();
    }

    pub fn delete_selected_drawing(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_clip.clone() else {
            return;
        };
        self.commit(cx, move |state| {
            state
                .drawing_segments
                .retain(|segment| segment.id.as_str() != id.as_ref());
        });
        self.selected_clip = None;
    }

    pub fn set_sidebar_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.sidebar_width = width.clamp(
            crate::ui::chrome::VIDEO_SIDEBAR_MIN,
            crate::ui::chrome::VIDEO_SIDEBAR_MAX,
        );
        cx.notify();
    }

    pub fn begin_sidebar_resize(&mut self, x: f32, cx: &mut Context<Self>) {
        self.sidebar_resize = Some((x, self.sidebar_width));
        cx.notify();
    }

    pub fn update_sidebar_resize(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some((origin, start)) = self.sidebar_resize else {
            return;
        };
        self.set_sidebar_width(start + (origin - x), cx);
    }

    pub fn end_sidebar_resize(&mut self, cx: &mut Context<Self>) {
        self.sidebar_resize = None;
        cx.notify();
    }

    pub fn play_keyboard_demo(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = &self.keyboard_demo {
            flag.store(true, Ordering::Relaxed);
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.keyboard_demo = Some(stop.clone());
        let kind = self.state.audio_style.keyboard_sound_type.clone();
        std::thread::spawn(move || play_keyboard_demo_loop(&kind, stop));
        cx.notify();
    }

    pub fn stop_keyboard_demo(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = self.keyboard_demo.take() {
            flag.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    pub fn is_keyboard_demo_playing(&self) -> bool {
        self.keyboard_demo.is_some()
    }

    pub fn delete_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let confirmed = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Delete Video")
            .set_description(format!(
                "Delete {}? This removes the recording and all of its assets.",
                model::project_display_name(&path)
            ))
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                "Delete".into(),
                "Cancel".into(),
            ))
            .show();
        if confirmed != rfd::MessageDialogResult::Custom("Delete".into()) {
            return;
        }

        if !crate::history_store::delete_path(&path, crate::history_store::HistoryItemType::Video) {
            crate::windows::toast::Toast::show(
                cx,
                "Delete failed",
                "The recording could not be deleted",
            );
            return;
        }
        let notify = crate::state::state(cx)
            .config
            .get()
            .general
            .show_deletion_notifications;
        window.remove_window();
        registry::close(RegistryKind::VideoEditor, cx);
        if notify {
            cx.defer(|cx| {
                crate::windows::toast::Toast::show(
                    cx,
                    "Video deleted",
                    "The video has been permanently deleted",
                );
            });
        }
    }

    pub fn update_cursor(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::CursorStyle),
    ) {
        self.commit(cx, |state| mutate(&mut state.cursor_style));
    }

    pub fn update_camera(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::CameraStyle),
    ) {
        self.commit(cx, |state| mutate(&mut state.camera_style));
    }

    pub fn update_audio(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::AudioStyle),
    ) {
        self.commit(cx, |state| mutate(&mut state.audio_style));
    }

    pub fn update_keyboard(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::KeyboardStyle),
    ) {
        self.commit(cx, |state| mutate(&mut state.keyboard_style));
    }

    pub fn update_subtitle(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::SubtitleStyle),
    ) {
        self.commit(cx, |state| mutate(&mut state.subtitle_style));
    }

    pub fn update_wallpaper(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::VideoWallpaperSettings),
    ) {
        self.commit(cx, |state| mutate(&mut state.wallpaper));
    }

    pub fn update_first_frame(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::FirstFrameSettings),
    ) {
        self.commit(cx, |state| mutate(&mut state.first_frame));
    }

    pub fn update_export(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::ExportSettings),
    ) {
        let before = self.state.export_settings.clone();
        mutate(&mut self.state.export_settings);
        if before.format != self.state.export_settings.format {
            styles::apply_format_defaults(&mut self.state.export_settings);
        }
        styles::normalize_export_settings(&mut self.state.export_settings);
        if self.state.export_settings == before {
            return;
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn update_zoom_settings(
        &mut self,
        cx: &mut Context<Self>,
        mutate: impl FnOnce(&mut styles::ZoomSettings),
    ) {
        self.commit(cx, |state| mutate(&mut state.zoom_settings));
    }

    fn seek(&mut self, delta: f64, cx: &mut Context<Self>) {
        self.set_playhead(self.playhead + delta, cx);
    }

    fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_text(window, cx) {
            window.blur(cx);
        }
        self.selected_clip = None;
        self.clip_drag = None;
        self.reorder_drag = None;
        self.draw_drag = None;
        self.sync_drawing_text_field(cx);
        cx.notify();
    }

    fn editing_text(&self, window: &Window, cx: &App) -> bool {
        [
            &self.rename_field,
            &self.prompt_field,
            &self.drawing_text_field,
        ]
        .into_iter()
        .chain(self.data_editor.as_ref().map(|editor| &editor.field))
        .any(|field| field.read(cx).focus_handle(cx).is_focused(window))
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.data_editor.is_some() && event.keystroke.key.as_str() == "escape" {
            self.close_data_editor(cx);
            cx.stop_propagation();
            return;
        }
        if event.keystroke.key.as_str() == "escape" {
            if self.menu.is_present(cx) {
                cx.stop_propagation();
                return;
            }
            self.clear_selection(window, cx);
            cx.stop_propagation();
            return;
        }
        if self.editing_text(window, cx) {
            return;
        }

        let modifiers = event.keystroke.modifiers;
        let primary = modifiers.control || modifiers.platform;
        let shortcuts = crate::state::state(cx).config.get().shortcuts;
        let key = event.keystroke.key.as_str();

        if primary {
            match key {
                "backspace" => self.delete_recording(window, cx),
                "s" => self.activate_tab(SidebarTab::Export, cx),
                "z" if modifiers.shift => self.redo(cx),
                "z" => self.undo(cx),
                "=" | "+" => self.zoom_timeline_in(cx),
                "-" => self.zoom_timeline_out(cx),
                "0" => self.set_timeline_zoom(timeline::DEFAULT_PIXELS_PER_SECOND, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }

        if modifiers.alt {
            match key {
                "left" => self.reorder_selected_segment(-1, cx),
                "right" => self.reorder_selected_segment(1, cx),
                _ => return,
            }
            cx.stop_propagation();
            return;
        }

        let handled = match key {
            "space" => {
                self.toggle_playback(cx);
                true
            }
            "c" => {
                self.toggle_cut_tool(cx);
                true
            }
            "f" => {
                self.fit_timeline_to_view(cx);
                true
            }
            "left" => {
                self.seek(if modifiers.shift { -5.0 } else { -1.0 }, cx);
                true
            }
            "right" => {
                self.seek(if modifiers.shift { 5.0 } else { 1.0 }, cx);
                true
            }
            "home" => {
                self.set_playhead(0.0, cx);
                true
            }
            "end" => {
                self.set_playhead((self.total_duration() - LAST_FRAME_EPSILON).max(0.0), cx);
                true
            }
            "backspace" | "delete" => {
                self.delete_selection(cx);
                true
            }
            "," => {
                self.seek(-FRAME_STEP, cx);
                true
            }
            "." => {
                self.seek(FRAME_STEP, cx);
                true
            }
            _ => false,
        };

        let tab = if modifiers.shift {
            None
        } else {
            SidebarTab::ALL.into_iter().find(|tab| {
                let shortcut = tab.shortcut(&shortcuts.video_editor_sidebar);
                !shortcut.is_empty() && shortcut.eq_ignore_ascii_case(key)
            })
        };
        if let Some(tab) = tab {
            self.activate_tab(tab, cx);
        }
        if handled || tab.is_some() {
            cx.stop_propagation();
        }
    }

    fn tracks(&self, has_camera: bool) -> Vec<Track> {
        use timeline::tracks::{range_clips, source_icon, Clip, MusicLane};

        let selected: Option<&str> = self.selected_clip.as_ref().map(|id| id.as_ref());
        let mut tracks = vec![Track::new(
            TrackKind::Video,
            timeline::tracks::video_clips(&self.state.segments, selected),
        )];

        tracks.push(Track::new(
            TrackKind::Zoom,
            range_clips(
                &self.state.zoom_segments,
                selected,
                |segment| segment.id.clone(),
                |segment| (segment.start_time, segment.end_time),
                |segment, clip: &mut Clip| {
                    clip.label = timeline::format_zoom_level(segment.zoom_level).into();
                    clip.icon = TrackKind::Zoom.icon();
                    clip.gradient = TrackKind::Zoom.gradient();
                    clip.zoom_level = Some(segment.zoom_level);
                },
            ),
        ));

        if has_camera {
            tracks.push(Track::new(
                TrackKind::Camera,
                range_clips(
                    &self.state.camera_segments,
                    selected,
                    |segment| segment.id.clone(),
                    |segment| (segment.start_time, segment.end_time),
                    |_, clip: &mut Clip| {
                        clip.label = "Camera".into();
                        clip.icon = TrackKind::Camera.icon();
                        clip.gradient = TrackKind::Camera.gradient();
                    },
                ),
            ));
        }

        if self.state.drawing_segments.is_empty() {
            tracks.push(Track::new(TrackKind::Drawing, Vec::new()));
        }
        for segment in &self.state.drawing_segments {
            let kind = segment.kind();
            tracks.push(
                Track::new(
                    TrackKind::Drawing,
                    range_clips(
                        std::slice::from_ref(segment),
                        selected,
                        |segment| segment.id.clone(),
                        |segment| (segment.start_time, segment.end_time),
                        |segment, clip: &mut Clip| {
                            clip.label = timeline::tracks::drawing_label(segment.kind()).into();
                            clip.icon = timeline::tracks::drawing_icon(segment.kind());
                            clip.gradient = timeline::tracks::drawing_gradient(segment.kind());
                        },
                    ),
                )
                .lane(&segment.id)
                .icon(timeline::tracks::drawing_icon(kind)),
            );
        }

        for group in model::music_groups(&self.state.music_tracks) {
            let Some(first) = group.first() else {
                continue;
            };
            let clips = range_clips(
                &group,
                selected,
                |track| track.id.clone(),
                |track| (track.start_time, track.end_time),
                |track, clip: &mut Clip| {
                    clip.label = track.name.clone().into();
                    clip.detail = Some(
                        crate::util::format::format_duration(track.end_time - track.start_time)
                            .into(),
                    );
                    clip.icon = source_icon(&track.source);
                    clip.gradient = TrackKind::Music.gradient();
                    if (track.speed - 1.0).abs() > f64::EPSILON {
                        clip.badge = Some(timeline::format_speed(track.speed).into());
                    }
                },
            );
            tracks.push(
                Track::new(TrackKind::Music, clips)
                    .lane(model::group_key(first))
                    .icon(source_icon(&first.source))
                    .music(MusicLane {
                        group_id: model::group_key(first).to_string().into(),
                        speed: first.speed,
                        removable: first.source == crate::video::audio_tracks::MUSIC_SOURCE,
                    }),
            );
        }
        tracks
    }
}

impl Render for VideoEditorWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = active_theme(cx);
        let shortcuts = crate::state::state(cx).config.get().shortcuts;
        let total_duration = self.total_duration();
        let has_project = self.path.is_some();
        let file_name = self
            .path
            .as_deref()
            .map(model::project_display_name)
            .unwrap_or_else(|| "Untitled".to_string());
        let poster = self.path.as_deref().and_then(model::poster_frame);
        let has_cursor_data = self
            .path
            .as_deref()
            .is_some_and(|path| project::cursor_path(path).is_file());
        let has_camera = self
            .path
            .as_deref()
            .is_some_and(|path| project::camera_video_path(path).is_file());
        let has_keyboard = self
            .path
            .as_deref()
            .is_some_and(|path| project::keys_path(path).is_file());
        let has_mic = self
            .path
            .as_deref()
            .is_some_and(|path| project::mic_audio_path(path).is_file());

        let title = title_bar::render(
            &title_bar::TitleBarState {
                file_name: file_name.into(),
                project_path: self
                    .path
                    .as_ref()
                    .map(|path| SharedString::from(path.to_string_lossy().to_string())),
                can_undo: !self.history.is_empty(),
                can_redo: !self.future.is_empty(),
                is_sidebar_open: self.state.ui.sidebar_open,
                is_exporting: self.is_exporting,
                export_progress: self.displayed_progress,
                export_completed: self.is_export_complete(cx),
                menu: self.menu.clone(),
            },
            &theme,
            window,
            cx,
        );

        let stage_content = match (&self.preview_image, self.preview_status, &poster) {
            // The composed frame is what the export writes, so it wins.
            (Some(image), _, _) => img(image.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            // Until the first frame lands, the cached poster stands in.
            (None, PreviewStatus::Loading, Some(path)) => img(path.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            (None, PreviewStatus::Loading, None) => loading_preview(&theme),
            (None, PreviewStatus::Unavailable, _) => empty_preview(
                "Could not read this recording",
                "The video file could not be decoded. Make sure FFmpeg is installed and the recording is not damaged.",
                self.path
                    .as_deref()
                    .map(|path| SharedString::from(path.to_string_lossy().to_string())),
                &theme,
            ),
            (None, _, Some(path)) => img(path.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            (None, _, None) => empty_preview(
                "No video loaded",
                "Open a .poratake project to begin editing.",
                None,
                &theme,
            ),
        };

        let mut stage_box = div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center();
        if let Some((width, height)) = self.composition_size {
            stage_box = stage_box.max_w(px(width as f32)).max_h(px(height as f32));
        }
        let preview = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(theme.background)
            .p(px(PREVIEW_INSET))
            .child(
                stage_box.child(stage_content).children(
                    (self.active_tab() == SidebarTab::Drawing && self.state.ui.sidebar_open)
                        .then(|| drawing_overlay::render(self, cx))
                        .flatten(),
                ),
            );

        let controls = timeline::controls::render(
            &timeline::controls::ControlsState {
                is_playing: self.is_playing,
                is_cut_tool_active: self.is_cut_tool_active,
                has_selected_segment: self.selected_segment_index().is_some(),
                can_delete_segment: self.state.segments.len() > 1,
                timeline_position: self.playhead,
                total_duration,
                segment_count: self.state.segments.len(),
                selected_segment_speed: self
                    .selected_segment_index()
                    .and_then(|index| self.state.segments.get(index))
                    .and_then(|segment| segment.speed)
                    .unwrap_or(1.0),
                pixels_per_second: self.pixels_per_second,
                scrub_audio_enabled: self.state.ui.scrub_audio_enabled,
                is_scrub_audio_available: has_project,
            },
            &theme,
            cx,
        );

        self.mirror_timeline_scroll();
        let display_duration = self.display_duration();
        let tracks = self.tracks(has_camera);
        let timeline_body = timeline::tracks::render(
            &timeline::tracks::Timeline {
                tracks: &tracks,
                display_duration,
                total_duration,
                pixels_per_second: self.pixels_per_second,
                playhead: self.preview_playhead.unwrap_or(self.playhead),
                is_cut_tool_active: self.is_cut_tool_active,
                clip_drag: self.clip_drag.as_ref(),
                reorder: self.reorder_drag.as_ref(),
                draw: self.draw_drag.as_ref(),
                menu: &self.menu,
                scroll: &self.tracks_scroll,
            },
            &theme,
            cx,
        );

        let mut stage = div().flex().flex_row().flex_1().min_h_0().child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(preview)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_none()
                        .border_t_1()
                        .border_color(theme.border)
                        .bg(theme.card)
                        .child(timeline::pane::resize_handle(
                            self.timeline_resize.is_some(),
                            &theme,
                            window,
                            cx,
                        ))
                        .child(controls)
                        .child(timeline::ruler::render(
                            display_duration,
                            self.pixels_per_second,
                            &self.ruler_scroll,
                            &theme,
                            cx,
                        ))
                        .child(
                            div()
                                .relative()
                                .h(px(self.timeline_height))
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .id("video-timeline-tracks")
                                        .track_scroll(&self.pane_scroll)
                                        .size_full()
                                        .overflow_y_scroll()
                                        .child(timeline_body),
                                )
                                .child(crate::windows::scrollbars::overlay_vertical(
                                    "video-timeline-tracks-scrollbar",
                                    &self.pane_scroll,
                                    theme.muted_foreground,
                                )),
                        ),
                ),
        );

        use gpui::AnimationExt as _;
        use herogpui::gpui;

        let sidebar_open = self.state.ui.sidebar_open;
        let sidebar_width = self.sidebar_width;
        let panel_width = (sidebar_width - crate::ui::chrome::VIDEO_SIDEBAR_RESIZE).max(0.0);
        let mut sidebar_panel = div()
            .flex()
            .flex_col()
            .w(px(panel_width))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.card);
        if sidebar_open || self.sidebar_closing {
            sidebar_panel = sidebar_panel.child(panels::render(
                self.active_tab(),
                self,
                &self.state,
                has_cursor_data,
                has_camera,
                has_keyboard,
                has_mic,
                self.is_exporting,
                self.displayed_progress,
                &self.menu,
                &theme,
                window,
                cx,
            ));
        }
        let sidebar = div()
            .flex()
            .flex_row()
            .flex_shrink_0()
            .h_full()
            .justify_end()
            .overflow_hidden()
            .child(sidebar::resize_handle(
                self.sidebar_resize.is_some(),
                &theme,
                window,
                cx,
            ))
            .child(sidebar_panel);
        let sidebar = if self.sidebar_animation_generation == 0 {
            sidebar
                .w(px(if sidebar_open { sidebar_width } else { 0.0 }))
                .opacity(if sidebar_open { 1.0 } else { 0.0 })
                .into_any_element()
        } else {
            sidebar
                .with_animation(
                    ("video-sidebar", self.sidebar_animation_generation),
                    gpui::Animation::new(SIDEBAR_ANIMATION_DURATION)
                        .with_easing(crate::ui::primitives::ease_out()),
                    move |sidebar, delta| {
                        let visible = sidebar_animation_visibility(sidebar_open, delta);
                        sidebar.w(px(sidebar_width * visible)).opacity(visible)
                    },
                )
                .into_any_element()
        };
        stage = stage.child(sidebar);

        stage = stage.child(sidebar::tab_rail(
            self.state.ui.sidebar_open.then(|| self.active_tab()),
            &shortcuts.video_editor_sidebar,
            &theme,
            cx,
        ));

        let release_view = cx.entity().downgrade();
        let release_handler = canvas(
            |_bounds, _window, _cx| (),
            move |_bounds, (), window, _cx| {
                window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, _window, cx| {
                    if phase != DispatchPhase::Bubble || event.button != gpui::MouseButton::Left {
                        return;
                    }
                    let _ = release_view.update(cx, |this, cx| {
                        this.end_drawing_stroke(cx);
                        this.end_clip_drag(cx);
                        this.end_sidebar_resize(cx);
                        this.end_timeline_resize(cx);
                        this.end_scrub(cx);
                    });
                });
            },
        )
        .absolute()
        .inset_0();

        div()
            .id("video-editor-window")
            .font_family(crate::ui::font::UI_FONT)
            .key_context("VideoEditor")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event: &gpui::MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle, cx);
                }),
            )
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                    if this.sidebar_resize.is_some() {
                        this.update_sidebar_resize(f32::from(event.position.x), cx);
                    }
                    if this.is_resizing_timeline() {
                        this.update_timeline_resize(f32::from(event.position.y), cx);
                    }
                    this.update_drawing_stroke(event.position, event.modifiers.shift, cx);
                }),
            )
            .child(release_handler)
            .child(title)
            .child(stage)
            .children(self.menu.render(cx))
            .children(
                self.data_editor
                    .as_ref()
                    .map(|editor| data_editor::render(editor, &theme, cx)),
            )
    }
}

struct TitlePopover {
    owner: &'static str,
    editor: gpui::WeakEntity<VideoEditorWindow>,
    theme: crate::theme::vars::ThemeVars,
}

impl Render for TitlePopover {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(editor) = self.editor.upgrade() else {
            return div();
        };
        let owner = self.owner;
        let theme = self.theme.clone();
        let view = editor.read(cx);
        let body = match owner {
            title_bar::EXPORT_POPOVER_ID => title_bar::export_popover(
                view.is_export_complete(cx),
                view.displayed_progress,
                SharedString::from(format!(
                    "{} elapsed",
                    panels::format_export_time(view.export_elapsed_secs(cx))
                )),
                SharedString::from(match view.export_remaining_secs() {
                    Some(remaining) => {
                        format!("{} remaining", panels::format_export_time(remaining))
                    }
                    None => "Calculating...".to_string(),
                }),
                &theme,
                {
                    let editor = editor.clone();
                    move |_window, cx| {
                        editor.update(cx, |this, cx| this.cancel_export(cx));
                    }
                },
            ),
            _ => title_bar::project_popover(
                SharedString::from(
                    view.path
                        .as_deref()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
                view.rename_field.clone(),
                view.rename_error.clone(),
                view.path_recently_copied(cx),
                &theme,
                {
                    let editor = editor.clone();
                    move |value, window, cx| {
                        let value = value.to_string();
                        editor.update(cx, |this, cx| this.rename_project(&value, window, cx));
                    }
                },
                {
                    let editor = editor.clone();
                    move |_window, cx| {
                        editor.update(cx, |this, cx| this.copy_project_path(cx));
                    }
                },
                {
                    let editor = editor.clone();
                    move |_window, cx| {
                        editor.update(cx, |this, cx| this.reveal_project(cx));
                    }
                },
            ),
        };
        body.rounded(px(crate::ui::chrome::RADIUS_LG))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .shadow_lg()
    }
}

fn empty_preview(
    title: &'static str,
    subtitle: &'static str,
    detail: Option<SharedString>,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .text_color(theme.muted_foreground)
        .child(icon_element("alert-circle", px(32.0)))
        .child(
            div()
                .text_size(px(14.0))
                .text_color(theme.foreground)
                .child(title),
        )
        .child(div().text_size(px(12.0)).text_center().child(subtitle))
        .children(detail.map(|detail| {
            div()
                .text_size(px(12.0))
                .font_family(crate::ui::colors::MONO_FONT)
                .child(detail)
        }))
        .into_any_element()
}

fn loading_preview(theme: &crate::theme::vars::ThemeVars) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .text_color(theme.muted_foreground)
        .text_size(px(12.0))
        .child(crate::ui::icon::spinner_element(
            "video-preview-loading",
            px(32.0),
        ))
        .child("Loading recording...")
        .into_any_element()
}

fn tween_progress(current: f32, target: f32, tick_ms: u64) -> f32 {
    let factor = 1.0 - (-(tick_ms as f32) / PROGRESS_TWEEN_MS as f32).exp();
    match (target - current).abs() < 0.001 {
        true => target,
        false => current + (target - current) * factor,
    }
}

fn subtitle_summary(data: &crate::video::sidecars::SubtitleData) -> String {
    let mut summary = format!("{} segments", data.segments.len());
    if !data.meta.model.is_empty() {
        summary.push_str(&format!(" \u{b7} Generated with {} model", data.meta.model));
    }
    if data
        .meta
        .prompt
        .as_deref()
        .is_some_and(|prompt| !prompt.trim().is_empty())
    {
        summary.push_str(" \u{b7} Using custom prompt");
    }
    summary
}

fn format_export_duration(seconds: f64) -> String {
    let total = seconds.round().max(0.0) as u64;
    let minutes = total / 60;
    let rest = total % 60;
    match minutes {
        0 => format!("{rest}s"),
        _ => format!("{minutes}m {rest}s"),
    }
}

fn first_frame_duration(state: &VideoEditorState) -> f64 {
    if !state.first_frame.enabled || state.first_frame.image_data.is_none() {
        return 0.0;
    }
    1.0 / crate::video::export::frame_rate(state).max(1) as f64
}

fn adjust_preview_quality(
    index: usize,
    fast_frames: u16,
    elapsed: Duration,
    budget: Duration,
) -> (usize, u16) {
    if elapsed > budget && index > 0 {
        return (index - 1, 0);
    }
    if elapsed >= budget.mul_f32(0.65) {
        return (index, 0);
    }
    let fast_frames = fast_frames.saturating_add(1);
    if fast_frames < PREVIEW_QUALITY_RAISE_FRAMES || index + 1 >= ACTIVE_PREVIEW_SIZES.len() {
        return (index, fast_frames);
    }
    (index + 1, 0)
}

fn active_preview_budget(frame_rate: f64) -> Duration {
    playback_tick(frame_rate).mul_f32(0.5)
}

fn playback_tick(frame_rate: f64) -> Duration {
    let frame_rate = if frame_rate.is_finite() && frame_rate > 0.0 {
        frame_rate.min(240.0)
    } else {
        60.0
    };
    Duration::from_secs_f64(1.0 / frame_rate)
}

fn materialize_builtin_audio_tracks(
    state: &mut VideoEditorState,
    project: &std::path::Path,
) -> bool {
    let duration = state.source_duration.unwrap_or_else(|| {
        state
            .segments
            .iter()
            .map(|segment| segment.original_end)
            .fold(0.0, f64::max)
    });
    if duration <= 0.0 {
        return false;
    }
    let sources = crate::video::audio_tracks::Sources::resolve(project);
    let mut built_in = Vec::new();
    if sources.system.is_some() {
        let name = match sources.system_is_embedded {
            true => "Audio",
            false => "System Audio",
        };
        built_in.push((
            crate::video::audio_tracks::SYSTEM_TRACK_ID,
            crate::video::audio_tracks::SYSTEM_SOURCE,
            name,
        ));
    }
    if sources.mic.is_some() {
        built_in.push((
            crate::video::audio_tracks::MIC_TRACK_ID,
            crate::video::audio_tracks::MIC_SOURCE,
            "Microphone",
        ));
    }

    let mut added = false;
    for (index, (id, source, name)) in built_in.into_iter().enumerate() {
        if state
            .music_tracks
            .iter()
            .any(|track| track.source == source)
        {
            continue;
        }
        state.music_tracks.insert(
            index.min(state.music_tracks.len()),
            model::MusicTrack {
                id: id.to_string(),
                group_id: id.to_string(),
                name: name.to_string(),
                source: source.to_string(),
                file_name: String::new(),
                volume: 1.0,
                enabled: true,
                start_time: 0.0,
                end_time: duration,
                original_duration: duration,
                trim_start: 0.0,
                trim_end: 0.0,
                speed: 1.0,
            },
        );
        added = true;
    }
    added
}

fn sidebar_animation_visibility(open: bool, delta: f32) -> f32 {
    if open {
        return delta;
    }
    1.0 - delta
}

fn play_keyboard_demo_loop(kind: &str, stop: Arc<AtomicBool>) {
    let started = std::time::Instant::now();
    let mut index = 1_u32;
    while started.elapsed() < Duration::from_secs(5) {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if let Some(path) = crate::video::keyboard_audio::sample_path(kind, index) {
            let path = path.display().to_string();
            let _ = if cfg!(windows) {
                std::process::Command::new("powershell")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!(
                            "Add-Type -AssemblyName PresentationCore; $m = New-Object System.Windows.Media.MediaPlayer; $m.Open([uri]((Resolve-Path '{path}').Path)); $m.Volume = 1; $m.Play(); Start-Sleep -Milliseconds 180"
                        ),
                    ])
                    .status()
            } else {
                std::process::Command::new("afplay").arg(&path).status()
            };
        }
        index = if index >= 4 { 1 } else { index + 1 };
        std::thread::sleep(Duration::from_millis(160));
    }
}

#[cfg(test)]
mod keyboard_demo_tests {
    use super::*;
    use crate::ui::chrome;

    fn test_state(cx: &mut gpui::TestAppContext, dir: &std::path::Path) {
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
    }

    #[herogpui::test]
    fn export_settings_stay_off_the_undo_stack(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                editor.update_cursor(cx, |style| style.size = 200.0);
                assert_eq!(editor.history.len(), 1, "an edit is undoable");

                editor.update_export(cx, |settings| settings.format = "gif".to_string());
                assert_eq!(editor.state.export_settings.format, "gif");
                assert_eq!(
                    editor.history.len(),
                    1,
                    "an export setting must not push history"
                );
                assert!(editor.future.is_empty());

                editor.undo(cx);
                assert_eq!(
                    editor.state.export_settings.format, "gif",
                    "undoing the earlier edit must preserve the export settings"
                );
                assert_eq!(
                    editor.state.cursor_style.size,
                    styles::CursorStyle::default().size
                );
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn the_timeline_pane_height_is_clamped_and_persisted(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                assert_eq!(editor.timeline_height, timeline::default_pane_height());
                assert_eq!(editor.state.ui.timeline_height, None);

                editor.begin_timeline_resize(400.0, cx);
                editor.update_timeline_resize(300.0, cx);
                assert_eq!(
                    editor.timeline_height,
                    timeline::default_pane_height() + 100.0
                );

                editor.update_timeline_resize(-10_000.0, cx);
                assert_eq!(
                    editor.timeline_height,
                    timeline::pane_height(timeline::MAX_PANE_TRACKS)
                );

                editor.end_timeline_resize(cx);
                assert_eq!(
                    editor.state.ui.timeline_height,
                    Some(timeline::pane_height(timeline::MAX_PANE_TRACKS) as f64)
                );
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn dragging_a_video_clip_past_a_neighbour_reorders_it(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                editor.state.source_duration = Some(12.0);
                editor.state.segments = vec![
                    model::Segment {
                        id: "a".into(),
                        original_start: 0.0,
                        original_end: 4.0,
                        trim_min_start: 0.0,
                        trim_max_end: 4.0,
                        speed: None,
                    },
                    model::Segment {
                        id: "b".into(),
                        original_start: 4.0,
                        original_end: 8.0,
                        trim_min_start: 4.0,
                        trim_max_end: 8.0,
                        speed: None,
                    },
                ];

                editor.begin_reorder("a".into(), 0.0, cx);
                assert!(editor.update_timeline_drag(7.0, 2.0, cx));
                editor.end_clip_drag(cx);
                assert_eq!(editor.state.segments[0].id, "a");

                editor.begin_reorder("a".into(), 0.0, cx);
                assert!(editor.update_timeline_drag(7.0, 40.0, cx));
                assert_eq!(
                    editor
                        .reorder_drag
                        .as_ref()
                        .expect("a reorder is in flight")
                        .drop_index,
                    1
                );
                editor.end_clip_drag(cx);
                assert_eq!(editor.state.segments[0].id, "b");
                assert_eq!(editor.state.segments[1].id, "a");
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn drawing_on_an_empty_lane_adds_a_clip(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                editor.state.source_duration = Some(20.0);
                editor.state.segments = vec![model::Segment::spanning(20.0)];

                editor.begin_draw(TrackKind::Zoom, 1.0, cx);
                editor.end_clip_drag(cx);
                assert_eq!(editor.state.zoom_segments.len(), 1);
                assert_eq!(editor.state.zoom_segments[0].start_time, 1.0);
                assert_eq!(editor.state.zoom_segments[0].end_time, 4.0);

                editor.begin_draw(TrackKind::Zoom, 6.0, cx);
                editor.update_timeline_drag(10.0, 0.0, cx);
                editor.end_clip_drag(cx);
                assert_eq!(editor.state.zoom_segments.len(), 2);
                assert_eq!(editor.state.zoom_segments[1].start_time, 6.0);
                assert_eq!(editor.state.zoom_segments[1].end_time, 10.0);

                editor.begin_draw(TrackKind::Zoom, 15.0, cx);
                editor.update_timeline_drag(15.2, 0.0, cx);
                editor.end_clip_drag(cx);
                assert_eq!(
                    editor.state.zoom_segments.len(),
                    2,
                    "a drag under the minimum duration draws nothing"
                );
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn changing_the_format_renormalizes_the_dependent_fields(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                assert_eq!(editor.state.export_settings.resolution, "4k");
                assert_eq!(editor.state.export_settings.frame_rate, "30");

                editor.update_export(cx, |settings| settings.format = "gif".to_string());
                let settings = &editor.state.export_settings;
                assert_eq!(settings.resolution, "720p");
                assert_eq!(settings.quality_preset, "web");
                assert_eq!(settings.frame_rate, "20");
                assert!(styles::GIF_FRAME_RATES.contains(&settings.frame_rate.as_str()));

                editor.update_export(cx, |settings| settings.format = "mp4".to_string());
                let settings = &editor.state.export_settings;
                assert_eq!(settings.resolution, "4k");
                assert_eq!(settings.frame_rate, "30");
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn cancelling_an_export_leaves_no_error_behind(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, cx| {
                editor.is_exporting = true;
                editor.export_progress = 0.4;
                editor.displayed_progress = 0.4;
                editor.export_error = Some(SharedString::from("stale"));

                editor.cancel_export(cx);

                assert!(editor.export_cancelled.load(Ordering::Relaxed));
                assert!(!editor.is_exporting);
                assert_eq!(editor.export_error(), None, "a cancel is not a failure");
                assert!(!editor.is_export_complete(cx));
                assert_eq!(editor.displayed_progress, 0.0);
            })
            .expect("update editor");
    }

    #[herogpui::test]
    fn a_failed_export_keeps_its_error_until_the_next_attempt(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        window
            .update(cx, |editor, _window, _cx| {
                editor.export_error = Some(SharedString::from("ffmpeg exited with 1"));
                assert_eq!(
                    editor.export_error().as_deref(),
                    Some("ffmpeg exited with 1")
                );
            })
            .expect("update editor");
    }

    #[test]
    fn the_completion_badge_matches_the_electron_hold() {
        assert_eq!(EXPORT_COMPLETION_MS, 3000);
        assert_eq!(COPY_FEEDBACK_MS, 2000);
    }

    #[test]
    fn the_progress_fill_eases_rather_than_jumping() {
        let first = tween_progress(0.0, 1.0, 100);
        assert!(first > 0.0 && first < 1.0, "{first}");
        let second = tween_progress(first, 1.0, 100);
        assert!(second > first && second < 1.0, "{second}");
        assert_eq!(tween_progress(0.5, 0.5, 100), 0.5);
    }

    #[test]
    fn the_subtitle_summary_names_the_model_and_the_prompt() {
        let mut data = crate::video::sidecars::SubtitleData::default();
        data.segments
            .push(crate::video::sidecars::SubtitleSegment::default());
        data.meta.model = "base".to_string();
        assert_eq!(
            subtitle_summary(&data),
            "1 segments \u{b7} Generated with base model"
        );
        data.meta.prompt = Some("names".to_string());
        assert_eq!(
            subtitle_summary(&data),
            "1 segments \u{b7} Generated with base model \u{b7} Using custom prompt"
        );
    }

    #[test]
    fn the_export_duration_reads_the_way_electron_formats_it() {
        assert_eq!(format_export_duration(9.4), "9s");
        assert_eq!(format_export_duration(65.0), "1m 5s");
        assert_eq!(format_export_duration(0.0), "0s");
    }

    #[herogpui::test]
    fn a_burst_of_edits_writes_once_with_a_fresh_saved_at(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        test_state(cx, dir.path());
        let project = dir.path().join("Take.poratake");
        std::fs::create_dir_all(&project).expect("project dir");
        let window =
            cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(Some(project.clone()), cx));

        for size in [120.0, 140.0, 160.0] {
            window
                .update(cx, |editor, _window, cx| {
                    editor.update_cursor(cx, move |style| style.size = size);
                })
                .expect("update editor");
        }
        cx.executor().advance_clock(SAVE_DEBOUNCE * 3);
        cx.run_until_parked();

        let written = model::load_state(&project);
        assert_eq!(
            written.cursor_style.size, 160.0,
            "only the last value lands"
        );
        assert!(
            chrono::DateTime::parse_from_rfc3339(&written.saved_at).is_ok(),
            "savedAt must be a fresh RFC 3339 timestamp, got {:?}",
            written.saved_at
        );
    }

    #[herogpui::test]
    fn editor_hotkeys_reach_the_focused_window(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let window = cx.add_window(|window, cx| {
            let editor = VideoEditorWindow::new_for_test(None, cx);
            window.focus(&editor.focus_handle, cx);
            editor
        });
        cx.refresh().expect("draw editor");
        cx.run_until_parked();

        cx.simulate_keystrokes(window.into(), "space");
        assert!(window.update(cx, |editor, _, _| editor.is_playing).unwrap());

        cx.simulate_keystrokes(window.into(), "ctrl-s");
        assert!(window
            .update(cx, |editor, _, _| {
                editor.state.ui.sidebar_open && editor.active_tab() == SidebarTab::Export
            })
            .unwrap());
    }

    #[herogpui::test]
    fn typing_in_a_text_field_does_not_fire_editor_shortcuts(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let window = cx.add_window(|window, cx| {
            let editor = VideoEditorWindow::new_for_test(None, cx);
            window.focus(&editor.focus_handle, cx);
            editor
        });
        cx.refresh().expect("draw editor");
        cx.run_until_parked();

        window
            .update(cx, |editor, window, cx| {
                let focus = editor.rename_field.read(cx).focus_handle(cx);
                window.focus(&focus, cx);
            })
            .expect("focus rename field");

        cx.simulate_keystrokes(window.into(), "space");
        assert!(!window.update(cx, |editor, _, _| editor.is_playing).unwrap());
    }

    #[herogpui::test]
    fn title_reserves_native_window_controls(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (_, cx) = cx.add_window_view(|_, cx| VideoEditorWindow::new_for_test(None, cx));
        cx.refresh().expect("draw editor");
        cx.run_until_parked();
        let title = cx.debug_bounds("video-title").unwrap();
        let expected = if cfg!(target_os = "macos") {
            chrome::MACOS_TITLE_LEADING_INSET + chrome::TITLE_BAR_PADDING_X
        } else {
            chrome::TITLE_BAR_PADDING_X + crate::ui::window_controls::drag_area_leading_inset()
        };
        assert_eq!(title.left(), px(expected));
        assert!(title.bottom() <= px(chrome::TITLE_BAR_HEIGHT));
    }

    #[herogpui::test]
    fn mouse_release_outside_the_window_ends_scrubbing(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (editor, cx) = cx.add_window_view(|window, cx| {
            let editor = VideoEditorWindow::new_for_test(None, cx);
            window.focus(&editor.focus_handle, cx);
            editor
        });
        cx.update(|_window, cx| {
            editor.update(cx, |editor, _cx| {
                editor.begin_scrub();
            });
        });
        cx.refresh().expect("draw editor");
        cx.run_until_parked();

        cx.simulate_mouse_up(
            gpui::point(px(-10.0), px(-10.0)),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );

        cx.read(|cx| {
            let editor = editor.read(cx);
            assert!(!editor.is_scrubbing);
        });
    }

    #[herogpui::test]
    fn sidebar_content_is_retained_until_collapse_finishes(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));
        window
            .update(cx, |editor, _window, cx| {
                editor.state.ui.sidebar_open = true;
                editor.toggle_sidebar(cx);
                assert!(editor.sidebar_closing);
            })
            .unwrap();

        cx.executor().advance_clock(SIDEBAR_ANIMATION_DURATION);
        cx.run_until_parked();

        assert!(!window
            .update(cx, |editor, _, _| editor.sidebar_closing)
            .unwrap());
    }

    #[test]
    fn playback_uses_the_source_frame_rate() {
        assert_eq!(playback_tick(30.0), Duration::from_secs_f64(1.0 / 30.0));
        assert_eq!(playback_tick(60.0), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(playback_tick(120.0), Duration::from_secs_f64(1.0 / 120.0));
        assert_eq!(playback_tick(f64::NAN), playback_tick(60.0));
    }

    #[test]
    fn frame_stepping_is_a_fixed_thirty_per_second() {
        assert_eq!(FRAME_STEP, 1.0 / 30.0);
        assert_eq!(LAST_FRAME_EPSILON, 0.01);
    }

    #[test]
    fn first_frame_uses_the_export_frame_rate() {
        let mut state = VideoEditorState::default();
        state.first_frame.enabled = true;
        state.first_frame.image_data = Some("data:,".into());
        state.export_settings.frame_rate = "24".into();
        assert_eq!(first_frame_duration(&state), 1.0 / 24.0);
        state.first_frame.enabled = false;
        assert_eq!(first_frame_duration(&state), 0.0);
    }

    #[test]
    fn active_preview_quality_tracks_the_frame_budget() {
        let budget = active_preview_budget(60.0);
        assert_eq!(
            adjust_preview_quality(3, 0, budget + Duration::from_millis(1), budget),
            (2, 0)
        );
        assert_eq!(
            adjust_preview_quality(
                2,
                PREVIEW_QUALITY_RAISE_FRAMES - 1,
                Duration::from_millis(1),
                budget
            ),
            (3, 0)
        );
        assert_eq!(
            adjust_preview_quality(0, 0, budget + Duration::from_millis(1), budget),
            (0, 0)
        );
        assert!(
            active_preview_budget(30.0).abs_diff(Duration::from_secs_f64(1.0 / 60.0))
                <= Duration::from_nanos(1)
        );
    }

    #[test]
    fn sidebar_animation_runs_in_both_directions() {
        assert_eq!(sidebar_animation_visibility(true, 0.25), 0.25);
        assert_eq!(sidebar_animation_visibility(false, 0.25), 0.75);
    }

    #[test]
    fn keyboard_demo_resolves_bundled_samples() {
        let path = crate::video::keyboard_audio::sample_path("cherry-blue", 1)
            .expect("bundled keyboard sample");
        assert!(path.ends_with("press-1.mp3"));
        assert!(path.is_file());
    }

    #[herogpui::test]
    fn export_eta_matches_the_electron_estimator(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let (editor, cx) = cx.add_window_view(|_, cx| VideoEditorWindow::new_for_test(None, cx));
        cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                editor.export_started_at =
                    Some(cx.background_executor().now() - Duration::from_secs(10));
                editor.export_progress = 0.01;
                editor.update_export_eta(cx);
                assert_eq!(editor.export_remaining_secs(), None);
                editor.export_progress = 0.5;
                editor.update_export_eta(cx);
                assert_eq!(editor.export_remaining_secs(), Some(10));
                editor.export_progress = 0.75;
                editor.update_export_eta(cx);
                let smoothed = editor.export_remaining.expect("smoothed eta");
                assert!((smoothed - 9.33).abs() < 0.5, "smoothed eta {smoothed}");
            });
        });
    }
}
