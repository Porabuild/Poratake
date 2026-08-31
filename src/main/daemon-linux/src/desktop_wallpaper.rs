use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine as _;
use image::{DynamicImage, ImageFormat};
use poratake_daemon_common::contract::{
    DESKTOP_WALLPAPER_MODULE, DesktopWallpaperMethod, DesktopWallpaperResult,
};
use poratake_daemon_common::protocol::{Request, Response, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};

use crate::Backend;

pub struct DesktopWallpaperModule {
    backend: Backend,
    in_flight: Arc<AtomicBool>,
}

impl DesktopWallpaperModule {
    pub fn new(backend: Backend) -> Self {
        Self {
            backend,
            in_flight: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct WallpaperGuard(Arc<AtomicBool>);

impl Drop for WallpaperGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Module for DesktopWallpaperModule {
    fn name(&self) -> &'static str {
        DESKTOP_WALLPAPER_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match DesktopWallpaperMethod::parse(&request.method) {
            Some(DesktopWallpaperMethod::Get) => {
                if self.backend != Backend::X11 {
                    return Reply::Now(Err((
                        "UNSUPPORTED_SESSION".into(),
                        "Desktop wallpaper is only available in X11 sessions".into(),
                    )));
                }
                if self
                    .in_flight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    return Reply::Now(Err((
                        "BUSY".into(),
                        "A desktop wallpaper request is already in progress".into(),
                    )));
                }
                let backend = self.backend;
                let id = request.id.clone();
                let worker_flag = self.in_flight.clone();
                let spawned = std::thread::Builder::new()
                    .name("linux-wallpaper".into())
                    .spawn(move || {
                        let guard = WallpaperGuard(worker_flag);
                        let response = match std::panic::catch_unwind(|| {
                            wallpaper_data_url(backend)
                        }) {
                            Ok(Ok(value)) => Response::success(
                                &id,
                                serde_json::to_value(DesktopWallpaperResult::Data(value)).ok(),
                            ),
                            Ok(Err(error)) => {
                                Response::error(&id, "WALLPAPER_UNAVAILABLE", &error.to_string())
                            }
                            Err(_) => Response::error(
                                &id,
                                "WALLPAPER_UNAVAILABLE",
                                "Linux wallpaper worker panicked",
                            ),
                        };
                        send_response(response);
                        drop(guard);
                    });
                match spawned {
                    Ok(_) => Reply::Deferred,
                    Err(error) => {
                        self.in_flight.store(false, Ordering::Release);
                        Reply::Now(Err((
                            "WALLPAPER_UNAVAILABLE".into(),
                            format!("Failed to start the Linux wallpaper worker: {error}"),
                        )))
                    }
                }
            }
            None => method_not_found(&request.method),
        }
    }
}

fn wallpaper_data_url(backend: Backend) -> anyhow::Result<String> {
    let image = crate::capture::desktop_wallpaper(backend)?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut png, ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png.into_inner())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str) -> Request {
        Request {
            id: "wallpaper".into(),
            module: DESKTOP_WALLPAPER_MODULE.into(),
            method: method.into(),
            params: None,
        }
    }

    #[test]
    fn rejects_non_x11_sessions() {
        let mut module = DesktopWallpaperModule::new(Backend::Wayland);
        let Reply::Now(result) = module.handle(&request(DesktopWallpaperMethod::Get.id())) else {
            panic!("wallpaper response should be immediate");
        };
        assert_eq!(
            result
                .expect_err("Wayland wallpaper should be unsupported")
                .0,
            "UNSUPPORTED_SESSION"
        );
    }

    #[test]
    fn rejects_unknown_methods() {
        let mut module = DesktopWallpaperModule::new(Backend::X11);
        let Reply::Now(result) = module.handle(&request("unknown")) else {
            panic!("unknown method should respond immediately");
        };
        assert_eq!(result.expect_err("unknown method").0, "METHOD_NOT_FOUND");
    }
}
