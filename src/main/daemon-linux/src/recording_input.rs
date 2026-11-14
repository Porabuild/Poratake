//! Input-event recording for X11 screen recording, mirroring the Windows
//! daemon's `recording_input` module: a passive XInput2 listener captures
//! global key presses, clicks, wheel scrolls, and pointer motion, and stop
//! writes `cursor.json` and — when keyboard recording is enabled —
//! `keys.json` beside the video in the same shape the Windows recorder and
//! the editor's `keyboard-data.ts` read.
//!
//! Events are delivered by the X server (raw key/button events on the root
//! window plus regular root motion), so the listener thread blocks in
//! `wait_for_event` and never polls. A throwaway window is destroyed on stop
//! to unblock the loop even on a machine where no input ever arrives.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use poratake_daemon_common::geometry::fit_rect_pixels;
use serde::Serialize;
use xcb::Connection;
use xcb::x;
use xcb::xinput;

use poratake_daemon_common::geometry::CaptureRect;

const KEYS_FILE: &str = "keys.json";
const CURSOR_FILE: &str = "cursor.json";
const WAKE_WINDOW_SIZE: u16 = 1;

/// One raw input observation, wall-clock stamped; timestamps become
/// recording-relative only at write time.
struct RawKeyEvent {
    wall_time: Instant,
    key: String,
    key_code: u32,
    modifiers: Vec<&'static str>,
    event_type: &'static str,
}

struct RawCursorEvent {
    wall_time: Instant,
    x: f64,
    y: f64,
    event_type: &'static str,
    button: Option<&'static str>,
    scroll_delta: Option<(f64, f64)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorFile {
    recording_area: RecordingArea,
    events: Vec<CursorEvent>,
    meta: EventMeta,
}

#[derive(Serialize)]
struct RecordingArea {
    width: i32,
    height: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorEvent {
    timestamp: f64,
    x: f64,
    y: f64,
    #[serde(rename = "type")]
    event_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    button: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scroll_delta: Option<ScrollDelta>,
}

#[derive(Serialize)]
struct ScrollDelta {
    x: f64,
    y: f64,
}

#[derive(Serialize)]
struct KeyboardFile {
    events: Vec<KeyboardEvent>,
    meta: EventMeta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyboardEvent {
    timestamp: f64,
    key: String,
    key_code: u32,
    modifiers: Vec<&'static str>,
    #[serde(rename = "type")]
    event_type: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EventMeta {
    start_time: String,
    duration: f64,
    sample_rate: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<&'static str>,
}

/// Shared between the module methods (router thread) and the listener thread.
struct Shared {
    keyboard_enabled: bool,
    stop: AtomicBool,
    origin: Mutex<Option<Instant>>,
    pauses: Mutex<Vec<(Instant, Instant)>>,
    pause_start: Mutex<Option<Instant>>,
    modifiers: Mutex<Vec<keymap::ModifierKind>>,
    key_events: Mutex<Vec<RawKeyEvent>>,
    cursor_events: Mutex<Vec<RawCursorEvent>>,
}

impl Shared {
    /// Folds a finished pause into the pause list so timestamps after it are
    /// shifted the same way the frame-indexed video shifts.
    fn close_pause_span(&self) {
        let mut pause_start = self.pause_start.lock().unwrap();
        if let Some(start) = pause_start.take() {
            self.pauses.lock().unwrap().push((start, Instant::now()));
        }
    }

    fn serialized_modifiers(&self) -> Vec<&'static str> {
        let pressed = self.modifiers.lock().unwrap();
        let mut modifiers = Vec::new();
        if pressed.contains(&keymap::ModifierKind::Meta) {
            modifiers.push("meta");
        }
        if pressed.contains(&keymap::ModifierKind::Control) {
            modifiers.push("control");
        }
        if pressed.contains(&keymap::ModifierKind::Alt) {
            modifiers.push("alt");
        }
        if pressed.contains(&keymap::ModifierKind::Shift) {
            modifiers.push("shift");
        }
        modifiers
    }

    fn set_modifier(&self, kind: keymap::ModifierKind, down: bool) {
        let mut pressed = self.modifiers.lock().unwrap();
        if down {
            if !pressed.contains(&kind) {
                pressed.push(kind);
            }
        } else {
            pressed.retain(|held| *held != kind);
        }
    }

    fn push_key(&self, event: RawKeyEvent) {
        self.key_events.lock().unwrap().push(event);
    }

    fn push_cursor(&self, event: RawCursorEvent) {
        let mut events = self.cursor_events.lock().unwrap();
        // Consecutive moves collapse to the latest position, matching the
        // Windows tracker, so fast motion does not flood the file.
        if event.event_type == "move"
            && let Some(last) = events.last_mut()
            && last.event_type == "move"
        {
            last.x = event.x;
            last.y = event.y;
            last.wall_time = event.wall_time;
            return;
        }
        events.push(event);
    }
}

/// A live passive listener. Call [`InputRecorder::stop`] to end the thread;
/// dropping without it leaves the listener blocked on the X connection.
pub struct InputRecorder {
    shared: Arc<Shared>,
    control: Connection,
    wake_window: x::Window,
    listener: Option<std::thread::JoinHandle<()>>,
    grab: CaptureRect,
    stopped: bool,
}

impl InputRecorder {
    /// Starts the passive listener, or returns `None` when the display has
    /// no XInput2 or keymap — a session where input recording genuinely
    /// cannot work.
    pub fn start(
        source_area: CaptureRect,
        frame: CaptureRect,
        keyboard_enabled: bool,
        window_bounds: Option<Arc<Mutex<CaptureRect>>>,
    ) -> Option<Self> {
        let Ok((control, screen_number)) =
            Connection::connect_with_extensions(None, &[xcb::Extension::Input], &[])
        else {
            return None;
        };
        let setup = control.get_setup();
        let screen = setup.roots().nth(screen_number as usize)?;
        let root = screen.root();
        let keymap = Arc::new(keymap::load(&control)?);

        let wake_window = wake_window(&control, root)?;
        let shared = Arc::new(Shared {
            keyboard_enabled,
            stop: AtomicBool::new(false),
            origin: Mutex::new(None),
            pauses: Mutex::new(Vec::new()),
            pause_start: Mutex::new(None),
            modifiers: Mutex::new(Vec::new()),
            key_events: Mutex::new(Vec::new()),
            cursor_events: Mutex::new(Vec::new()),
        });
        let listener = std::thread::Builder::new()
            .name("linux-input-listener".into())
            .spawn({
                let shared = shared.clone();
                // XInput2 events reach every connection that selected them,
                // so the listener gets its own connection instead of a clone.
                let Ok((connection, listener_screen)) =
                    Connection::connect_with_extensions(None, &[xcb::Extension::Input], &[])
                else {
                    return None;
                };
                let listener_root = connection
                    .get_setup()
                    .roots()
                    .nth(listener_screen as usize)?
                    .root();
                let xi_version = connection
                    .wait_for_reply(connection.send_request(&xinput::XiQueryVersion {
                        major_version: 2,
                        minor_version: 1,
                    }))
                    .ok()?;
                if xi_version.major_version() < 2 {
                    return None;
                }
                let masks = [xinput::EventMaskBuf::new(
                    xinput::Device::All,
                    &[xinput::XiEventMask::RAW_KEY_PRESS
                        | xinput::XiEventMask::RAW_KEY_RELEASE
                        | xinput::XiEventMask::RAW_BUTTON_PRESS
                        | xinput::XiEventMask::RAW_BUTTON_RELEASE
                        | xinput::XiEventMask::RAW_MOTION],
                )];
                if connection
                    .send_and_check_request(&xinput::XiSelectEvents {
                        window: listener_root,
                        masks: &masks,
                    })
                    .is_err()
                    || connection
                        .send_and_check_request(&x::ChangeWindowAttributes {
                            window: wake_window,
                            value_list: &[x::Cw::EventMask(x::EventMask::STRUCTURE_NOTIFY)],
                        })
                        .is_err()
                {
                    return None;
                }
                move || {
                    listen(
                        connection,
                        listener_root,
                        wake_window,
                        source_area,
                        frame,
                        window_bounds,
                        keymap,
                        shared,
                    )
                }
            })
            .ok()?;

        Some(Self {
            shared,
            control,
            wake_window,
            listener: Some(listener),
            grab: frame,
            stopped: false,
        })
    }

    /// Aligns event timestamps with the first captured video frame.
    pub fn sync_origin(&self) {
        let mut origin = self.shared.origin.lock().unwrap();
        if origin.is_none() {
            *origin = Some(Instant::now());
        }
    }

    pub fn pause(&self) {
        let mut pause_start = self.shared.pause_start.lock().unwrap();
        if pause_start.is_none() {
            *pause_start = Some(Instant::now());
        }
    }

    pub fn resume(&self) {
        self.shared.close_pause_span();
    }

    /// Tears the listener down without writing files, for sessions that
    /// never produced video.
    pub fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = self.control.send_request(&x::DestroyWindow {
            window: self.wake_window,
        });
        let _ = self.control.flush();
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }

    /// Stops the listener and writes `cursor.json` (and `keys.json` when
    /// keyboard recording was enabled) beside the video, staged under a dot
    /// name and renamed so readers never see a half-written file.
    pub fn finish(
        &mut self,
        duration: f64,
        output_directory: &Path,
    ) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
        self.shutdown();

        let Some(origin) = *self.shared.origin.lock().unwrap() else {
            return Ok((None, None));
        };
        let pauses = self.shared.pauses.lock().unwrap().clone();
        let start_time = format_system_time(SystemTime::now());

        let cursor_events: Vec<CursorEvent> = self
            .shared
            .cursor_events
            .lock()
            .unwrap()
            .iter()
            .map(|event| CursorEvent {
                timestamp: event_timestamp(event.wall_time, origin, &pauses),
                x: event.x,
                y: event.y,
                event_type: event.event_type,
                button: event.button,
                scroll_delta: event.scroll_delta.map(|(x, y)| ScrollDelta { x, y }),
            })
            .collect();
        let cursor_count = cursor_events.len();
        let cursor_file = CursorFile {
            recording_area: RecordingArea {
                width: self.grab.width,
                height: self.grab.height,
            },
            events: cursor_events,
            meta: EventMeta {
                start_time: start_time.clone(),
                duration,
                sample_rate: sample_rate(cursor_count, duration),
                platform: Some("linux"),
            },
        };
        let cursor_path = output_directory.join(CURSOR_FILE);
        write_json_atomically(&cursor_path, &cursor_file)?;

        let keys_path = if self.shared.keyboard_enabled {
            let events: Vec<KeyboardEvent> = self
                .shared
                .key_events
                .lock()
                .unwrap()
                .iter()
                .map(|event| KeyboardEvent {
                    timestamp: event_timestamp(event.wall_time, origin, &pauses),
                    key: event.key.clone(),
                    key_code: event.key_code,
                    modifiers: event.modifiers.clone(),
                    event_type: event.event_type,
                })
                .collect();
            let count = events.len();
            let keyboard_file = KeyboardFile {
                events,
                meta: EventMeta {
                    start_time,
                    duration,
                    sample_rate: sample_rate(count, duration),
                    platform: Some("linux"),
                },
            };
            let path = output_directory.join(KEYS_FILE);
            write_json_atomically(&path, &keyboard_file)?;
            Some(path)
        } else {
            None
        };

        Ok((Some(cursor_path), keys_path))
    }
}

/// Blocks on the X connection until an input event or the wake-up window's
/// destruction; there is no polling anywhere on this path.
fn listen(
    connection: Connection,
    root: x::Window,
    wake_window: x::Window,
    source_area: CaptureRect,
    frame: CaptureRect,
    window_bounds: Option<Arc<Mutex<CaptureRect>>>,
    keymap: Arc<keymap::Keymap>,
    shared: Arc<Shared>,
) {
    loop {
        let Ok(event) = connection.wait_for_event() else {
            return;
        };
        match event {
            xcb::Event::Input(xinput::Event::RawKeyPress(event)) => {
                let key_code = event.detail();
                let modifiers = shared.serialized_modifiers();
                shared.push_key(RawKeyEvent {
                    wall_time: Instant::now(),
                    key: keymap.key_name(key_code),
                    key_code,
                    modifiers,
                    event_type: "down",
                });
                if let Some(kind) = keymap.modifier_kind(key_code) {
                    shared.set_modifier(kind, true);
                }
            }
            xcb::Event::Input(xinput::Event::RawKeyRelease(event)) => {
                let key_code = event.detail();
                if let Some(kind) = keymap.modifier_kind(key_code) {
                    shared.set_modifier(kind, false);
                }
                let modifiers = shared.serialized_modifiers();
                shared.push_key(RawKeyEvent {
                    wall_time: Instant::now(),
                    key: keymap.key_name(key_code),
                    key_code,
                    modifiers,
                    event_type: "up",
                });
            }
            xcb::Event::Input(xinput::Event::RawButtonPress(event)) => {
                let detail = event.detail();
                let (x, y) = pointer_position(&connection, root);
                let point = (f64::from(x), f64::from(y));
                let Some((x, y)) = (match &window_bounds {
                    Some(bounds) => {
                        window_cursor_position_in_bounds(point, *bounds.lock().unwrap(), frame)
                    }
                    None => area_cursor_position(point, source_area, frame),
                }) else {
                    continue;
                };
                shared.push_cursor(RawCursorEvent {
                    wall_time: Instant::now(),
                    x,
                    y,
                    event_type: "down",
                    button: button_name(detail),
                    scroll_delta: wheel_delta(detail),
                });
            }
            xcb::Event::Input(xinput::Event::RawButtonRelease(event)) => {
                let detail = event.detail();
                let (x, y) = pointer_position(&connection, root);
                let point = (f64::from(x), f64::from(y));
                let Some((x, y)) = (match &window_bounds {
                    Some(bounds) => {
                        window_cursor_position_in_bounds(point, *bounds.lock().unwrap(), frame)
                    }
                    None => area_cursor_position(point, source_area, frame),
                }) else {
                    continue;
                };
                shared.push_cursor(RawCursorEvent {
                    wall_time: Instant::now(),
                    x,
                    y,
                    event_type: "up",
                    button: button_name(detail),
                    scroll_delta: None,
                });
            }
            xcb::Event::Input(xinput::Event::RawMotion(_)) => {
                let (x, y) = pointer_position(&connection, root);
                let point = (f64::from(x), f64::from(y));
                let Some((x, y)) = (match &window_bounds {
                    Some(bounds) => {
                        window_cursor_position_in_bounds(point, *bounds.lock().unwrap(), frame)
                    }
                    None => area_cursor_position(point, source_area, frame),
                }) else {
                    continue;
                };
                shared.push_cursor(RawCursorEvent {
                    wall_time: Instant::now(),
                    x,
                    y,
                    event_type: "move",
                    button: None,
                    scroll_delta: None,
                });
            }
            xcb::Event::X(x::Event::DestroyNotify(notify)) if notify.window() == wake_window => {
                return;
            }
            _ => {}
        }
    }
}

fn pointer_position(connection: &Connection, root: x::Window) -> (i32, i32) {
    let Ok(reply) =
        connection.wait_for_reply(connection.send_request(&x::QueryPointer { window: root }))
    else {
        return (0, 0);
    };
    (i32::from(reply.root_x()), i32::from(reply.root_y()))
}

fn area_cursor_position(
    point: (f64, f64),
    source: CaptureRect,
    frame: CaptureRect,
) -> Option<(f64, f64)> {
    if source.width <= 0 || source.height <= 0 || frame.width <= 0 || frame.height <= 0 {
        return None;
    }
    let x = point.0 - f64::from(source.x);
    let y = point.1 - f64::from(source.y);
    let coordinate = |value: f64, source_size: i32, frame_size: i32| {
        if value < 0.0 {
            0.0
        } else if value >= f64::from(source_size) {
            1.0
        } else {
            value / f64::from(frame_size)
        }
    };
    Some((
        coordinate(x, source.width, frame.width),
        coordinate(y, source.height, frame.height),
    ))
}

fn window_cursor_position_in_bounds(
    point: (f64, f64),
    bounds: CaptureRect,
    frame: CaptureRect,
) -> Option<(f64, f64)> {
    if bounds.width <= 0 || bounds.height <= 0 || frame.width <= 0 || frame.height <= 0 {
        return None;
    }
    let relative = (
        ((point.0 - f64::from(bounds.x)) / f64::from(bounds.width)).clamp(0.0, 1.0),
        ((point.1 - f64::from(bounds.y)) / f64::from(bounds.height)).clamp(0.0, 1.0),
    );
    let fit = fit_rect_pixels(
        (bounds.width as u32, bounds.height as u32),
        (frame.width as u32, frame.height as u32),
    );
    if fit.width == 0 || fit.height == 0 {
        return None;
    }
    Some((
        (f64::from(fit.x) + relative.0 * f64::from(fit.width)) / f64::from(frame.width),
        (f64::from(fit.y) + relative.1 * f64::from(fit.height)) / f64::from(frame.height),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcb::Xid;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> CaptureRect {
        CaptureRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn odd_area_cursor_keeps_the_last_real_pixel_and_places_outside_at_the_video_edge() {
        let source = rect(100, 50, 101, 99);
        let frame = rect(100, 50, 102, 100);

        let last_real_pixel = area_cursor_position((200.0, 148.0), source, frame).expect("inside");
        let outside = area_cursor_position((201.0, 149.0), source, frame).expect("edge");

        assert_eq!(last_real_pixel.0 * f64::from(frame.width), 100.0);
        assert_eq!(last_real_pixel.1 * f64::from(frame.height), 98.0);
        assert_eq!(outside, (1.0, 1.0));
    }

    #[test]
    fn moved_window_keeps_the_cursor_at_the_same_position_in_the_letterboxed_frame() {
        let frame = rect(0, 0, 800, 600);
        let before_bounds = rect(100, 100, 400, 600);
        let after_bounds = rect(500, 300, 400, 600);

        let before = window_cursor_position_in_bounds((300.0, 400.0), before_bounds, frame)
            .expect("before move");
        let after = window_cursor_position_in_bounds((700.0, 600.0), after_bounds, frame)
            .expect("after move");

        assert_eq!(before, after);
        assert_eq!(before, (0.5, 0.5));
    }

    #[test]
    fn resized_window_cursor_uses_the_effective_pixel_fit_rectangle() {
        let bounds = rect(0, 0, 400, 600);
        let frame = rect(0, 0, 800, 600);

        let center =
            window_cursor_position_in_bounds((200.0, 300.0), bounds, frame).expect("window center");
        let left_edge =
            window_cursor_position_in_bounds((0.0, 0.0), bounds, frame).expect("window edge");

        assert_eq!(center, (0.5, 0.5));
        assert_eq!(left_edge, (0.25, 0.0));
    }

    #[test]
    #[ignore = "requires an X11 display"]
    fn records_global_raw_input_when_a_child_selects_xi2_events() {
        let (connection, screen_number) = Connection::connect_with_extensions(
            None,
            &[xcb::Extension::Input, xcb::Extension::Test],
            &[],
        )
        .expect("connect to X11");
        let root = connection
            .get_setup()
            .roots()
            .nth(screen_number as usize)
            .expect("X11 screen")
            .root();
        let child: x::Window = connection.generate_id();
        connection
            .check_request(connection.send_request_checked(&x::CreateWindow {
                depth: x::COPY_FROM_PARENT as u8,
                wid: child,
                parent: root,
                x: 40,
                y: 50,
                width: 101,
                height: 99,
                border_width: 0,
                class: x::WindowClass::InputOutput,
                visual: x::COPY_FROM_PARENT,
                value_list: &[x::Cw::BackPixel(0x00ff_0000)],
            }))
            .expect("create child window");
        connection.send_request(&x::MapWindow { window: child });
        connection
            .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
            .expect("synchronize child window");
        let (consumer, _) =
            Connection::connect_with_extensions(None, &[xcb::Extension::Input], &[])
                .expect("connect XI2 child client");
        let xi_version = consumer
            .wait_for_reply(consumer.send_request(&xcb::xinput::XiQueryVersion {
                major_version: 2,
                minor_version: 1,
            }))
            .expect("query XI2 version");
        let supports_grabbed_raw_input = xi_version.major_version() > 2
            || xi_version.major_version() == 2 && xi_version.minor_version() >= 1;
        let child_masks = [xcb::xinput::EventMaskBuf::new(
            xcb::xinput::Device::All,
            &[xcb::xinput::XiEventMask::BUTTON_PRESS
                | xcb::xinput::XiEventMask::BUTTON_RELEASE
                | xcb::xinput::XiEventMask::MOTION],
        )];
        consumer
            .send_and_check_request(&xcb::xinput::XiSelectEvents {
                window: child,
                masks: &child_masks,
            })
            .expect("child client selects XI2 events");
        let source = rect(40, 50, 101, 99);
        let frame = rect(40, 50, 102, 100);
        let mut input = InputRecorder::start(source, frame, false, None).expect("start input");
        input.sync_origin();
        let fake_click = || {
            for (event_type, detail) in [(6, 0), (4, 1), (5, 1)] {
                connection.send_request(&xcb::xtest::FakeInput {
                    r#type: event_type,
                    detail,
                    time: 0,
                    root,
                    root_x: 140,
                    root_y: 148,
                    deviceid: 0,
                });
            }
            connection.flush().expect("flush fake cursor events");
            connection
                .wait_for_reply(connection.send_request(&x::GetInputFocus {}))
                .expect("synchronize fake cursor events");
            consumer
                .wait_for_reply(consumer.send_request(&x::GetInputFocus {}))
                .expect("synchronize child input events");
        };
        let child_events = || {
            let mut child_saw_press = false;
            let mut child_saw_release = false;
            let mut child_saw_motion = false;
            while let Some(event) = consumer.poll_for_event().expect("poll child XI2 events") {
                match event {
                    xcb::Event::Input(xcb::xinput::Event::ButtonPress(_)) => {
                        child_saw_press = true;
                    }
                    xcb::Event::Input(xcb::xinput::Event::ButtonRelease(_)) => {
                        child_saw_release = true;
                    }
                    xcb::Event::Input(xcb::xinput::Event::Motion(_)) => {
                        child_saw_motion = true;
                    }
                    _ => {}
                }
            }
            (child_saw_press, child_saw_release, child_saw_motion)
        };
        fake_click();
        let (child_saw_press, child_saw_release, child_saw_motion) = child_events();
        assert!(child_saw_press);
        assert!(child_saw_release);
        assert!(child_saw_motion);
        if supports_grabbed_raw_input {
            let grab_mask = [(xcb::xinput::XiEventMask::BUTTON_PRESS
                | xcb::xinput::XiEventMask::BUTTON_RELEASE
                | xcb::xinput::XiEventMask::MOTION)
                .bits()];
            let grab = consumer
                .wait_for_reply(consumer.send_request(&xcb::xinput::XiGrabDevice {
                    window: child,
                    time: x::CURRENT_TIME,
                    cursor: x::Cursor::none(),
                    device: xcb::xinput::Device::Id(2),
                    mode: x::GrabMode::Async,
                    paired_device_mode: x::GrabMode::Async,
                    owner_events: xcb::xinput::GrabOwner::NoOwner,
                    mask: &grab_mask,
                }))
                .expect("grab child pointer");
            assert_eq!(grab.status(), x::GrabStatus::Success);
            fake_click();
            let grabbed_events = child_events();
            assert!(grabbed_events.0);
            assert!(grabbed_events.1);
            assert!(grabbed_events.2);
            consumer.send_request(&xcb::xinput::XiUngrabDevice {
                time: x::CURRENT_TIME,
                device: xcb::xinput::Device::Id(2),
            });
        }

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let output_directory = std::env::temp_dir().join(format!(
            "poratake-linux-input-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&output_directory).expect("create sidecar directory");
        let (cursor_path, keys_path) = input
            .finish(1.0, &output_directory)
            .expect("finish input sidecar");
        assert!(keys_path.is_none());
        let cursor_path = cursor_path.expect("cursor sidecar");
        let cursor: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&cursor_path).expect("read cursor sidecar"))
                .expect("parse cursor sidecar");
        assert_eq!(cursor["recordingArea"]["width"], 102);
        assert_eq!(cursor["recordingArea"]["height"], 100);
        let events = cursor["events"].as_array().expect("cursor events");
        assert!(events.len() >= 3);
        assert_eq!(events.first().expect("first event")["type"], "move");
        assert!(events.iter().any(|event| event["type"] == "down"));
        assert!(events.iter().any(|event| event["type"] == "up"));
        for event in events {
            assert!((event["x"].as_f64().expect("cursor x") - 100.0 / 102.0).abs() < 1e-12);
            assert!((event["y"].as_f64().expect("cursor y") - 98.0 / 100.0).abs() < 1e-12);
        }
        connection.send_request(&x::DestroyWindow { window: child });
        std::fs::remove_dir_all(output_directory).expect("remove sidecar directory");
    }
}

fn button_name(detail: u32) -> Option<&'static str> {
    match detail {
        1 => Some("left"),
        2 => Some("middle"),
        3 => Some("right"),
        _ => None,
    }
}

/// X11 wheel buttons: 4/5 are vertical up/down, 6/7 horizontal left/right.
fn wheel_delta(detail: u32) -> Option<(f64, f64)> {
    match detail {
        4 => Some((0.0, 1.0)),
        5 => Some((0.0, -1.0)),
        6 => Some((1.0, 0.0)),
        7 => Some((-1.0, 0.0)),
        _ => None,
    }
}

fn wake_window(connection: &Connection, root: x::Window) -> Option<x::Window> {
    let window = connection.generate_id();
    connection
        .send_and_check_request(&x::CreateWindow {
            depth: x::COPY_FROM_PARENT as u8,
            wid: window,
            parent: root,
            x: 0,
            y: 0,
            width: WAKE_WINDOW_SIZE,
            height: WAKE_WINDOW_SIZE,
            border_width: 0,
            class: x::WindowClass::InputOnly,
            visual: x::COPY_FROM_PARENT,
            value_list: &[x::Cw::EventMask(x::EventMask::STRUCTURE_NOTIFY)],
        })
        .ok()?;
    Some(window)
}

fn event_timestamp(wall_time: Instant, origin: Instant, pauses: &[(Instant, Instant)]) -> f64 {
    if wall_time <= origin {
        return 0.0;
    }
    let mut elapsed = wall_time.duration_since(origin);
    for (start, end) in pauses {
        if *start >= wall_time || *end <= origin {
            continue;
        }
        let overlap_start = (*start).max(origin);
        let overlap_end = (*end).min(wall_time);
        elapsed = elapsed.saturating_sub(overlap_end.duration_since(overlap_start));
    }
    elapsed.as_secs_f64()
}

fn sample_rate(count: usize, duration: f64) -> u64 {
    if count == 0 || duration <= 0.0 {
        return 0;
    }
    (count as f64 / duration).floor() as u64
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> Result<()> {
    let staged = path.with_file_name(format!(
        ".{}.poratake-staged",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("invalid event file name"))?
    ));
    std::fs::write(&staged, serde_json::to_vec(value)?)?;
    std::fs::rename(&staged, path)?;
    Ok(())
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ`, matching the Windows recorder's metadata.
fn format_system_time(time: SystemTime) -> String {
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_seconds = elapsed.as_secs() as i64;
    let millis = elapsed.subsec_millis();
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Days since the Unix epoch to a civil (Gregorian) date.
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

/// Keycode to keysym/name mapping loaded once from the server keymap.
pub(super) mod keymap {
    use std::collections::HashMap;

    use xcb::{Connection, x};

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum ModifierKind {
        Meta,
        Control,
        Alt,
        Shift,
    }

    pub(super) struct Keymap {
        names: HashMap<u32, String>,
        modifiers: HashMap<u32, ModifierKind>,
    }

    impl Keymap {
        pub(super) fn key_name(&self, key_code: u32) -> String {
            self.names.get(&key_code).cloned().unwrap_or_default()
        }

        pub(super) fn modifier_kind(&self, key_code: u32) -> Option<ModifierKind> {
            self.modifiers.get(&key_code).copied()
        }
    }

    pub(super) fn load(connection: &Connection) -> Option<Keymap> {
        let setup = connection.get_setup();
        let min = setup.min_keycode();
        let count = u32::from(setup.max_keycode() - min + 1);
        let reply = connection
            .wait_for_reply(connection.send_request(&x::GetKeyboardMapping {
                first_keycode: min,
                count: count as u8,
            }))
            .ok()?;
        let per_keycode = usize::from(reply.keysyms_per_keycode().max(1));
        let mut names = HashMap::new();
        let mut modifiers = HashMap::new();
        for (index, keysyms) in reply.keysyms().chunks(per_keycode).enumerate() {
            let key_code = u32::from(min + index as u8);
            // Column 0 is the unshifted keysym, matching Windows' virtual-key
            // behaviour where Shift changes the event, not the recorded name.
            let Some(keysym) = keysyms.first().copied().filter(|sym| *sym != 0) else {
                continue;
            };
            names.insert(key_code, keysym_name(keysym));
            if let Some(kind) = modifier_kind(keysym) {
                modifiers.insert(key_code, kind);
            }
        }
        Some(Keymap { names, modifiers })
    }

    fn modifier_kind(keysym: u32) -> Option<ModifierKind> {
        match keysym {
            0xFFE1 | 0xFFE2 => Some(ModifierKind::Shift),
            0xFFE3 | 0xFFE4 => Some(ModifierKind::Control),
            0xFFE9 | 0xFFEA => Some(ModifierKind::Alt),
            0xFFE7 | 0xFFE8 | 0xFFEB | 0xFFEC => Some(ModifierKind::Meta),
            _ => None,
        }
    }

    /// Names a keysym the way the Windows recorder names virtual keys:
    /// letters and digits as single characters, function and keypad keys
    /// prefixed, everything else by its common name.
    fn keysym_name(keysym: u32) -> String {
        if (0x020..=0x07E).contains(&keysym)
            && let Some(character) = char::from_u32(keysym)
        {
            return match keysym {
                0x020 => "Space".into(),
                _ => character.to_ascii_lowercase().to_string(),
            };
        }
        match keysym {
            0xFF08 => "Delete".into(),
            0xFF09 => "Tab".into(),
            0xFF0D => "Return".into(),
            0xFF1B => "Escape".into(),
            0xFF50 => "Home".into(),
            0xFF51 => "Left".into(),
            0xFF52 => "Up".into(),
            0xFF53 => "Right".into(),
            0xFF54 => "Down".into(),
            0xFF55 => "PageUp".into(),
            0xFF56 => "PageDown".into(),
            0xFF57 => "End".into(),
            0xFF63 => "Insert".into(),
            0xFF9F => "KeypadDelete".into(),
            0xFFBE..=0xFFD1 => format!("F{}", keysym - 0xFFBD),
            0xFFB0..=0xFFB9 => format!("Keypad{}", keysym - 0xFFB0),
            0xFFAC => "KeypadSeparator".into(),
            0xFFAE => "KeypadPeriod".into(),
            0xFFAA => "KeypadMultiply".into(),
            0xFFAB => "KeypadAdd".into(),
            0xFFAD => "KeypadSubtract".into(),
            0xFFAF => "KeypadDivide".into(),
            0xFFFF => "Delete".into(),
            _ => String::new(),
        }
    }
}
