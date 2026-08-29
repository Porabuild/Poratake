pub mod data_editor;
pub mod model;
pub mod panel_kit;
pub mod panels;
pub mod preview;
pub mod sidebar;
pub mod styles;
pub mod timeline;
pub mod title_bar;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
const UNDO_HISTORY_LIMIT: usize = 100;
/// Playback pulls one composed frame per tick; the composition is software
/// rasterized, so this is a preview rate rather than the export frame rate.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    div, img, prelude::*, px, size, App, Bounds, Context, Entity, FocusHandle, KeyDownEvent,
    Render, ScrollHandle, SharedString, Styled, Window,
};

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

/// Where the preview pipeline is: a project has to be decoded before the first
/// composed frame can appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreviewStatus {
    Idle,
    Loading,
    Ready,
    Unavailable,
}

pub struct VideoEditorWindow {
    path: Option<PathBuf>,
    state: VideoEditorState,
    history: Vec<VideoEditorState>,
    future: Vec<VideoEditorState>,
    pixels_per_second: f32,
    playhead: f64,
    is_playing: bool,
    is_cut_tool_active: bool,
    is_exporting: bool,
    export_progress: f32,
    selected_clip: Option<SharedString>,
    menu: MenuHandle,
    save_scheduled: bool,
    ruler_scroll: ScrollHandle,
    tracks_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    source: Option<preview::Handle>,
    source_frame_rate: f64,
    preview_status: PreviewStatus,
    preview_image: Option<Arc<gpui::RenderImage>>,
    /// One compose runs at a time; a request that arrives while one is in
    /// flight replaces the queued position rather than piling up.
    compose_in_flight: bool,
    queued_frame: Option<f64>,
    playback_generation: u64,
    /// The export runs on the background executor; progress is published in
    /// permille and cancellation is a flag the render loop checks per frame.
    export_progress_permille: Arc<AtomicU32>,
    export_cancelled: Arc<AtomicBool>,
    clip_drag: Option<ClipDrag>,
    /// The transcript's segment count, refreshed whenever it is written.
    subtitle_count: usize,
    /// The open JSON data editor, if any.
    data_editor: Option<data_editor::DataEditor>,
    is_transcribing: bool,
    transcription_model: String,
    transcription_prompt: String,
    prompt_field: Entity<crate::ui::text_area::TextArea>,
    upload_to_cloud: bool,
    cloud_upload: crate::cloud::UploadState,
    uploaded_url: Option<String>,
    drawing_tools: styles::DrawingToolSettings,
    sidebar_width: f32,
    sidebar_resize: Option<(f32, f32)>,
    speed_selector_open: bool,
    desktop_wallpaper_source: Option<String>,
    desktop_wallpaper_preview: Option<Arc<gpui::RenderImage>>,
    keyboard_demo: Option<std::sync::Arc<AtomicBool>>,
    /// The state a drag started from, pushed onto the undo stack when it ends.
    gesture_snapshot: Option<VideoEditorState>,
}

impl VideoEditorWindow {
    pub fn open(cx: &mut App, path: Option<String>) {
        registry::open_or_activate(RegistryKind::VideoEditor, cx, move |cx| {
            let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
            cx.open_window(
                crate::windows::app_window_options(bounds, Some(size(px(1200.0), px(750.0)))),
                |window, cx| {
                    let view = cx.new(|cx| {
                        let mut editor = Self::new(path.map(PathBuf::from), cx);
                        editor.load_preview(cx);
                        editor.load_desktop_wallpaper(cx);
                        editor
                    });
                    window.focus(&view.read(cx).focus_handle);
                    view
                },
            )
            .ok()
            .map(Into::into)
        });
    }

    fn new(path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let mut state = path.as_deref().map(model::load_state).unwrap_or_default();
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
        let pixels_per_second = state
            .timeline_zoom
            .map(|value| timeline::clamp_zoom(value as f32))
            .unwrap_or(timeline::DEFAULT_PIXELS_PER_SECOND);
        Self {
            path,
            state,
            history: Vec::new(),
            future: Vec::new(),
            pixels_per_second,
            playhead: 0.0,
            is_playing: false,
            is_cut_tool_active: false,
            is_exporting: false,
            export_progress: 0.0,
            selected_clip: None,
            menu: MenuHandle::new(),
            save_scheduled: false,
            ruler_scroll: ScrollHandle::new(),
            tracks_scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            source: None,
            source_frame_rate: 60.0,
            preview_status: PreviewStatus::Idle,
            preview_image: None,
            compose_in_flight: false,
            queued_frame: None,
            playback_generation: 0,
            export_progress_permille: Arc::new(AtomicU32::new(0)),
            export_cancelled: Arc::new(AtomicBool::new(false)),
            clip_drag: None,
            gesture_snapshot: None,
            subtitle_count: 0,
            data_editor: None,
            is_transcribing: false,
            transcription_model: "base".to_string(),
            transcription_prompt: String::new(),
            prompt_field: cx.new(|cx| {
                crate::ui::text_area::TextArea::new(String::new(), cx)
                    .placeholder("Add context to improve accuracy...")
                    .rows(4)
            }),
            upload_to_cloud: false,
            cloud_upload: crate::cloud::UploadState::Idle,
            uploaded_url: None,
            drawing_tools: styles::DrawingToolSettings::default(),
            sidebar_width: crate::ui::chrome::VIDEO_SIDEBAR_WIDTH,
            sidebar_resize: None,
            speed_selector_open: false,
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
        let Some(output) = rfd::FileDialog::new()
            .set_title("Export video")
            .add_filter(filter_name, extensions)
            .set_file_name(
                suggested
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(if is_gif { "export.gif" } else { "export.mp4" }),
            )
            .set_directory(suggested.parent().unwrap_or(&project))
            .save_file()
        else {
            return;
        };
        // A picker can hand back a name without the extension the format needs.
        let output = if output
            .extension()
            .is_some_and(|value| value.eq_ignore_ascii_case(extensions[0]))
        {
            output
        } else {
            output.with_extension(extensions[0])
        };

        self.is_exporting = true;
        self.export_progress = 0.0;
        self.export_progress_permille = Arc::new(AtomicU32::new(0));
        self.export_cancelled = Arc::new(AtomicBool::new(false));
        cx.notify();

        let progress = self.export_progress_permille.clone();
        let cancelled = self.export_cancelled.clone();
        let reveal = state.export_settings.open_in_finder;

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

            let (title, body) = match &result {
                Ok(path) => {
                    if reveal {
                        crate::system::desktop::reveal_in_file_manager(path);
                    }
                    ("Export finished", path.to_string_lossy().to_string())
                }
                Err(error) => ("Export failed", error.clone()),
            };
            let _ = cx.update(|cx| crate::windows::toast::Toast::show(cx, title, &body));
            let output = result.ok();
            let _ = entity.update(cx, |this, cx| {
                this.is_exporting = false;
                this.export_progress = 0.0;
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
                cx.notify();
                true
            });
            if !matches!(running, Ok(true)) {
                break;
            }
        })
        .detach();
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
        let time = self.playhead;
        if self.compose_in_flight {
            self.queued_frame = Some(time);
            return;
        }
        self.compose_in_flight = true;

        cx.spawn(async move |entity, cx| {
            let composed = cx
                .background_executor()
                .spawn(async move {
                    let mut guard = source.lock();
                    guard
                        .compose(time)
                        .as_ref()
                        .and_then(preview::to_render_image)
                })
                .await;

            let _ = entity.update(cx, |this, cx| {
                this.compose_in_flight = false;
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
        let state = self.state.clone();
        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .spawn(async move { source.lock().set_state(state) })
                .await;
            let _ = entity.update(cx, |this, cx| this.request_frame(cx));
        })
        .detach();
    }

    /// Advances the playhead in wall-clock time while playing. Each run is
    /// tagged so a pause or a second play stops the previous loop.
    fn drive_playback(&mut self, cx: &mut Context<Self>) {
        self.playback_generation += 1;
        let generation = self.playback_generation;
        let tick = playback_tick(self.source_frame_rate);
        let started = std::time::Instant::now();
        let initial_playhead = self.playhead;

        cx.spawn(async move |entity, cx| loop {
            cx.background_executor().timer(tick).await;
            let advanced = entity.update(cx, |this, cx| {
                if !this.is_playing || this.playback_generation != generation {
                    return false;
                }
                let total = this.total_duration();
                let next = initial_playhead + started.elapsed().as_secs_f64();
                if next >= total {
                    this.playhead = total;
                    this.is_playing = false;
                    this.request_frame(cx);
                    cx.notify();
                    return false;
                }
                this.playhead = next;
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
        cx.notify();
    }

    fn total_duration(&self) -> f64 {
        model::total_duration(
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
        if self.history.len() >= UNDO_HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(before);
        self.future.clear();
        self.persist(cx);
        self.sync_preview_state(cx);
        cx.notify();
    }

    /// Takes the snapshot a gesture will be undone to. Repeated calls during
    /// one gesture keep the first snapshot.
    fn begin_gesture(&mut self) {
        if self.gesture_snapshot.is_none() {
            self.gesture_snapshot = Some(self.state.clone());
        }
    }

    /// Closes a gesture, pushing one history entry for everything it changed.
    fn end_gesture(&mut self, cx: &mut Context<Self>) {
        let Some(before) = self.gesture_snapshot.take() else {
            return;
        };
        if before == self.state {
            return;
        }
        if self.history.len() >= UNDO_HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push(before);
        self.future.clear();
        self.persist(cx);
        self.sync_preview_state(cx);
        cx.notify();
    }

    /// Slider drags land many changes per second, so writes are debounced the
    /// same way `use-editor-state-persistence.ts` does.
    fn persist(&mut self, cx: &mut Context<Self>) {
        if self.path.is_none() || self.save_scheduled {
            return;
        }
        self.save_scheduled = true;
        cx.spawn(async move |entity, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            let _ = entity.update(cx, |this, _cx| {
                this.save_scheduled = false;
                if let Some(path) = &this.path {
                    model::save_state(path, &this.state);
                }
            });
        })
        .detach();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(previous) = self.history.pop() else {
            return;
        };
        self.future
            .push(std::mem::replace(&mut self.state, previous));
        self.persist(cx);
        self.sync_preview_state(cx);
        cx.notify();
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        let Some(next) = self.future.pop() else {
            return;
        };
        self.history.push(std::mem::replace(&mut self.state, next));
        self.persist(cx);
        self.sync_preview_state(cx);
        cx.notify();
    }

    pub fn reset_state(&mut self, cx: &mut Context<Self>) {
        self.commit(cx, |state| {
            let segments = std::mem::take(&mut state.segments);
            let zoom_segments = std::mem::take(&mut state.zoom_segments);
            let camera_segments = std::mem::take(&mut state.camera_segments);
            let drawing_segments = std::mem::take(&mut state.drawing_segments);
            let music_tracks = std::mem::take(&mut state.music_tracks);
            let source_duration = state.source_duration;
            let recording_type = state.recording_type.clone();
            *state = VideoEditorState {
                segments,
                zoom_segments,
                camera_segments,
                drawing_segments,
                music_tracks,
                source_duration,
                recording_type,
                ..VideoEditorState::default()
            };
        });
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

    pub fn select_tab(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        if self.state.ui.sidebar_open && self.active_tab() == tab {
            self.state.ui.sidebar_open = false;
        } else {
            self.state.ui.sidebar_open = true;
            self.state.ui.sidebar_tab = tab.id().to_string();
        }
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.ui.sidebar_open = !self.state.ui.sidebar_open;
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.is_playing = !self.is_playing;
        if self.is_playing {
            if self.playhead >= self.total_duration() {
                self.playhead = 0.0;
            }
            self.drive_playback(cx);
        }
        cx.notify();
    }

    fn activate_tab(&mut self, tab: SidebarTab, cx: &mut Context<Self>) {
        self.state.ui.sidebar_open = true;
        self.state.ui.sidebar_tab = tab.id().to_string();
        self.persist(cx);
        cx.notify();
    }

    pub fn toggle_speed_selector(&mut self, cx: &mut Context<Self>) {
        self.speed_selector_open = !self.speed_selector_open;
        cx.notify();
    }

    pub fn close_speed_selector(&mut self, cx: &mut Context<Self>) {
        self.speed_selector_open = false;
        cx.notify();
    }

    /// Moves the playhead to an absolute position — the timeline click and
    /// scrub path.
    pub fn set_playhead(&mut self, time: f64, cx: &mut Context<Self>) {
        let total = self.total_duration();
        self.playhead = time.clamp(0.0, total);
        self.request_frame(cx);
        cx.notify();
    }

    pub fn toggle_cut_tool(&mut self, cx: &mut Context<Self>) {
        self.is_cut_tool_active = !self.is_cut_tool_active;
        cx.notify();
    }

    pub fn set_scrub_audio(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.state.ui.scrub_audio_enabled = enabled;
        self.persist(cx);
        cx.notify();
    }

    pub fn select_clip(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.speed_selector_open = false;
        self.selected_clip = if self.selected_clip.as_ref() == Some(&id) {
            None
        } else {
            Some(id)
        };
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
        let next = index as isize + delta;
        if next < 0 || next >= self.state.segments.len() as isize {
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
        self.speed_selector_open = false;
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
                ..model::DrawingSegment::default()
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
        self.is_transcribing
    }

    pub fn transcription_model(&self) -> &str {
        &self.transcription_model
    }

    pub fn set_transcription_model(&mut self, model: String, cx: &mut Context<Self>) {
        self.transcription_model = model;
        cx.notify();
    }

    fn refresh_subtitle_count(&mut self) {
        self.subtitle_count = self
            .path
            .as_deref()
            .and_then(crate::video::sidecars::load_subtitle)
            .map(|data| data.segments.len())
            .unwrap_or(0);
    }

    /// Downloads the model if it is missing, transcribes the recording and
    /// writes `subtitle.json`, then reloads the composition so the captions
    /// show in the preview.
    pub fn generate_subtitles(&mut self, cx: &mut Context<Self>) {
        if self.is_transcribing {
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
        self.is_transcribing = true;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::video::transcription::download_model(&model)?;
                    crate::video::transcription::transcribe(
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
                    )
                })
                .await;

            let (title, body) = match &result {
                Ok(data) => (
                    "Subtitles ready",
                    format!("{} segments transcribed.", data.segments.len()),
                ),
                Err(error) => ("Transcription failed", error.clone()),
            };
            let _ = cx.update(|cx| crate::windows::toast::Toast::show(cx, title, &body));
            let _ = entity.update(cx, |this, cx| {
                this.is_transcribing = false;
                this.refresh_subtitle_count();
                this.reload_preview(cx);
                cx.notify();
            });
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
        field.update(cx, |field, cx| field.set_value(&value, cx));
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
        field.update(cx, |field, cx| field.set_value(&value, cx));
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
            // Video clips are trimmed through their own segment controls, not
            // by dragging, because their extents are video time.
            TrackKind::Video => {}
        }
        cx.notify();
    }

    pub fn end_clip_drag(&mut self, cx: &mut Context<Self>) {
        if self.clip_drag.take().is_none() {
            return;
        }
        self.end_gesture(cx);
    }

    pub fn is_dragging_clip(&self) -> bool {
        self.clip_drag.is_some()
    }

    pub fn playhead(&self) -> f64 {
        self.playhead
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

    pub fn cancel_export(&mut self, cx: &mut Context<Self>) {
        self.export_cancelled.store(true, Ordering::Relaxed);
        self.is_exporting = false;
        self.export_progress = 0.0;
        cx.notify();
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
        if let Some(url) = &self.uploaded_url {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(url.clone());
            }
        }
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

        let removed = match project::project_folder(&path) {
            Some(folder) => std::fs::remove_dir_all(folder),
            None => std::fs::remove_file(&path),
        };
        if let Err(error) = removed {
            eprintln!("[video-editor] delete failed: {error}");
            return;
        }
        crate::thumbnails::remove(&path);
        window.remove_window();
        registry::close(RegistryKind::VideoEditor, cx);
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
        self.commit(cx, |state| mutate(&mut state.export_settings));
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

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        let primary = modifiers.control || modifiers.platform;
        let shortcuts = crate::state::state(cx).config.get().shortcuts;
        let key = event.keystroke.key.as_str();

        if key == "escape" {
            self.speed_selector_open = false;
            self.selected_clip = None;
            cx.stop_propagation();
            cx.notify();
            return;
        }

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
                self.set_playhead(self.total_duration(), cx);
                true
            }
            "backspace" | "delete" => {
                self.delete_selection(cx);
                true
            }
            "," => {
                self.seek(-frame_step(self.source_frame_rate), cx);
                true
            }
            "." => {
                self.seek(frame_step(self.source_frame_rate), cx);
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

    fn tracks(&self) -> Vec<Track> {
        let selected: Option<&str> = self.selected_clip.as_ref().map(|id| id.as_ref());
        let mut tracks = vec![Track {
            kind: TrackKind::Video,
            clips: timeline::tracks::video_clips(&self.state.segments, selected),
        }];

        // `<ZoomTrack>` is rendered unconditionally in
        // `video-editor-window.tsx`, and an empty drawing lane falls back to a
        // bare `<TrackRow />`. Both lanes are therefore always visible, empty or
        // not -- this shell only drew a lane that had content, so a fresh
        // recording showed one lane where the reference shows three.
        {
            tracks.push(Track {
                kind: TrackKind::Zoom,
                clips: timeline::tracks::range_clips(
                    &self.state.zoom_segments,
                    selected,
                    |segment| segment.id.clone(),
                    |segment| (segment.start_time, segment.end_time),
                    |segment| format!("{:.1}x", segment.zoom_level),
                    |_| ("#818cf8", "#4f46e5"),
                    |segment| Some(segment.zoom_level),
                ),
            });
        }
        if !self.state.camera_segments.is_empty() {
            tracks.push(Track {
                kind: TrackKind::Camera,
                clips: timeline::tracks::range_clips(
                    &self.state.camera_segments,
                    selected,
                    |segment| segment.id.clone(),
                    |segment| (segment.start_time, segment.end_time),
                    |_| "Camera".to_string(),
                    |_| ("#c084fc", "#7e22ce"),
                    |_| None,
                ),
            });
        }
        {
            tracks.push(Track {
                kind: TrackKind::Drawing,
                clips: timeline::tracks::range_clips(
                    &self.state.drawing_segments,
                    selected,
                    |segment| segment.id.clone(),
                    |segment| (segment.start_time, segment.end_time),
                    |segment| segment.kind().to_string(),
                    |segment| timeline::tracks::drawing_gradient(segment.kind()),
                    |_| None,
                ),
            });
        }
        if !self.state.music_tracks.is_empty() {
            tracks.push(Track {
                kind: TrackKind::Music,
                clips: timeline::tracks::range_clips(
                    &self.state.music_tracks,
                    selected,
                    |track| track.id.clone(),
                    |track| (track.start_time, track.end_time),
                    |track| track.name.clone(),
                    |_| ("#f472b6", "#be185d"),
                    |_| None,
                ),
            });
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
                export_progress: self.export_progress,
            },
            &theme,
            window,
            cx,
        );

        let stage_content = match (&self.preview_image, self.preview_status, &poster) {
            // The composed frame is what the export writes, so it wins.
            (Some(image), _, _) => img(image.clone())
                .max_w_full()
                .max_h_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            // Until the first frame lands, the cached poster stands in.
            (None, PreviewStatus::Loading, Some(path)) => img(path.clone())
                .max_w_full()
                .max_h_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            (None, PreviewStatus::Loading, None) => {
                empty_preview("Loading\u{2026}", "Decoding the recording.", &theme)
            }
            (None, PreviewStatus::Unavailable, _) => empty_preview(
                "Preview unavailable",
                "This recording could not be decoded on this system.",
                &theme,
            ),
            (None, _, Some(path)) => img(path.clone())
                .max_w_full()
                .max_h_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            (None, _, None) => empty_preview(
                "No video loaded",
                "Open a .poratake project to begin editing.",
                &theme,
            ),
        };

        let preview = div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(theme.surface)
            .child(stage_content);

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
                speed_selector_open: self.speed_selector_open,
                pixels_per_second: self.pixels_per_second,
                scrub_audio_enabled: self.state.ui.scrub_audio_enabled,
                is_scrub_audio_available: has_project,
            },
            &theme,
            cx,
        );

        let tracks = self.tracks();
        let timeline_body = timeline::tracks::render(
            &tracks,
            total_duration,
            self.pixels_per_second,
            self.playhead,
            self.is_cut_tool_active,
            &self.menu,
            &self.tracks_scroll,
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
                        .child(controls)
                        .child(timeline::ruler::render(
                            total_duration,
                            self.pixels_per_second,
                            &self.ruler_scroll,
                            &theme,
                            cx,
                        ))
                        .child(
                            div()
                                .id("video-timeline-tracks")
                                .h(px(crate::ui::chrome::video_timeline_tracks_height(
                                    timeline::TRACK_HEIGHT,
                                )))
                                .overflow_x_scroll()
                                .child(timeline_body),
                        ),
                ),
        );

        if self.state.ui.sidebar_open {
            use gpui::AnimationExt as _;

            let sidebar_width = self.sidebar_width;
            let panel_width = (sidebar_width - crate::ui::chrome::VIDEO_SIDEBAR_RESIZE).max(0.0);
            stage = stage.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_shrink_0()
                    .h_full()
                    .w(px(sidebar_width))
                    .justify_end()
                    .overflow_hidden()
                    .child(sidebar::resize_handle(
                        self.sidebar_resize.is_some(),
                        &theme,
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .w(px(panel_width))
                            .flex_shrink_0()
                            .h_full()
                            .bg(theme.card)
                            .child(panels::render(
                                self.active_tab(),
                                self,
                                &self.state,
                                has_cursor_data,
                                has_camera,
                                has_keyboard,
                                has_mic,
                                self.is_exporting,
                                self.export_progress,
                                &self.menu,
                                &theme,
                                window,
                                cx,
                            )),
                    )
                    .with_animation(
                        "video-sidebar-open",
                        gpui::Animation::new(Duration::from_millis(180))
                            .with_easing(crate::ui::primitives::ease_out()),
                        move |sidebar, delta| sidebar.w(px(sidebar_width * delta)).opacity(delta),
                    ),
            );
        }

        stage = stage.child(sidebar::tab_rail(
            self.state.ui.sidebar_open.then(|| self.active_tab()),
            &shortcuts.video_editor_sidebar,
            &theme,
            cx,
        ));

        div()
            .id("video-editor-window")
            .key_context("VideoEditor")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                    if this.sidebar_resize.is_some() {
                        this.update_sidebar_resize(f32::from(event.position.x), cx);
                    }
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event: &gpui::MouseUpEvent, _window, cx| {
                    this.end_clip_drag(cx);
                    this.end_sidebar_resize(cx);
                }),
            )
            .child(title)
            .child(stage)
            .children(self.menu.render())
            .children(
                self.data_editor
                    .as_ref()
                    .map(|editor| data_editor::render(editor, &theme, cx)),
            )
    }
}

fn empty_preview(
    title: &'static str,
    subtitle: &'static str,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .text_color(theme.muted_foreground)
        .child(icon_element("film", px(32.0)))
        .child(div().text_size(px(14.0)).child(title))
        .child(div().text_size(px(12.0)).child(subtitle))
        .into_any_element()
}

fn frame_step(frame_rate: f64) -> f64 {
    1.0 / frame_rate.max(1.0)
}

fn playback_tick(frame_rate: f64) -> Duration {
    let frame_rate = if frame_rate.is_finite() && frame_rate > 0.0 {
        frame_rate.min(240.0)
    } else {
        60.0
    };
    Duration::from_secs_f64(1.0 / frame_rate)
}

fn keyboard_sound_file(kind: &str, index: u32) -> Option<PathBuf> {
    let relative = PathBuf::from("public")
        .join("sounds")
        .join("keyboard")
        .join(kind)
        .join(format!("press-{index}.mp3"));
    if relative.is_file() {
        return Some(relative);
    }
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        roots.push(exe);
    }
    for root in roots {
        for ancestor in root.ancestors() {
            let candidate = ancestor.join(&relative);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn play_keyboard_demo_loop(kind: &str, stop: Arc<AtomicBool>) {
    let started = std::time::Instant::now();
    let mut index = 1_u32;
    while started.elapsed() < Duration::from_secs(5) {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if let Some(path) = keyboard_sound_file(kind, index) {
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

    #[gpui::test]
    fn editor_hotkeys_reach_the_focused_window(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let window = cx.add_window(|window, cx| {
            let editor = VideoEditorWindow::new_for_test(None, cx);
            window.focus(&editor.focus_handle);
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

    #[test]
    fn playback_uses_the_source_frame_rate() {
        assert_eq!(playback_tick(30.0), Duration::from_secs_f64(1.0 / 30.0));
        assert_eq!(playback_tick(60.0), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(playback_tick(120.0), Duration::from_secs_f64(1.0 / 120.0));
        assert_eq!(playback_tick(f64::NAN), playback_tick(60.0));
    }

    #[test]
    fn frame_step_uses_the_source_frame_rate() {
        assert_eq!(frame_step(30.0), 1.0 / 30.0);
        assert_eq!(frame_step(60.0), 1.0 / 60.0);
        assert_eq!(frame_step(120.0), 1.0 / 120.0);
    }

    #[test]
    fn keyboard_demo_resolves_bundled_samples() {
        let path = keyboard_sound_file("cherry-blue", 1).expect("bundled keyboard sample");
        assert!(path.ends_with("press-1.mp3"));
        assert!(path.is_file());
    }
}
