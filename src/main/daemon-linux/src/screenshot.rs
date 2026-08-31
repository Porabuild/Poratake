use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use poratake_daemon_common::contract::{
    SCREENSHOT_MODULE, ScreenshotLinuxMethod, ScreenshotMethod,
};
use poratake_daemon_common::geometry::{CaptureAreaRequest, CaptureWindowRequest};
use poratake_daemon_common::protocol::{Request, Response, params, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;

use crate::Backend;

pub struct ScreenshotModule {
    backend: Backend,
    frozen: crate::capture::FrozenFrames,
    capture_in_flight: Arc<AtomicBool>,
}

struct CaptureGuard(Arc<AtomicBool>);

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn spawn_capture(
    in_flight: &Arc<AtomicBool>,
    id: String,
    capture: impl FnOnce() -> Result<std::path::PathBuf> + Send + 'static,
) -> Reply {
    if in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Reply::Now(Err((
            "BUSY".into(),
            "A Linux capture is already in progress".into(),
        )));
    }
    let worker_flag = in_flight.clone();
    let spawned = std::thread::Builder::new()
        .name("linux-capture".into())
        .spawn(move || {
            let guard = CaptureGuard(worker_flag);
            let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(capture)) {
                Ok(Ok(path)) => Response::success(&id, Some(json!({ "path": path }))),
                Ok(Err(error)) => Response::error(&id, "CAPTURE_FAILED", &format!("{error:#}")),
                Err(_) => Response::error(&id, "CAPTURE_FAILED", "Linux capture worker panicked"),
            };
            drop(guard);
            send_response(response);
        });
    match spawned {
        Ok(_) => Reply::Deferred,
        Err(error) => {
            in_flight.store(false, Ordering::Release);
            Reply::Now(Err((
                "CAPTURE_FAILED".into(),
                format!("Failed to start the Linux capture: {error}"),
            )))
        }
    }
}

impl ScreenshotModule {
    pub fn new(backend: Backend, frozen: crate::capture::FrozenFrames) -> Self {
        Self {
            backend,
            frozen,
            capture_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Module for ScreenshotModule {
    fn name(&self) -> &'static str {
        SCREENSHOT_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScreenshotMethod::parse(&request.method) {
            Some(ScreenshotMethod::CaptureArea) => {
                let capture: CaptureAreaRequest = match params(request) {
                    Ok(capture) => capture,
                    Err(error) => return Reply::Now(Err(error)),
                };
                let backend = self.backend;
                let frozen = self.frozen.clone();
                let id = request.id.clone();
                spawn_capture(&self.capture_in_flight, id, move || {
                    crate::capture::capture_area_to_file(backend, &capture, &frozen)?;
                    Ok(capture.path)
                })
            }
            Some(ScreenshotMethod::CaptureWindow) => {
                let capture: CaptureWindowRequest = match params(request) {
                    Ok(capture) => capture,
                    Err(error) => return Reply::Now(Err(error)),
                };
                let backend = self.backend;
                let id = request.id.clone();
                spawn_capture(&self.capture_in_flight, id, move || {
                    crate::capture::capture_window_to_file(backend, &capture)?;
                    Ok(capture.path)
                })
            }
            None => match ScreenshotLinuxMethod::parse(&request.method) {
                Some(ScreenshotLinuxMethod::ListDisplays) => {
                    match crate::capture::list_displays(self.backend) {
                        Ok(displays) => Reply::Now(Ok(Some(json!({ "displays": displays })))),
                        Err(error) => {
                            Reply::Now(Err(("DISPLAY_LIST_FAILED".into(), format!("{error:#}"))))
                        }
                    }
                }
                None => method_not_found(&request.method),
            },
        }
    }
}
