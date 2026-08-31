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

use anyhow::anyhow;
use image::RgbaImage;
use poratake_daemon_common::contract::{
    SCREEN_RECORDER_MODULE, ScreenRecorderMethod, ScreenRecorderMicrophoneRequest,
    ScreenRecorderStartRequest, ScreenRecorderToggleRequest,
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
use crate::capture::{capture_x11_drawable, x11_region_source};
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
    /// True while a session is being set up off-thread (a Wayland portal
    /// consent dialog can park the setup for a while); lets start reject a
    /// second start that races the first one's slow setup.
    setup_pending: Arc<AtomicBool>,
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
            setup_pending: Arc::new(AtomicBool::new(false)),
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
        if let Some(mut active) = self.recording.lock().unwrap().take() {
            self.track_prefs.clear();
            active.control.stop.store(true, Ordering::SeqCst);
            active.control.pace_changed.notify_all();
            drop(active.stdin.lock().unwrap().take());
            if let Some(input) = active.input.as_mut() {
                input.shutdown();
            }
            for track in &mut active.tracks {
                track.kill();
                let _ = std::fs::remove_file(&track.path);
            }
            let _ = active.child.kill();
            let _ = active.child.wait();
            // Only a session that was actually reaped reports an error: a
            // fatal whose session is already gone is stale — stop finalized
            // it or the start deadline tore it down — and the client already
            // knows. Replaying it here would fail the next healthy start.
            send_event(
                "screen-recorder:error",
                Some(json!({ "code": "CAPTURE_ERROR", "message": message })),
            );
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
        if wire.window_id.is_some() {
            return reply_error(
                "CONFIGURATION_ERROR",
                "window recording is not available on Linux yet",
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
        // The setup window counts as active: a Wayland portal consent dialog
        // can park the setup for a while, and a second start must not race it.
        if self.setup_pending.load(Ordering::SeqCst) || self.recording.lock().unwrap().is_some() {
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

        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
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

        self.setup_pending.store(true, Ordering::SeqCst);
        // Everything after this point runs off the router thread: building
        // the Wayland frame source can block on the portal consent dialog,
        // and this daemon serves every module from one thread.
        let recording_slot = self.recording.clone();
        let track_prefs = self.track_prefs.clone();
        let setup_pending = self.setup_pending.clone();
        let backend = self.backend;
        let frame_rate = wire.frame_rate;
        let keyboard_enabled = wire.keyboard_enabled;
        let request_id = request.id.clone();
        std::thread::spawn(move || {
            // The frame source is backend-specific: X11 grabs the resolved
            // output per frame; Wayland blocks per frame on the portal
            // stream, and opening it waits for the user's consent — exactly
            // why this whole setup runs off the router thread.
            let grab_frame: Box<dyn FnMut() -> anyhow::Result<RgbaImage>>;
            let grab = match backend {
                Backend::X11 => {
                    let region = match x11_region_source(capture_rect) {
                        Ok(region) => region,
                        Err(error) => {
                            setup_pending.store(false, Ordering::SeqCst);
                            respond_error(&request_id, "CAPTURE_ERROR", &format!("{error:#}"));
                            return;
                        }
                    };
                    // With no explicit area the recording covers the whole
                    // display.
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
                Backend::Wayland => {
                    #[cfg(feature = "wayland")]
                    {
                        let mut capturer = match crate::capture::wayland_capturer(frame_rate) {
                            Ok(capturer) => capturer,
                            Err(error) => {
                                setup_pending.store(false, Ordering::SeqCst);
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
                                |error| anyhow!("the Wayland frame stream failed: {error}"),
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
                        setup_pending.store(false, Ordering::SeqCst);
                        respond_error(
                            &request_id,
                            "CONFIGURATION_ERROR",
                            "Wayland capture support was not included in this build",
                        );
                        return;
                    }
                }
                Backend::Headless => {
                    setup_pending.store(false, Ordering::SeqCst);
                    respond_error(
                        &request_id,
                        "CONFIGURATION_ERROR",
                        "recording needs an X11 or Wayland graphical session",
                    );
                    return;
                }
            };
            let Some((child, stdin)) = spawn_ffmpeg(
                &ffmpeg_binary,
                grab.width as u16,
                grab.height as u16,
                frame_rate,
                &output_path,
            ) else {
                setup_pending.store(false, Ordering::SeqCst);
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
                InputRecorder::start(grab, keyboard_enabled)
            } else {
                None
            };

            if let Some(input) = input.as_ref() {
                input.sync_origin();
            }

            let pump_stdin = stdin.clone();

            *recording_slot.lock().unwrap() = Some(ActiveRecording {
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
            // The slot now guards double starts; the setup flag is spent.
            setup_pending.store(false, Ordering::SeqCst);

            let completion_request_id = request_id.clone();
            let completion_slot = recording_slot.clone();
            let completion_prefs = track_prefs.clone();
            let completion_output = output_path.clone();
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
                    // No frames ever landed: free the slot for retries and
                    // answer first — the pump keeps winding down on the
                    // setup thread.
                    let Some(mut active) = take_session(&completion_slot, generation) else {
                        return;
                    };
                    completion_prefs.clear();
                    respond_error(
                        &completion_request_id,
                        "START_FAILED",
                        "Recorder worker stopped before the first frame",
                    );
                    active.control.stop.store(true, Ordering::SeqCst);
                    active.control.pace_changed.notify_all();
                    drop(active.stdin.lock().unwrap().take());
                    if let Some(input) = active.input.as_mut() {
                        input.shutdown();
                    }
                    for track in &mut active.tracks {
                        track.kill();
                        let _ = std::fs::remove_file(&track.path);
                    }
                    let _ = active.child.kill();
                    let _ = active.child.wait();
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
        let Some(active) = self.recording.lock().unwrap().take() else {
            return reply_error("INVALID_STATE", "no recording is active");
        };
        // The session is over: the toggles it set must not shape the next one.
        self.track_prefs.clear();
        // Finalizing runs off the router thread: the pipe handoff and the
        // encoder wait can block, and this daemon serves every module from
        // one thread. Windows defers its stop the same way.
        let request_id = request.id.clone();
        std::thread::spawn(move || finalize_stop(active, request_id));
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

/// Takes the session out of the slot only if it is still the one the caller
/// was started for.
fn take_session(slot: &Mutex<Option<ActiveRecording>>, generation: u64) -> Option<ActiveRecording> {
    let mut guard = slot.lock().unwrap();
    if guard
        .as_ref()
        .is_some_and(|active| active.generation == generation)
    {
        guard.take()
    } else {
        None
    }
}

/// Winds a stopped session down and reports the outcome. Runs on its own
/// thread: the pipe handoff and the encoder wait can block, and the router
/// must stay responsive to the rest of the protocol.
fn finalize_stop(mut active: ActiveRecording, request_id: String) {
    active.control.stop.store(true, Ordering::SeqCst);
    active.control.pace_changed.notify_all();
    // Taking the pipe tells FFmpeg to finalize, but the pump may be mid
    // write holding the mutex over a whole frame; kill the encoder first in
    // that case — the failed write frees both the pipe and the pump.
    let pipe = match active.stdin.try_lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => {
            let _ = active.child.kill();
            active.stdin.lock().unwrap().take()
        }
    };
    if let Some(mut pipe) = pipe {
        let _ = pipe.flush();
    }
    // Signalling every track before waiting lets the trailers be written
    // concurrently instead of serially.
    for track in &mut active.tracks {
        track.request_finalize();
    }

    let finalized = ffmpeg::wait_for_exit(&mut active.child, FINALIZE_TIMEOUT);
    // The pipe is closed, so the frame count is final even though the pump
    // may still be winding down. Only a Timeout — a pump stuck inside a hung
    // X11 reply — is a failure: Disconnected covers a pump that exited
    // before its done-send (an early screen failure), and that is not the
    // encoder's fault.
    let pump_finished = !matches!(
        active.pump_done.recv_timeout(PUMP_EXIT_TIMEOUT),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
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
        respond_error(
            &request_id,
            "STOP_FAILED",
            "the recorder did not finalize the recording",
        );
        return;
    }
    let (cursor_path, keys_path) = input_files.unwrap_or((None, None));
    let output_path = active.output_path.to_string_lossy().into_owned();
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
                eprintln!("[recorder] frame grab failed: {error:#}");
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
    width: u16,
    height: u16,
    frame_rate: u32,
    output_path: &std::path::Path,
) -> Option<(Child, Arc<Mutex<Option<ChildStdin>>>)> {
    let mut args = ffmpeg::quiet_args();
    args.extend(ffmpeg::raw_video_input_args(
        "rgba",
        u32::from(width),
        u32::from(height),
        frame_rate,
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
