use crate::desktop_frame::{LinearFrame, capture_window_linear};
use crate::display_color::{DisplayTransfer, TRANSFER_SIZE, hdr_white_scale};
use crate::overlay::{
    bitmap_info, create_popup_window, default_wndproc, ensure_window_class, monitors,
};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DIB_RGB_COLORS, EndPaint, PAINTSTRUCT, SetDIBitsToDevice, UpdateWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, HWND_BOTTOM, LWA_ALPHA, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_PAINT,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

const PROBE_CLASS: &str = "PoratakeDisplayProbe";
const PROBE_WIDTH: usize = TRANSFER_SIZE;
const PROBE_HEIGHT: i32 = 16;
const PROBE_MARGIN: i32 = 8;
const CALIBRATION_ATTEMPTS: usize = 3;
const RETRY_DELAY: Duration = Duration::from_secs(30);
const PREWARM_TIMEOUT: Duration = Duration::from_secs(2);

struct CachedTransfer {
    transfer: Arc<DisplayTransfer>,
    reported_white: f32,
}

struct CalibrationSignal {
    running: Mutex<bool>,
    done: Condvar,
}

impl CalibrationSignal {
    fn begin(&self) -> bool {
        let Ok(mut running) = self.running.lock() else {
            return false;
        };
        if *running {
            return false;
        }

        *running = true;
        true
    }

    fn finish(&self) {
        let Ok(mut running) = self.running.lock() else {
            return;
        };
        *running = false;
        self.done.notify_all();
    }

    fn wait(&self, timeout: Duration) {
        let Ok(mut running) = self.running.lock() else {
            return;
        };
        let deadline = Instant::now() + timeout;

        while *running {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }

            let (guard, result) = self
                .done
                .wait_timeout(running, remaining)
                .unwrap_or_else(|error| error.into_inner());
            running = guard;
            if result.timed_out() {
                break;
            }
        }
    }
}

struct RefreshGuard {
    signal: &'static CalibrationSignal,
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.signal.finish();
    }
}

struct ProbeWindow(HWND);

impl Drop for ProbeWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

pub fn prewarm() {
    let (finished, done) = std::sync::mpsc::channel();
    if start_refresh(Some(finished)) {
        let _ = done.recv_timeout(PREWARM_TIMEOUT);
        return;
    }

    wait_for_refresh(PREWARM_TIMEOUT);
}

pub fn transfer_for(device: &str) -> Option<Arc<DisplayTransfer>> {
    let reported_white = hdr_white_scale(device)?;

    if let Some(transfer) = cached_transfer_exact(device, reported_white) {
        return Some(transfer);
    }

    refresh_in_background();
    if let Some(transfer) = calibrated_for_white(device, reported_white) {
        return Some(transfer);
    }

    Some(Arc::new(DisplayTransfer::from_reported_white(
        reported_white,
    )))
}

pub fn transfer_for_blocking(device: &str) -> Option<Arc<DisplayTransfer>> {
    let reported_white = hdr_white_scale(device)?;

    if let Some(transfer) = cached_transfer_exact(device, reported_white) {
        return Some(transfer);
    }

    refresh_in_background();
    wait_for_refresh(PREWARM_TIMEOUT);

    if let Some(transfer) = cached_transfer_exact(device, reported_white) {
        return Some(transfer);
    }
    if let Some(transfer) = calibrated_for_white(device, reported_white) {
        return Some(transfer);
    }

    Some(Arc::new(DisplayTransfer::from_reported_white(
        reported_white,
    )))
}

pub fn refresh_in_background() {
    let _ = start_refresh(None);
}

fn wait_for_refresh(timeout: Duration) {
    calibration_signal().wait(timeout);
}

fn start_refresh(finished: Option<std::sync::mpsc::Sender<()>>) -> bool {
    let signal = calibration_signal();
    if !signal.begin() {
        return false;
    }

    let spawned = std::thread::Builder::new()
        .name("display-calibration".into())
        .spawn(move || {
            let _guard = RefreshGuard { signal };
            calibrate_all();
            if let Some(finished) = finished {
                let _ = finished.send(());
            }
        });
    if spawned.is_err() {
        signal.finish();
        return false;
    }

    true
}

fn calibrate_all() {
    for monitor in monitors() {
        let Some(reported_white) = hdr_white_scale(&monitor.device) else {
            continue;
        };
        if cached_transfer_exact(&monitor.device, reported_white).is_some() {
            continue;
        }
        if !should_attempt(&monitor.device) {
            continue;
        }

        let Some(transfer) = measure(monitor.rect) else {
            continue;
        };
        store_transfer(&monitor.device, reported_white, transfer);
        clear_attempt(&monitor.device);
    }
}

fn cached_transfer_exact(device: &str, reported_white: f32) -> Option<Arc<DisplayTransfer>> {
    let cache = cache().lock().ok()?;
    let entry = cache.get(device)?;

    if (entry.reported_white - reported_white).abs() > 0.001 {
        return None;
    }

    Some(Arc::clone(&entry.transfer))
}

fn calibrated_for_white(device: &str, reported_white: f32) -> Option<Arc<DisplayTransfer>> {
    let cache = cache().lock().ok()?;
    let entry = cache.get(device)?;

    if (entry.reported_white - reported_white).abs() <= 0.001 {
        return Some(Arc::clone(&entry.transfer));
    }

    let ratio = reported_white / entry.reported_white;
    Some(Arc::new(entry.transfer.with_scaled_white(ratio)?))
}

fn store_transfer(device: &str, reported_white: f32, transfer: Arc<DisplayTransfer>) {
    let Ok(mut cache) = cache().lock() else {
        return;
    };

    cache.insert(
        device.to_string(),
        CachedTransfer {
            transfer,
            reported_white,
        },
    );
}

fn cache() -> &'static Mutex<HashMap<String, CachedTransfer>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedTransfer>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn calibration_signal() -> &'static CalibrationSignal {
    static SIGNAL: OnceLock<CalibrationSignal> = OnceLock::new();
    SIGNAL.get_or_init(|| CalibrationSignal {
        running: Mutex::new(false),
        done: Condvar::new(),
    })
}

fn should_attempt(device: &str) -> bool {
    let Ok(mut attempts) = attempts().lock() else {
        return false;
    };
    let now = Instant::now();

    if let Some(previous) = attempts.get(device)
        && now.duration_since(*previous) < RETRY_DELAY
    {
        return false;
    }

    attempts.insert(device.to_string(), now);
    true
}

fn clear_attempt(device: &str) {
    if let Ok(mut attempts) = attempts().lock() {
        attempts.remove(device);
    }
}

fn attempts() -> &'static Mutex<HashMap<String, Instant>> {
    static ATTEMPTS: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn measure(rect: RECT) -> Option<Arc<DisplayTransfer>> {
    for attempt in 0..CALIBRATION_ATTEMPTS {
        let Some(window) = create_probe_window(rect) else {
            continue;
        };
        let window = ProbeWindow(window);
        unsafe {
            let _ = DwmFlush();
        }

        let frame = capture_window_linear(window.0);
        drop(window);
        let Ok(frame) = frame else {
            continue;
        };
        let Some(samples) = sample_columns(&frame) else {
            continue;
        };
        if let Some(transfer) = DisplayTransfer::from_probe(&samples) {
            return Some(Arc::new(transfer));
        }

        crate::trace::trace(&format!(
            "display calibration attempt {} produced an unusable curve",
            attempt + 1
        ));
    }

    crate::trace::trace("display calibration failed");
    None
}

fn create_probe_window(bounds: RECT) -> Option<HWND> {
    ensure_window_class(PROBE_CLASS, Some(probe_wndproc), None);

    let rect = RECT {
        left: bounds.left + PROBE_MARGIN,
        top: bounds.top + PROBE_MARGIN,
        right: bounds.left + PROBE_MARGIN + PROBE_WIDTH as i32,
        bottom: bounds.top + PROBE_MARGIN + PROBE_HEIGHT,
    };
    let style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    let window = create_popup_window(PROBE_CLASS, style, &rect)?;

    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), 0, LWA_ALPHA);
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(window);
        let _ = SetWindowPos(
            window,
            Some(HWND_BOTTOM),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    Some(window)
}

unsafe extern "system" fn probe_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        paint_probe(window);
        return LRESULT(0);
    }
    if message == WM_ERASEBKGND {
        return LRESULT(1);
    }

    default_wndproc(window, message, wparam, lparam)
}

fn paint_probe(window: HWND) {
    let pixels = probe_ramp();
    let info = bitmap_info(PROBE_WIDTH as i32, PROBE_HEIGHT, pixels.len() as u32);

    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        let _ = SetDIBitsToDevice(
            dc,
            0,
            0,
            PROBE_WIDTH as u32,
            PROBE_HEIGHT as u32,
            0,
            0,
            0,
            PROBE_HEIGHT as u32,
            pixels.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
        );
        let _ = EndPaint(window, &paint);
    }
}

fn probe_ramp() -> &'static [u8] {
    static RAMP: OnceLock<Box<[u8]>> = OnceLock::new();
    RAMP.get_or_init(|| {
        let mut pixels = vec![0u8; PROBE_WIDTH * PROBE_HEIGHT as usize * 4];
        for row in pixels.as_chunks_mut::<{ PROBE_WIDTH * 4 }>().0 {
            for (code, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                pixel[0] = code as u8;
                pixel[1] = code as u8;
                pixel[2] = code as u8;
                pixel[3] = u8::MAX;
            }
        }
        pixels.into_boxed_slice()
    })
}

fn sample_columns(frame: &LinearFrame) -> Option<[f32; TRANSFER_SIZE]> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }

    let expected = frame.width as usize * frame.height as usize * 4;
    if frame.pixels.len() != expected {
        return None;
    }

    let row = frame.height as usize / 2;
    let mut samples = [0f32; TRANSFER_SIZE];
    for (code, sample) in samples.iter_mut().enumerate() {
        let column = code * (frame.width as usize - 1) / (TRANSFER_SIZE - 1);
        let offset = (row * frame.width as usize + column) * 4;
        *sample = frame.pixels[offset];
    }

    Some(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32, pixel: impl Fn(u32, u32) -> f32) -> LinearFrame {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let value = pixel(x, y);
                pixels.extend_from_slice(&[value, value, value, 1.0]);
            }
        }

        LinearFrame {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn samples_every_column_of_a_wide_probe() {
        let frame = frame(PROBE_WIDTH as u32, PROBE_HEIGHT as u32, |x, _| x as f32);
        let samples = sample_columns(&frame).expect("samples");
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[128], 128.0);
        assert_eq!(samples[255], 255.0);
    }

    #[test]
    fn samples_the_middle_row() {
        let frame = frame(PROBE_WIDTH as u32, PROBE_HEIGHT as u32, |x, y| {
            if y == PROBE_HEIGHT as u32 / 2 {
                x as f32
            } else {
                -1.0
            }
        });
        let samples = sample_columns(&frame).expect("samples");
        assert_eq!(samples[64], 64.0);
    }

    #[test]
    fn rejects_a_mismatched_buffer() {
        let frame = LinearFrame {
            width: 2,
            height: 2,
            pixels: vec![0.0; 3],
        };
        assert!(sample_columns(&frame).is_none());
    }

    #[test]
    fn cache_matches_the_measured_white() {
        let device = "test-cache-white";
        let transfer = Arc::new(DisplayTransfer::from_reported_white(6.0));
        store_transfer(device, 6.0, Arc::clone(&transfer));

        let cached = cached_transfer_exact(device, 6.0).expect("matching cache");
        assert!(Arc::ptr_eq(&cached, &transfer));
        assert!(cached_transfer_exact(device, 3.0).is_none());
    }

    #[test]
    fn white_change_rescales_the_measured_curve() {
        let device = "test-cache-rescale";
        let samples = crate::display_color::probe_samples(2.4, 6.0);
        let transfer = DisplayTransfer::from_probe(&samples).expect("gamma transfer");
        store_transfer(device, 6.0, Arc::new(transfer));

        let scaled = calibrated_for_white(device, 3.0).expect("scaled transfer");
        assert_eq!(scaled.white(), 3.0);

        for (code, sample) in crate::display_color::probe_samples(2.4, 3.0)
            .iter()
            .enumerate()
        {
            let mapped = scaled.encode(*sample, *sample, *sample);
            assert!(mapped[0].abs_diff(code as u8) <= 1);
        }
    }
}
