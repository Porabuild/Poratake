use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_APP,
};

type Job = Box<dyn FnOnce() + Send + 'static>;

const WM_JOB: u32 = WM_APP + 1;

struct UiThread {
    thread_id: u32,
    jobs: Mutex<VecDeque<Job>>,
}

static UI: OnceLock<UiThread> = OnceLock::new();

pub fn init() {
    UI.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel::<u32>();

        std::thread::spawn(move || {
            unsafe {
                let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            }

            let mut message = MSG::default();
            unsafe {
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
            }

            let _ = sender.send(unsafe { GetCurrentThreadId() });

            loop {
                let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
                if result.0 <= 0 {
                    break;
                }

                if message.hwnd.is_invalid() && message.message == WM_JOB {
                    run_pending_jobs();
                    continue;
                }

                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        });

        let thread_id = receiver.recv().expect("UI thread failed to start");
        UiThread {
            thread_id,
            jobs: Mutex::new(VecDeque::new()),
        }
    });
}

pub fn run_on_ui<F: FnOnce() + Send + 'static>(job: F) {
    init();
    let Some(ui) = UI.get() else {
        return;
    };

    if let Ok(mut jobs) = ui.jobs.lock() {
        jobs.push_back(Box::new(job));
    }

    unsafe {
        let _ = PostThreadMessageW(ui.thread_id, WM_JOB, WPARAM(0), LPARAM(0));
    }
}

fn run_pending_jobs() {
    let Some(ui) = UI.get() else {
        return;
    };

    loop {
        let job = ui.jobs.lock().ok().and_then(|mut jobs| jobs.pop_front());
        let Some(job) = job else {
            break;
        };
        job();
    }
}
