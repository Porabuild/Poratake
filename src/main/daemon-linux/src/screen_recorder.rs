//! Screen recording for X11: an X11 frame pump piped as raw video into a
//! distribution FFmpeg subprocess, mirroring the Windows daemon's
//! `screen-recorder` module contract — method ids, response payloads, event
//! names, and error codes.
//!
//! Pausing suspends the frame pump instead of the encoder clock, so rawvideo's
//! frame-index timestamps skip the paused span exactly like the Windows
//! pipeline does; side tracks suspend their encoders with SIGSTOP over the
//! same span, and the input listener (`recording_input`) records the cursor
//! and keystroke timelines over the same span (X11 sessions — XInput2 has no
//! native Wayland counterpart). System and microphone audio and the camera
//! picture-in-picture (`recording_tracks`) are recorded when a Pulse/PipeWire
//! server or a V4L2 device provides the source; every side file is reported
//! in the stop payload like Windows. Wayland sessions pump frames through the
//! ScreenCast portal (`build_frame_source`), so a start there waits for the
//! desktop's consent dialog — the setup runs off the router thread.

use image::RgbaImage;
use poratake_daemon_common::contract::{
    RECORDING_TARGET_CLOSED, SCREEN_RECORDER_MODULE, ScreenRecorderMethod,
    ScreenRecorderMicrophoneRequest, ScreenRecorderStartRequest, ScreenRecorderToggleRequest,
};
use poratake_daemon_common::ffmpeg;
use poratake_daemon_common::geometry::CaptureRect;
use poratake_daemon_common::protocol::{
    Request, params, respond_error, respond_success, send_event,
};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::Backend;
use crate::capture::{
    X11WindowClosed, X11WindowSource, capture_x11_drawable, even_video_dimensions, pad_video_frame,
    x11_region_source,
};
use crate::recording_input::InputRecorder;
use crate::recording_tracks::{Track, TrackKind, TrackPrefs, spawn_tracks};

const STATE_RECORDING: u8 = 1;
const STATE_PAUSED: u8 = 2;

/// How long `stop` waits for FFmpeg to flush and finalize the container.
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `start` waits for the first captured frame before failing. The
/// client's own request window is 60s, so this stays under it while
/// tolerating a cold encoder and a slow first grab (Windows has no cap at
/// all — only the client bounds it).
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(55);
/// How long `stop` waits for the pump thread to acknowledge its exit flag.
const PUMP_EXIT_TIMEOUT: Duration = Duration::from_secs(3);
/// Consecutive per-frame grab failures tolerated (a locked screen can blank
/// the root pixmap briefly) before the session is declared broken.
const MAX_CONSECUTIVE_GRAB_FAILURES: u32 = 30;

pub struct ScreenRecorderModule {
    backend: Backend,
    recording: Arc<Mutex<Option<ActiveRecording>>>,
    /// Bumped per session so deferred workers can tell whether the slot
    /// still holds the session they were started for.
    generation: AtomicU64,
    /// The pump reports a fatally broken session here (dead encoder, no more
    /// frames); whichever method runs next reaps the dead slot and tells the
    /// client.
    fatal: Mutex<Option<Receiver<String>>>,
    /// Settings-surface track toggles, scoped to the running session.
    track_prefs: Arc<TrackPrefs>,
    setup_pending: Arc<Mutex<Option<u64>>>,
}

/// One live recording session. `stop` takes the whole value out of the module
/// slot, so every field is owned by whoever holds the slot.
struct ActiveRecording {
    control: Arc<PumpControl>,
    child: Child,
    /// Shared with the pump thread: `stop` closes the pipe so the pump's next
    /// write fails, and a fatally broken pump closes it so FFmpeg finalizes.
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    output_path: std::path::PathBuf,
    frame_rate: u32,
    generation: u64,
    /// Signalled when the pump — which runs on the session-setup thread —
    /// exits, letting `stop` bound its wait.
    pump_done: Receiver<()>,
    /// Audio tracks and camera picture-in-picture; the recording stays
    /// video-only when none could be started.
    tracks: Vec<Track>,
    /// Passive input capture (cursor + keyboard timelines), `None` when the
    /// display cannot support it.
    input: Option<InputRecorder>,
}

/// Shared between the module (control) and the pump thread (worker).
struct PumpControl {
    state: AtomicU8,
    stop: AtomicBool,
    frames: AtomicU64,
    failures: AtomicU32,
    /// Handed to the `start` completion thread when the first frame lands.
    first_frame: Mutex<Option<Sender<()>>>,
    /// Handed back when the pump thread exits, so `stop` can bound its join.
    done: Mutex<Option<Sender<()>>>,
    pace: Mutex<Instant>,
    pace_changed: Condvar,
}

impl ScreenRecorderModule {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            recording: Arc::new(Mutex::new(None)),
            generation: AtomicU64::new(0),
            fatal: Mutex::new(None),
            track_prefs: Arc::new(TrackPrefs::default()),
            setup_pending: Arc::new(Mutex::new(None)),
        }
    }

    /// If the pump reported a fatally broken session, clear the slot, reap
    /// the encoder, and tell the client — otherwise a dead recording would
    /// report active forever and block every retry with "already active".
    fn reap_if_dead(&self) {
        let message = self
            .fatal
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|fatal| fatal.try_recv().ok());
        let Some(message) = message else {
            return;
        };
        if let Some(active) = take_session(&self.recording, &self.setup_pending, None) {
            self.track_prefs.clear();
            // Only a session that was actually reaped reports an error: a
            // fatal whose session is already gone is stale — stop finalized
            // it or the start deadline tore it down — and the client already
            // knows. Replaying it here would fail the next healthy start.
            send_event(
                "screen-recorder:error",
                Some(json!({ "code": "CAPTURE_ERROR", "message": message })),
            );
            spawn_failed_start_cleanup(active, self.setup_pending.clone(), PUMP_EXIT_TIMEOUT);
        }
    }

    fn start(&self, request: &Request) -> Reply {
        self.reap_if_dead();
        let wire: ScreenRecorderStartRequest = match params(request) {
            Ok(wire) => wire,
            Err((code, message)) => return Reply::Now(Err((code, message))),
        };
        if let Err(message) = wire.validate() {
            return reply_error("INVALID_PARAMS", message);
        }
        if wire.window_id.is_some() && self.backend != Backend::X11 {
            return reply_error(
                "CONFIGURATION_ERROR",
                "window recording is only available in X11 sessions",
            );
        }
        if self.backend == Backend::Headless {
            return reply_error(
                "CONFIGURATION_ERROR",
                "recording needs an X11 or Wayland graphical session",
            );
        }
        if wire.ios_device_id.is_some() {
            eprintln!("[recorder] iOS device overlays are not implemented on Linux; ignoring");
        }
        let Some(ffmpeg_binary) = ffmpeg::resolve_h264() else {
            return reply_error(
                "CONFIGURATION_ERROR",
                "recording on Linux needs FFmpeg with the libx264 encoder on PATH \
                 (or PORATAKE_FFMPEG_PATH)",
            );
        };
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        if !reserve_setup(&self.recording, &self.setup_pending, generation) {
            return reply_error("INVALID_STATE", "a recording is already active");
        }

        let capture_rect = CaptureRect {
            x: wire.x.unwrap_or_default(),
            y: wire.y.unwrap_or_default(),
            width: wire.width.unwrap_or_default(),
            height: wire.height.unwrap_or_default(),
        };
        let output_path = wire.output_path.clone();
        if let Some(parent) = output_path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            clear_setup_pending(&self.setup_pending, generation);
            return reply_error("CONFIGURATION_ERROR", &format!("{error}"));
        }
        let output_directory = output_path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut kinds = Vec::new();
        if self.track_prefs.system_enabled(wire.include_audio) {
            kinds.push(TrackKind::System);
        }
        let microphone_device = if self.track_prefs.microphone_enabled(wire.mic_enabled) {
            kinds.push(TrackKind::Microphone);
            self.track_prefs
                .microphone_device(wire.mic_device_id.as_deref())
        } else {
            None
        };
        let camera_device = if self.track_prefs.camera_enabled(wire.camera_enabled) {
            kinds.push(TrackKind::Camera);
            wire.camera_device_id.clone()
        } else {
            None
        };

        let (first_frame_tx, first_frame_rx) = std::sync::mpsc::channel();
        let (fatal_tx, fatal_rx) = std::sync::mpsc::channel();
        *self.fatal.lock().unwrap() = Some(fatal_rx);
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(Some(first_frame_tx)),
            done: Mutex::new(Some(done_tx)),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });

        // Everything after this point runs off the router thread: building
        // the Wayland frame source can block on the portal consent dialog,
        // and this daemon serves every module from one thread.
        let recording_slot = self.recording.clone();
        let track_prefs = self.track_prefs.clone();
        let setup_pending = self.setup_pending.clone();
        let backend = self.backend;
        let frame_rate = wire.frame_rate;
        let keyboard_enabled = wire.keyboard_enabled;
        let window_id = wire.window_id;
        let request_id = request.id.clone();
        std::thread::spawn(move || {
            // The frame source is backend-specific: X11 grabs the resolved
            // output per frame; Wayland blocks per frame on the portal
            // stream, and opening it waits for the user's consent — exactly
            // why this whole setup runs off the router thread.
            let mut grab_frame: Box<dyn FnMut() -> anyhow::Result<RgbaImage>>;
            let mut window_bounds = None;
            let grab = match backend {
                Backend::X11 => {
                    if let Some(window_id) = window_id {
                        let mut source = match X11WindowSource::new(window_id) {
                            Ok(source) => source,
                            Err(error) => {
                                clear_setup_pending(&setup_pending, generation);
                                respond_error(&request_id, "CAPTURE_ERROR", &format!("{error:#}"));
                                return;
                            }
                        };
                        let grab = source.bounds();
                        window_bounds = Some(source.bounds_tracker());
                        let target = even_video_dimensions((grab.width as u32, grab.height as u32));
                        grab_frame = Box::new(move || source.capture_frame(target));
                        grab
                    } else {
                        let region = match x11_region_source(capture_rect) {
                            Ok(region) => region,
                            Err(error) => {
                                clear_setup_pending(&setup_pending, generation);
                                respond_error(&request_id, "CAPTURE_ERROR", &format!("{error:#}"));
                                return;
                            }
                        };
                        let grab = if capture_rect.has_positive_size() {
                            capture_rect
                        } else {
                            CaptureRect {
                                x: i32::from(region.crtc_x),
                                y: i32::from(region.crtc_y),
                                width: i32::from(region.crtc_width),
                                height: i32::from(region.crtc_height),
                            }
                        };
                        grab_frame = Box::new(move || {
                            let screen = region.screen()?;
                            capture_x11_drawable(
                                &region.connection,
                                screen,
                                xcb::x::Drawable::Window(screen.root()),
                                grab.x as i16,
                                grab.y as i16,
                                grab.width as u16,
                                grab.height as u16,
                            )
                        });
                        grab
                    }
                }
                Backend::Wayland => {
                    #[cfg(feature = "wayland")]
                    {
                        let mut capturer = match crate::capture::wayland_capturer(frame_rate) {
                            Ok(capturer) => capturer,
                            Err(error) => {
                                clear_setup_pending(&setup_pending, generation);
                                respond_error(
                                    &request_id,
                                    "CONFIGURATION_ERROR",
                                    &format!("{error:#}"),
                                );
                                return;
                            }
                        };
                        let [width, height] = capturer.get_output_frame_size();
                        grab_frame = Box::new(move || {
                            crate::capture::frame_to_rgba(capturer.get_next_frame().map_err(
                                |error| anyhow::anyhow!("the Wayland frame stream failed: {error}"),
                            )?)
                        });
                        CaptureRect {
                            x: 0,
                            y: 0,
                            width: width as i32,
                            height: height as i32,
                        }
                    }
                    #[cfg(not(feature = "wayland"))]
                    {
                        clear_setup_pending(&setup_pending, generation);
                        respond_error(
                            &request_id,
                            "CONFIGURATION_ERROR",
                            "Wayland capture support was not included in this build",
                        );
                        return;
                    }
                }
                Backend::Headless => {
                    clear_setup_pending(&setup_pending, generation);
                    respond_error(
                        &request_id,
                        "CONFIGURATION_ERROR",
                        "recording needs an X11 or Wayland graphical session",
                    );
                    return;
                }
            };
            let encoded_size = even_video_dimensions((grab.width as u32, grab.height as u32));
            let video_grab = CaptureRect {
                width: encoded_size.0 as i32,
                height: encoded_size.1 as i32,
                ..grab
            };
            let mut frame_source = grab_frame;
            grab_frame = Box::new(move || pad_video_frame(frame_source()?, encoded_size));
            let Some((child, stdin)) = spawn_ffmpeg(
                &ffmpeg_binary,
                encoded_size.0,
                encoded_size.1,
                frame_rate,
                &output_path,
            ) else {
                clear_setup_pending(&setup_pending, generation);
                respond_error(
                    &request_id,
                    "START_FAILED",
                    "could not spawn the FFmpeg encoder",
                );
                return;
            };

            // Side tracks spawn before the video pump so their first sample
            // does not lag the first frame — the editor muxes both at offset
            // 0. A machine without a Pulse/PipeWire server or camera (or the
            // requested device) gets a recording without that track instead
            // of a failed start.
            let tracks = spawn_tracks(
                &ffmpeg_binary,
                &kinds,
                microphone_device.as_deref(),
                camera_device.as_deref(),
                frame_rate,
                &output_directory,
            );
            // Passive input capture mirrors the Windows tracker; XInput2 does
            // not exist on native Wayland, so sessions there record without
            // the cursor/keystroke timelines.
            let input = if backend == Backend::X11 {
                InputRecorder::start(grab, video_grab, keyboard_enabled, window_bounds)
            } else {
                None
            };

            if let Some(input) = input.as_ref() {
                input.sync_origin();
            }

            let pump_stdin = stdin.clone();

            let mut recording = recording_slot.lock().unwrap();
            *recording = Some(ActiveRecording {
                control: control.clone(),
                child,
                stdin,
                output_path: output_path.clone(),
                frame_rate,
                generation,
                pump_done: done_rx,
                tracks,
                input,
            });
            clear_setup_pending(&setup_pending, generation);
            drop(recording);

            let completion_request_id = request_id.clone();
            let completion_slot = recording_slot.clone();
            let completion_prefs = track_prefs.clone();
            let completion_output = output_path.clone();
            let completion_setup_pending = setup_pending.clone();
            std::thread::Builder::new()
                .name("linux-recorder-started".into())
                .spawn(move || {
                    let started = match first_frame_rx.recv_timeout(FIRST_FRAME_TIMEOUT) {
                        Ok(()) => true,
                        Err(_) => {
                            // A frame landing exactly at the deadline loses
                            // the channel race but not the session; only a
                            // stillborn session still occupying the slot
                            // counts as failure.
                            completion_slot
                                .lock()
                                .unwrap()
                                .as_ref()
                                .is_some_and(|active| {
                                    active.generation == generation
                                        && active.control.frames.load(Ordering::SeqCst) > 0
                                })
                        }
                    };
                    let response_output = completion_output.to_string_lossy().into_owned();
                    if started {
                        respond_success(
                            &completion_request_id,
                            json!({
                                "success": true,
                                "state": "recording",
                                "message": "Recording started",
                                "outputPath": response_output,
                            }),
                        );
                        send_event(
                            "screen-recorder:started",
                            Some(json!({ "outputPath": response_output })),
                        );
                        return;
                    }
                    let Some(active) = take_session(
                        &completion_slot,
                        &completion_setup_pending,
                        Some(generation),
                    ) else {
                        return;
                    };
                    completion_prefs.clear();
                    respond_error(
                        &completion_request_id,
                        "START_FAILED",
                        "Recorder worker stopped before the first frame",
                    );
                    cleanup_failed_start(active, completion_setup_pending, PUMP_EXIT_TIMEOUT);
                })
                .ok();

            // This thread IS the pump: it parks per frame (X11 grab or
            // portal stream) and exits when stop or a fatal error closes
            // the pipe.
            run_pump(grab_frame, pump_stdin, control, frame_rate, fatal_tx);
        });

        Reply::Deferred
    }

    fn pause(&self, request: &Request) -> Reply {
        self.reap_if_dead();
        let mut slot = self.recording.lock().unwrap();
        let Some(active) = slot.as_mut() else {
            return reply_error("INVALID_STATE", "no recording is active");
        };
        if active
            .control
            .state
            .compare_exchange(
                STATE_RECORDING,
                STATE_PAUSED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return reply_error("INVALID_STATE", "the recording is not running");
        }
        active.control.pace_changed.notify_all();
        // Suspending the track processes skips the paused span the same way
        // the frame-indexed video does.
        for track in &mut active.tracks {
            track.pause();
        }
        if let Some(input) = active.input.as_ref() {
            input.pause();
        }
        let duration = elapsed_seconds(active);
        respond_success(
            &request.id,
            json!({
                "success": true,
                "state": "paused",
                "message": "Recording paused",
                "duration": duration,
            }),
        );
        send_event(
            "screen-recorder:paused",
            Some(json!({ "duration": duration })),
        );
        Reply::Deferred
    }

    fn resume(&self, request: &Request) -> Reply {
        self.reap_if_dead();
        let mut slot = self.recording.lock().unwrap();
        let Some(active) = slot.as_mut() else {
            return reply_error("INVALID_STATE", "no recording is active");
        };
        if active
            .control
            .state
            .compare_exchange(
                STATE_PAUSED,
                STATE_RECORDING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return reply_error("INVALID_STATE", "the recording is not paused");
        }
        *active.control.pace.lock().unwrap() = Instant::now();
        active.control.pace_changed.notify_all();
        for track in &mut active.tracks {
            track.resume();
        }
        if let Some(input) = active.input.as_ref() {
            input.resume();
        }
        let duration = elapsed_seconds(active);
        respond_success(
            &request.id,
            json!({
                "success": true,
                "state": "recording",
                "message": "Recording resumed",
                "duration": duration,
            }),
        );
        send_event(
            "screen-recorder:resumed",
            Some(json!({ "duration": duration })),
        );
        Reply::Deferred
    }

    fn stop(&self, request: &Request) -> Reply {
        self.reap_if_dead();
        let Some(active) = take_session(&self.recording, &self.setup_pending, None) else {
            return reply_error("INVALID_STATE", "no recording is active");
        };
        // The session is over: the toggles it set must not shape the next one.
        self.track_prefs.clear();
        // Finalizing runs off the router thread: the pipe handoff and the
        // encoder wait can block, and this daemon serves every module from
        // one thread. Windows defers its stop the same way.
        let request_id = request.id.clone();
        let setup_pending = self.setup_pending.clone();
        std::thread::spawn(move || finalize_stop(active, request_id, setup_pending));
        Reply::Deferred
    }

    fn status(&self, _request: &Request) -> Reply {
        self.reap_if_dead();
        let recording = self.recording.lock().unwrap();
        // A session that has not delivered its first frame yet may still be
        // torn down by its own start deadline, so it does not count as live.
        let payload = match recording
            .as_ref()
            .filter(|active| active.control.frames.load(Ordering::SeqCst) > 0)
        {
            None => json!({ "state": "idle", "duration": 0 }),
            Some(active) => json!({
                "state": state_str(active.control.state.load(Ordering::SeqCst)),
                "duration": elapsed_seconds(active),
            }),
        };
        Reply::Now(Ok(Some(payload)))
    }

    fn set_microphone(&self, request: &Request) -> Reply {
        let Ok(params) = params::<ScreenRecorderMicrophoneRequest>(request) else {
            return reply_error("INVALID_PARAMS", "invalid microphone params");
        };
        self.track_prefs
            .set_microphone(params.enabled, params.device_id);
        toggle_reply(params.enabled)
    }

    fn set_system_audio(&self, request: &Request) -> Reply {
        let Ok(params) = params::<ScreenRecorderToggleRequest>(request) else {
            return reply_error("INVALID_PARAMS", "invalid toggle params");
        };
        self.track_prefs.set_system(params.enabled);
        toggle_reply(params.enabled)
    }

    fn set_camera(&self, request: &Request) -> Reply {
        let Ok(params) = params::<ScreenRecorderToggleRequest>(request) else {
            return reply_error("INVALID_PARAMS", "invalid toggle params");
        };
        self.track_prefs.set_camera(params.enabled);
        toggle_reply(params.enabled)
    }
}

impl Module for ScreenRecorderModule {
    fn name(&self) -> &'static str {
        SCREEN_RECORDER_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScreenRecorderMethod::parse(&request.method) {
            Some(ScreenRecorderMethod::Start) => self.start(request),
            Some(ScreenRecorderMethod::Pause) => self.pause(request),
            Some(ScreenRecorderMethod::Resume) => self.resume(request),
            Some(ScreenRecorderMethod::Stop) => self.stop(request),
            Some(ScreenRecorderMethod::Status) => self.status(request),
            Some(ScreenRecorderMethod::SetMicrophone) => self.set_microphone(request),
            Some(ScreenRecorderMethod::SetSystemAudio) => self.set_system_audio(request),
            Some(ScreenRecorderMethod::SetCamera) => self.set_camera(request),
            None => method_not_found(&request.method),
        }
    }
}

fn take_session(
    slot: &Mutex<Option<ActiveRecording>>,
    setup_pending: &Mutex<Option<u64>>,
    generation: Option<u64>,
) -> Option<ActiveRecording> {
    let mut guard = slot.lock().unwrap();
    let active = guard.as_ref()?;
    if generation.is_some_and(|generation| active.generation != generation) {
        return None;
    }
    *setup_pending.lock().unwrap() = Some(active.generation);
    guard.take()
}

fn reserve_setup(
    slot: &Mutex<Option<ActiveRecording>>,
    setup_pending: &Mutex<Option<u64>>,
    generation: u64,
) -> bool {
    let recording = slot.lock().unwrap();
    let mut pending = setup_pending.lock().unwrap();
    if recording.is_some() || pending.is_some() {
        return false;
    }
    *pending = Some(generation);
    true
}

fn clear_setup_pending(setup_pending: &Mutex<Option<u64>>, generation: u64) {
    let mut pending = setup_pending.lock().unwrap();
    if *pending == Some(generation) {
        *pending = None;
    }
}

fn finalize_stop(
    mut active: ActiveRecording,
    request_id: String,
    setup_pending: Arc<Mutex<Option<u64>>>,
) {
    signal_pump_stop(&active.control);
    for track in &mut active.tracks {
        track.request_finalize();
    }
    let (pump_finished, finalized) = finish_pump_and_encoder(
        &active.pump_done,
        &active.stdin,
        &mut active.child,
        PUMP_EXIT_TIMEOUT,
        FINALIZE_TIMEOUT,
    );
    for track in &mut active.tracks {
        track.wait_finalized();
    }
    let track_path = |kind: TrackKind| {
        active
            .tracks
            .iter()
            .find(|track| track.kind == kind)
            .map(|track| track.path.to_string_lossy().into_owned())
    };
    let system_audio_path = track_path(TrackKind::System);
    let mic_audio_path = track_path(TrackKind::Microphone);
    let camera_path = track_path(TrackKind::Camera);
    let duration = elapsed_seconds(&active);
    let output_directory = active
        .output_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let input_files = match active.input.as_mut() {
        Some(input) => input.finish(duration, &output_directory).ok(),
        None => Some((None, None)),
    };
    if !finalized || !pump_finished || input_files.is_none() {
        if pump_finished {
            clear_setup_pending(&setup_pending, active.generation);
        } else {
            release_setup_when_pump_stops(
                active.pump_done,
                setup_pending,
                active.generation,
                Duration::ZERO,
            );
        }
        respond_error(
            &request_id,
            "STOP_FAILED",
            "the recorder did not finalize the recording",
        );
        return;
    }
    let (cursor_path, keys_path) = input_files.unwrap_or((None, None));
    let output_path = active.output_path.to_string_lossy().into_owned();
    clear_setup_pending(&setup_pending, active.generation);
    respond_success(
        &request_id,
        json!({
            "success": true,
            "state": "idle",
            "message": "Recording stopped",
            "outputPath": output_path,
            "cursorPath": cursor_path,
            "cameraPath": camera_path,
            "keysPath": keys_path,
            "systemAudioPath": system_audio_path,
            "micAudioPath": mic_audio_path,
            "duration": duration,
        }),
    );
    send_event(
        "screen-recorder:stopped",
        Some(json!({
            "outputPath": output_path,
            "cursorPath": cursor_path,
            "cameraPath": camera_path,
            "keysPath": keys_path,
            "systemAudioPath": system_audio_path,
            "micAudioPath": mic_audio_path,
            "duration": duration,
        })),
    );
}

fn signal_pump_stop(control: &PumpControl) {
    control.stop.store(true, Ordering::SeqCst);
    control.pace_changed.notify_all();
}

fn cleanup_failed_start(
    mut active: ActiveRecording,
    setup_pending: Arc<Mutex<Option<u64>>>,
    pump_timeout: Duration,
) {
    let generation = active.generation;
    signal_pump_stop(&active.control);
    let _ = active.child.kill();
    if let Some(input) = active.input.as_mut() {
        input.shutdown();
    }
    for track in &mut active.tracks {
        track.kill();
        let _ = std::fs::remove_file(&track.path);
    }
    let _ = ffmpeg::wait_for_exit(&mut active.child, pump_timeout);
    release_setup_when_pump_stops(active.pump_done, setup_pending, generation, pump_timeout);
}

fn spawn_failed_start_cleanup(
    active: ActiveRecording,
    setup_pending: Arc<Mutex<Option<u64>>>,
    pump_timeout: Duration,
) {
    std::thread::spawn(move || cleanup_failed_start(active, setup_pending, pump_timeout));
}

fn release_setup_when_pump_stops(
    pump_done: Receiver<()>,
    setup_pending: Arc<Mutex<Option<u64>>>,
    generation: u64,
    timeout: Duration,
) {
    if matches!(
        pump_done.recv_timeout(timeout),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ) {
        let _ = std::thread::Builder::new()
            .name("linux-recorder-worker-cleanup".into())
            .spawn(move || {
                let _ = pump_done.recv();
                clear_setup_pending(&setup_pending, generation);
            });
        return;
    }
    clear_setup_pending(&setup_pending, generation);
}

fn finish_pump_and_encoder(
    pump_done: &Receiver<()>,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    child: &mut Child,
    pump_timeout: Duration,
    encoder_timeout: Duration,
) -> (bool, bool) {
    let pump_finished = match pump_done.recv_timeout(pump_timeout) {
        Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => true,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            let _ = child.kill();
            !matches!(
                pump_done.recv_timeout(pump_timeout),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            )
        }
    };
    let pipe = match stdin.try_lock() {
        Ok(mut guard) => guard.take(),
        Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner().take(),
        Err(std::sync::TryLockError::WouldBlock) => None,
    };
    drop(pipe);
    let encoder_finished = ffmpeg::wait_for_exit(child, encoder_timeout);
    (pump_finished, encoder_finished)
}

/// Drives one recording session's frame pump: grabs a frame, feeds it to the
/// encoder pipe, and paces or pauses on the shared control state. Runs on the
/// session-setup thread until stop or a fatal error closes the pipe.
fn run_pump(
    mut grab_frame: Box<dyn FnMut() -> anyhow::Result<RgbaImage>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    control: Arc<PumpControl>,
    frame_rate: u32,
    fatal: Sender<String>,
) {
    let frame_interval = Duration::from_secs_f64(1.0 / f64::from(frame_rate.max(1)));

    loop {
        if control.stop.load(Ordering::SeqCst) {
            break;
        }
        // A paused session parks here until it resumes or stops; the pacing
        // deadline restarts afterwards so no stale burst is emitted.
        {
            let mut next_at = control.pace.lock().unwrap();
            while control.state.load(Ordering::SeqCst) == STATE_PAUSED
                && !control.stop.load(Ordering::SeqCst)
            {
                next_at = control.pace_changed.wait(next_at).unwrap();
            }
            if control.stop.load(Ordering::SeqCst) {
                break;
            }
            let now = Instant::now();
            if *next_at < now {
                *next_at = now;
            }
            let wait = next_at.saturating_duration_since(now);
            if !wait.is_zero() {
                let (guard, _) = control.pace_changed.wait_timeout(next_at, wait).unwrap();
                next_at = guard;
                if control.state.load(Ordering::SeqCst) == STATE_PAUSED
                    || control.stop.load(Ordering::SeqCst)
                {
                    continue;
                }
            }
        }

        let frame = grab_frame();
        match frame {
            Ok(image) => {
                let mut pipe = stdin.lock().unwrap();
                match pipe.as_mut() {
                    Some(pipe) => {
                        if pipe.write_all(image.as_raw()).is_err() {
                            // The encoder exited (crash, OOM, a failed
                            // output write); report so the session does not
                            // linger as active forever.
                            let _ = fatal
                                .send("the encoder stopped before the recording finished".into());
                            // Fail a still-pending start immediately instead
                            // of at the first-frame deadline.
                            control.first_frame.lock().unwrap().take();
                            break;
                        }
                        control.failures.store(0, Ordering::SeqCst);
                        if control.frames.fetch_add(1, Ordering::SeqCst) == 0
                            && let Some(sender) = control.first_frame.lock().unwrap().take()
                        {
                            let _ = sender.send(());
                        }
                    }
                    None => break, // stop() closed the pipe.
                }
                *control.pace.lock().unwrap() += frame_interval;
            }
            Err(error) => {
                if error.downcast_ref::<X11WindowClosed>().is_some() {
                    control.first_frame.lock().unwrap().take();
                    if control.frames.load(Ordering::SeqCst) > 0 {
                        send_event(
                            "screen-recorder:error",
                            Some(json!({
                                "code": RECORDING_TARGET_CLOSED,
                                "message": "The recorded window was closed",
                            })),
                        );
                    }
                    break;
                }
                eprintln!("[recorder] frame grab failed: {error:#}");
                *control.pace.lock().unwrap() += frame_interval;
                let failures = control.failures.fetch_add(1, Ordering::SeqCst) + 1;
                if failures >= MAX_CONSECUTIVE_GRAB_FAILURES {
                    let _ = fatal.send("the frame pump stopped delivering frames".into());
                    // Fail a still-pending start immediately instead of at
                    // the first-frame deadline.
                    control.first_frame.lock().unwrap().take();
                    // Close the pipe so FFmpeg finalizes what it has.
                    drop(stdin.lock().unwrap().take());
                    break;
                }
            }
        }
    }
    if let Some(done) = control.done.lock().unwrap().take() {
        let _ = done.send(());
    }
}

fn spawn_ffmpeg(
    binary: &std::path::Path,
    width: u32,
    height: u32,
    frame_rate: u32,
    output_path: &std::path::Path,
) -> Option<(Child, Arc<Mutex<Option<ChildStdin>>>)> {
    let mut args = ffmpeg::quiet_args();
    args.extend(ffmpeg::raw_video_input_args(
        "rgba", width, height, frame_rate,
    ));
    args.extend(ffmpeg::h264_encode_args(ffmpeg::VideoRate::Crf(23)));
    args.push("-y".into());
    let mut command = Command::new(binary);
    command
        .args(&args)
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = command.spawn().ok()?;
    let stdin = child.stdin.take()?;
    let stderr = child.stderr.take()?;
    let stdin = Arc::new(Mutex::new(Some(stdin)));
    ffmpeg::spawn_stderr_tail(stderr, |line| eprintln!("[recorder] ffmpeg: {line}"));
    Some((child, stdin))
}

fn state_str(state: u8) -> &'static str {
    match state {
        STATE_RECORDING => "recording",
        STATE_PAUSED => "paused",
        _ => "idle",
    }
}

fn elapsed_seconds(active: &ActiveRecording) -> f64 {
    let frames = active.control.frames.load(Ordering::SeqCst);
    frames as f64 / f64::from(active.frame_rate.max(1))
}

fn toggle_reply(enabled: bool) -> Reply {
    Reply::Now(Ok(Some(json!({ "success": true, "enabled": enabled }))))
}

fn reply_error(code: &str, message: &str) -> Reply {
    Reply::Now(Err((code.to_string(), message.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn states_render_like_the_windows_contract() {
        assert_eq!(state_str(0), "idle");
        assert_eq!(state_str(STATE_RECORDING), "recording");
        assert_eq!(state_str(STATE_PAUSED), "paused");
    }

    #[test]
    fn duration_is_the_encoded_frame_count_at_the_session_rate() {
        let control = PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(150),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        };
        let active = ActiveRecording {
            control: Arc::new(control),
            child: ffmpeg_placeholder_child(),
            stdin: Arc::new(Mutex::new(None)),
            output_path: std::path::PathBuf::from("/tmp/recording.mov"),
            frame_rate: 30,
            generation: 0,
            pump_done: std::sync::mpsc::channel().1,
            tracks: Vec::new(),
            input: None,
        };
        assert_eq!(elapsed_seconds(&active), 5.0);
    }

    #[test]
    fn paces_a_transient_capture_failure_and_recovers_on_the_next_frame() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn raw-frame sink");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let attempts = Arc::new(AtomicU32::new(0));
        let frame_control = control.clone();
        let frame_attempts = attempts.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let started_at = Instant::now();

        run_pump(
            Box::new(move || {
                if frame_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Err(anyhow::anyhow!("transient capture failure"));
                }
                frame_control.stop.store(true, Ordering::SeqCst);
                Ok(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])))
            }),
            stdin.clone(),
            control.clone(),
            30,
            fatal_sender,
        );

        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert!(started_at.elapsed() >= Duration::from_millis(20));
        done_receiver.recv().expect("pump completion");
        drop(stdin.lock().unwrap().take());
        assert!(child.wait().expect("wait for raw-frame sink").success());
    }

    #[test]
    fn stop_waits_for_an_in_flight_frame_before_closing_encoder_input() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn raw-frame sink");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let (frame_started_sender, frame_started_receiver) = std::sync::mpsc::channel();
        let (continue_frame_sender, continue_frame_receiver) = std::sync::mpsc::channel();
        let pump_control = control.clone();
        let pump_stdin = stdin.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let pump = std::thread::spawn(move || {
            run_pump(
                Box::new(move || {
                    frame_started_sender.send(()).expect("signal frame start");
                    continue_frame_receiver
                        .recv()
                        .expect("allow in-flight frame to finish");
                    Ok(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])))
                }),
                pump_stdin,
                pump_control,
                30,
                fatal_sender,
            )
        });
        frame_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("frame pump entered capture");

        let stop_control = control.clone();
        let stop_stdin = stdin.clone();
        let stop = std::thread::spawn(move || {
            signal_pump_stop(&stop_control);
            finish_pump_and_encoder(
                &done_receiver,
                &stop_stdin,
                &mut child,
                Duration::from_secs(1),
                Duration::from_secs(1),
            )
        });
        let stop_deadline = Instant::now() + Duration::from_secs(1);
        while !control.stop.load(Ordering::SeqCst) {
            assert!(Instant::now() < stop_deadline, "stop was not signalled");
            std::thread::yield_now();
        }
        continue_frame_sender
            .send(())
            .expect("release in-flight frame");

        let (pump_finished, encoder_finished) = stop.join().expect("stop completion");
        pump.join().expect("frame pump completion");

        assert!(pump_finished);
        assert!(encoder_finished);
        assert_eq!(control.frames.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn stop_kills_a_blocked_encoder_and_rejects_its_failed_exit() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn blocked encoder");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let (frame_started_sender, frame_started_receiver) = std::sync::mpsc::channel();
        let pump_control = control.clone();
        let pump_stdin = stdin.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let pump = std::thread::spawn(move || {
            run_pump(
                Box::new(move || {
                    frame_started_sender.send(()).expect("signal frame start");
                    Ok(RgbaImage::from_pixel(2048, 512, Rgba([1, 2, 3, 255])))
                }),
                pump_stdin,
                pump_control,
                30,
                fatal_sender,
            )
        });
        frame_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("frame pump entered capture");
        let write_deadline = Instant::now() + Duration::from_secs(1);
        let writer_blocked = loop {
            match stdin.try_lock() {
                Ok(guard) => drop(guard),
                Err(std::sync::TryLockError::WouldBlock) => break true,
                Err(std::sync::TryLockError::Poisoned(error)) => drop(error.into_inner()),
            }
            if Instant::now() >= write_deadline {
                break false;
            }
            std::thread::yield_now();
        };

        signal_pump_stop(&control);
        let (pump_finished, encoder_finished) = finish_pump_and_encoder(
            &done_receiver,
            &stdin,
            &mut child,
            Duration::from_millis(50),
            Duration::from_millis(50),
        );
        pump.join().expect("frame pump completion");

        assert!(writer_blocked, "frame writer never blocked on the encoder");
        assert!(pump_finished);
        assert!(!encoder_finished);
        assert_eq!(control.frames.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stop_rejects_an_encoder_that_exits_with_a_nonzero_status() {
        let control = PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(1),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        };
        let (_, done_receiver) = std::sync::mpsc::channel();
        let mut child = Command::new("sh")
            .args(["-c", "exit 23"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn failed encoder");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));

        signal_pump_stop(&control);
        let (pump_finished, encoder_finished) = finish_pump_and_encoder(
            &done_receiver,
            &stdin,
            &mut child,
            Duration::from_millis(50),
            Duration::from_secs(1),
        );

        assert!(pump_finished);
        assert!(!encoder_finished);
    }

    #[test]
    fn failed_start_kills_a_blocked_encoder_before_waiting_for_its_pipe_lock() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn blocked encoder");
        let child_id = child.id();
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let (frame_started_sender, frame_started_receiver) = std::sync::mpsc::channel();
        let pump_control = control.clone();
        let pump_stdin = stdin.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let pump = std::thread::spawn(move || {
            run_pump(
                Box::new(move || {
                    frame_started_sender.send(()).expect("signal frame start");
                    Ok(RgbaImage::from_pixel(2048, 512, Rgba([1, 2, 3, 255])))
                }),
                pump_stdin,
                pump_control,
                30,
                fatal_sender,
            )
        });
        frame_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("frame pump entered capture");
        let write_deadline = Instant::now() + Duration::from_secs(1);
        let writer_blocked = loop {
            match stdin.try_lock() {
                Ok(guard) => drop(guard),
                Err(std::sync::TryLockError::WouldBlock) => break true,
                Err(std::sync::TryLockError::Poisoned(error)) => drop(error.into_inner()),
            }
            if Instant::now() >= write_deadline {
                break false;
            }
            std::thread::yield_now();
        };
        let setup_pending = Arc::new(Mutex::new(Some(0)));
        let active = ActiveRecording {
            control,
            child,
            stdin,
            output_path: std::path::PathBuf::from("/tmp/failed-start.mp4"),
            frame_rate: 30,
            generation: 0,
            pump_done: done_receiver,
            tracks: Vec::new(),
            input: None,
        };
        let pending_for_cleanup = setup_pending.clone();
        let (cleanup_done_sender, cleanup_done_receiver) = std::sync::mpsc::channel();
        let cleanup = std::thread::spawn(move || {
            cleanup_failed_start(active, pending_for_cleanup, Duration::from_millis(100));
            cleanup_done_sender
                .send(())
                .expect("signal cleanup completion");
        });
        let cleanup_finished = cleanup_done_receiver
            .recv_timeout(Duration::from_secs(1))
            .is_ok();
        if !cleanup_finished {
            unsafe {
                libc::kill(child_id as libc::pid_t, libc::SIGKILL);
            }
        }
        cleanup.join().expect("failed-start cleanup");
        pump.join().expect("frame pump completion");

        assert!(writer_blocked, "frame writer never blocked on the encoder");
        assert!(
            cleanup_finished,
            "failed-start cleanup blocked on pipe lock"
        );
        assert_eq!(*setup_pending.lock().unwrap(), None);
    }

    #[test]
    fn failed_start_cleanup_runs_off_thread_and_keeps_retry_blocked_until_worker_exits() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn encoder");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let (frame_started_sender, frame_started_receiver) = std::sync::mpsc::channel();
        let (continue_frame_sender, continue_frame_receiver) = std::sync::mpsc::channel();
        let pump_control = control.clone();
        let pump_stdin = stdin.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let pump = std::thread::spawn(move || {
            run_pump(
                Box::new(move || {
                    frame_started_sender.send(()).expect("signal frame start");
                    continue_frame_receiver
                        .recv()
                        .expect("release blocked frame capture");
                    Ok(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])))
                }),
                pump_stdin,
                pump_control,
                30,
                fatal_sender,
            )
        });
        frame_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("frame pump entered capture");
        let setup_pending = Arc::new(Mutex::new(Some(0)));
        spawn_failed_start_cleanup(
            ActiveRecording {
                control,
                child,
                stdin,
                output_path: std::path::PathBuf::from("/tmp/stuck-start.mp4"),
                frame_rate: 30,
                generation: 0,
                pump_done: done_receiver,
                tracks: Vec::new(),
                input: None,
            },
            setup_pending.clone(),
            Duration::from_millis(50),
        );
        assert_eq!(*setup_pending.lock().unwrap(), Some(0));
        continue_frame_sender
            .send(())
            .expect("release blocked frame");
        let deadline = Instant::now() + Duration::from_secs(1);
        while *setup_pending.lock().unwrap() == Some(0) {
            assert!(
                Instant::now() < deadline,
                "worker cleanup did not release retry gate"
            );
            std::thread::yield_now();
        }
        pump.join().expect("frame pump completion");
    }

    #[test]
    fn stop_keeps_retries_blocked_until_a_timed_out_pump_exits() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        *control.done.lock().unwrap() = Some(done_sender);
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn encoder");
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let (frame_started_sender, frame_started_receiver) = std::sync::mpsc::channel();
        let (continue_frame_sender, continue_frame_receiver) = std::sync::mpsc::channel();
        let pump_control = control.clone();
        let pump_stdin = stdin.clone();
        let (fatal_sender, _fatal_receiver) = std::sync::mpsc::channel();
        let pump = std::thread::spawn(move || {
            run_pump(
                Box::new(move || {
                    frame_started_sender.send(()).expect("signal frame start");
                    continue_frame_receiver
                        .recv()
                        .expect("release blocked frame capture");
                    Ok(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])))
                }),
                pump_stdin,
                pump_control,
                30,
                fatal_sender,
            )
        });
        frame_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("frame pump entered capture");
        let setup_pending = Arc::new(Mutex::new(Some(1)));
        signal_pump_stop(&control);
        let (pump_finished, encoder_finished) = finish_pump_and_encoder(
            &done_receiver,
            &stdin,
            &mut child,
            Duration::from_millis(50),
            Duration::from_millis(50),
        );
        assert!(!pump_finished);
        assert!(!encoder_finished);
        release_setup_when_pump_stops(done_receiver, setup_pending.clone(), 1, Duration::ZERO);
        assert_eq!(*setup_pending.lock().unwrap(), Some(1));
        continue_frame_sender
            .send(())
            .expect("release blocked frame");
        let deadline = Instant::now() + Duration::from_secs(1);
        while *setup_pending.lock().unwrap() == Some(1) {
            assert!(
                Instant::now() < deadline,
                "timed-out stop did not release retry gate after worker exit"
            );
            std::thread::yield_now();
        }
        pump.join().expect("frame pump completion");
    }

    #[test]
    fn session_transfer_blocks_retries_until_finalization_and_scopes_gate_release() {
        let control = Arc::new(PumpControl {
            state: AtomicU8::new(STATE_RECORDING),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(1),
            failures: AtomicU32::new(0),
            first_frame: Mutex::new(None),
            done: Mutex::new(None),
            pace: Mutex::new(Instant::now()),
            pace_changed: Condvar::new(),
        });
        let recording = Arc::new(Mutex::new(Some(ActiveRecording {
            control,
            child: ffmpeg_placeholder_child(),
            stdin: Arc::new(Mutex::new(None)),
            output_path: std::path::PathBuf::from("/tmp/same-output.mp4"),
            frame_rate: 30,
            generation: 1,
            pump_done: std::sync::mpsc::channel().1,
            tracks: Vec::new(),
            input: None,
        })));
        let setup_pending = Arc::new(Mutex::new(None));
        let (taken_sender, taken_receiver) = std::sync::mpsc::channel();
        let (finalize_sender, finalize_receiver) = std::sync::mpsc::channel();
        let cleanup_recording = recording.clone();
        let cleanup_gate = setup_pending.clone();
        let cleanup = std::thread::spawn(move || {
            let mut active = take_session(&cleanup_recording, &cleanup_gate, Some(1))
                .expect("take session for stop finalization");
            taken_sender.send(()).expect("signal session transfer");
            finalize_receiver
                .recv()
                .expect("finish old output before releasing reservation");
            clear_setup_pending(&cleanup_gate, active.generation);
            active.child.wait().expect("wait for completed encoder");
        });
        taken_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("session transfer");

        assert!(!reserve_setup(&recording, &setup_pending, 2));
        assert_eq!(*setup_pending.lock().unwrap(), Some(1));
        finalize_sender.send(()).expect("release old reservation");
        cleanup.join().expect("old session finalization");

        assert!(reserve_setup(&recording, &setup_pending, 2));
        clear_setup_pending(&setup_pending, 1);
        assert_eq!(*setup_pending.lock().unwrap(), Some(2));
    }

    fn ffmpeg_placeholder_child() -> Child {
        // The duration math never touches the child, so a shell no-op is a
        // cheap stand-in for a real encoder process in tests.
        Command::new("true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn true")
    }
}
