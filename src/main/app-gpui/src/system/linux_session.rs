use std::ffi::{OsStr, OsString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub use poratake_daemon_common::platform::LinuxBackend as LinuxSession;

static SESSION: OnceLock<LinuxSession> = OnceLock::new();

/// `xdg-desktop-portal` may still be starting when we launch (autostart at
/// login) and FFmpeg may only appear later, so failed lookups back off
/// instead of becoming permanent verdicts.
const PROBE_COOLDOWN: Duration = Duration::from_secs(5);

/// A boolean capability that costs a subprocess or D-Bus round-trip to
/// determine: probed in the background, cached, and re-tried on a cooldown so
/// a late-starting backend is picked up without an app restart.
struct LazyProbe {
    value: AtomicBool,
    in_flight: AtomicBool,
    announced: AtomicBool,
    last: Mutex<Option<Instant>>,
    label: &'static str,
    probe: fn() -> bool,
}

/// Clears the in-flight marker on every exit path so a panicking probe thread
/// cannot wedge future retries.
struct ProbeGuard(&'static LazyProbe);

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        self.0.in_flight.store(false, Ordering::Release);
    }
}

impl LazyProbe {
    /// The latest known verdict; a `false` answer kicks off a fresh probe
    /// when the cooldown allows one.
    fn get(&'static self) -> bool {
        let value = self.value.load(Ordering::Relaxed);
        if !value && self.probe_allowed() {
            self.spawn();
        }
        value
    }

    /// Seeds a probe without waiting for a consumer of the verdict.
    fn seed(&'static self) {
        if !self.value.load(Ordering::Relaxed) {
            self.spawn();
        }
    }

    fn probe_allowed(&self) -> bool {
        let last = self.last.lock().unwrap();
        !matches!(*last, Some(started) if started.elapsed() < PROBE_COOLDOWN)
    }

    fn spawn(&'static self) {
        if self.in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        // Stamp only once a probe is actually claimed, or a no-op spawn would
        // consume the cooldown without producing a verdict.
        *self.last.lock().unwrap() = Some(Instant::now());
        let spawned = std::thread::Builder::new()
            .name(format!("{}-probe", self.label))
            .spawn(move || {
                let _guard = ProbeGuard(self);
                let available = (self.probe)();
                self.value.store(available, Ordering::Relaxed);
                if !available && !self.announced.swap(true, Ordering::Relaxed) {
                    eprintln!("[display] {} is unavailable", self.label);
                }
            });
        if let Err(error) = spawned {
            self.in_flight.store(false, Ordering::Release);
            eprintln!("[display] {} probe failed to spawn: {error}", self.label);
        }
    }
}

static SCREEN_CAST: LazyProbe = LazyProbe {
    value: AtomicBool::new(false),
    in_flight: AtomicBool::new(false),
    announced: AtomicBool::new(false),
    last: Mutex::new(None),
    label: "Wayland ScreenCast portal",
    probe: screen_cast_portal_available,
};

static FFMPEG_ENCODER: LazyProbe = LazyProbe {
    value: AtomicBool::new(false),
    in_flight: AtomicBool::new(false),
    announced: AtomicBool::new(false),
    last: Mutex::new(None),
    label: "FFmpeg libx264 encoder",
    probe: poratake_daemon_common::ffmpeg::h264_available,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinuxCapabilities {
    pub screen_cast: bool,
    pub ffmpeg_encoder: bool,
}

pub struct DisplayEnvironment {
    wayland_display: Option<OsString>,
    changed: bool,
}

impl DisplayEnvironment {
    pub fn restore(self) {
        if !self.changed {
            return;
        }
        if let Some(display) = self.wayland_display {
            unsafe { std::env::set_var("WAYLAND_DISPLAY", display) };
            return;
        }
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
    }
}

pub fn configure() -> DisplayEnvironment {
    let wsl =
        std::env::var_os("WSL_INTEROP").is_some() || std::env::var_os("WSL_DISTRO_NAME").is_some();
    let wayland = std::env::var_os("WAYLAND_DISPLAY");
    let x11 = std::env::var_os("DISPLAY");
    let (session, wslg_fallback) = resolve(wsl, wayland.as_deref(), x11.as_deref());
    if wslg_fallback {
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
        eprintln!(
            "[display] using WSLg X11 compatibility because its native Wayland shell is unsupported"
        );
    }
    debug_assert!(
        SESSION.set(session).is_ok(),
        "Linux session configured twice"
    );
    match session {
        LinuxSession::Wayland => SCREEN_CAST.seed(),
        // Recording is an X11 feature on Linux, so the encoder only matters
        // there; probing it on Wayland would log a warning nothing consumes.
        LinuxSession::X11 => FFMPEG_ENCODER.seed(),
        LinuxSession::Headless => {}
    }
    DisplayEnvironment {
        wayland_display: wayland,
        changed: wslg_fallback,
    }
}

/// The ScreenCast and encoder verdicts are probed in the background so the
/// lookups never block startup or a CLI hand-off; callers read the latest
/// known answers and late-starting backends are picked up on later calls.
pub fn capabilities() -> LinuxCapabilities {
    let session = SESSION.get();
    LinuxCapabilities {
        screen_cast: if session == Some(&LinuxSession::Wayland) {
            SCREEN_CAST.get()
        } else {
            false
        },
        ffmpeg_encoder: if session == Some(&LinuxSession::X11) {
            FFMPEG_ENCODER.get()
        } else {
            false
        },
    }
}

fn screen_cast_portal_available() -> bool {
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties as _;
    use dbus::blocking::Connection;

    let Ok(connection) = Connection::new_session() else {
        return false;
    };
    let proxy = connection.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_secs(2),
    );
    let source_types: Result<u32, _> =
        proxy.get("org.freedesktop.portal.ScreenCast", "AvailableSourceTypes");
    source_types.is_ok_and(|source_types| source_types & 1 == 1)
}

pub fn current() -> LinuxSession {
    #[cfg(test)]
    if SESSION.get().is_none() {
        return LinuxSession::X11;
    }
    *SESSION
        .get()
        .expect("Linux session must be configured before GPUI starts")
}

fn resolve(wsl: bool, wayland: Option<&OsStr>, x11: Option<&OsStr>) -> (LinuxSession, bool) {
    let has_wayland = wayland.is_some_and(|display| !display.is_empty());
    let has_x11 = x11.is_some_and(|display| !display.is_empty());
    let wslg_fallback = wsl && wayland == Some(OsStr::new("wayland-0")) && has_x11;
    if wslg_fallback || (!has_wayland && has_x11) {
        return (LinuxSession::X11, wslg_fallback);
    }
    if has_wayland {
        return (LinuxSession::Wayland, false);
    }
    (LinuxSession::Headless, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_wslg_falls_back_to_x11() {
        assert_eq!(
            resolve(true, Some(OsStr::new("wayland-0")), Some(OsStr::new(":0"))),
            (LinuxSession::X11, true)
        );
    }

    #[test]
    fn nested_wayland_remains_wayland() {
        assert_eq!(
            resolve(
                true,
                Some(OsStr::new("wayland-poratake")),
                Some(OsStr::new(":0"))
            ),
            (LinuxSession::Wayland, false)
        );
    }

    #[test]
    fn missing_graphical_environment_is_headless() {
        assert_eq!(resolve(false, None, None), (LinuxSession::Headless, false));
    }

    #[test]
    fn capabilities_stay_false_outside_graphic_sessions() {
        // In tests the session is unconfigured, so both probes must stay
        // untouched — no subprocesses, no threads.
        let capabilities = capabilities();
        assert!(!capabilities.screen_cast);
        assert!(!capabilities.ffmpeg_encoder);
    }
}
