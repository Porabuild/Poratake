use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine as _;
use gtk::gdk::prelude::GdkContextExt as _;
use gtk::prelude::*;
use poratake_daemon_common::contract::{
    PNG_SIGNATURE, PRINT_MODULE, PrintImageRequest, PrintMethod,
};
use poratake_daemon_common::protocol::{Request, Response, params, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;

use crate::gtk_runtime::GtkRuntime;

static PRINT_ACTIVE: AtomicBool = AtomicBool::new(false);

struct PrintGuard;

impl PrintGuard {
    fn acquire() -> Option<Self> {
        PRINT_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            .then_some(Self)
    }
}

impl Drop for PrintGuard {
    fn drop(&mut self) {
        PRINT_ACTIVE.store(false, Ordering::Release);
    }
}

pub struct PrintModule {
    runtime: GtkRuntime,
}

impl PrintModule {
    pub fn new(runtime: GtkRuntime) -> Self {
        Self { runtime }
    }

    fn print_image(&self, request: &Request) -> Reply {
        let params: PrintImageRequest = match params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };
        let image = match base64::engine::general_purpose::STANDARD.decode(params.image_base64) {
            Ok(image) => image,
            Err(_) => {
                return Reply::Now(Err((
                    "INVALID_IMAGE".into(),
                    "Failed to decode image data".into(),
                )));
            }
        };
        if !image.starts_with(PNG_SIGNATURE) {
            return Reply::Now(Err((
                "INVALID_IMAGE".into(),
                "Failed to decode image data".into(),
            )));
        }
        let Some(guard) = PrintGuard::acquire() else {
            return Reply::Now(Err((
                "PRINT_IN_PROGRESS".into(),
                "Another print dialog is already open".into(),
            )));
        };
        let id = request.id.clone();
        match self.runtime.dispatch(move || print_png(image, id, guard)) {
            Ok(()) => Reply::Deferred,
            Err(error) => Reply::Now(Err(("UI_ERROR".into(), error))),
        }
    }
}

impl Module for PrintModule {
    fn name(&self) -> &'static str {
        PRINT_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match PrintMethod::parse(&request.method) {
            Some(PrintMethod::Image) => self.print_image(request),
            None => method_not_found(&request.method),
        }
    }
}

fn print_png(image: Vec<u8>, id: String, _guard: PrintGuard) {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    if loader.write(&image).is_err() || loader.close().is_err() {
        send_response(Response::error(
            &id,
            "INVALID_IMAGE",
            "Failed to decode image data",
        ));
        return;
    }
    let Some(pixbuf) = loader.pixbuf() else {
        send_response(Response::error(
            &id,
            "INVALID_IMAGE",
            "Failed to decode image data",
        ));
        return;
    };

    let operation = gtk::PrintOperation::new();
    operation.set_job_name("Poratake Screenshot");
    operation.set_n_pages(1);
    operation.set_show_progress(true);
    operation.set_unit(gtk::Unit::Points);
    let setup = gtk::PageSetup::new();
    setup.set_top_margin(36.0, gtk::Unit::Points);
    setup.set_bottom_margin(36.0, gtk::Unit::Points);
    setup.set_left_margin(36.0, gtk::Unit::Points);
    setup.set_right_margin(36.0, gtk::Unit::Points);
    operation.set_default_page_setup(Some(&setup));

    let begin_pixbuf = pixbuf.clone();
    operation.connect_begin_print(move |operation, context| {
        let scale = (context.width() / f64::from(begin_pixbuf.width())).min(1.0);
        let printed_height = f64::from(begin_pixbuf.height()) * scale;
        let pages = (printed_height / context.height()).ceil().max(1.0) as i32;
        operation.set_n_pages(pages);
    });
    operation.connect_draw_page(move |_, context, page| {
        let Some(cairo) = context.cairo_context() else {
            return;
        };
        let scale = (context.width() / f64::from(pixbuf.width())).min(1.0);
        let x = (context.width() - f64::from(pixbuf.width()) * scale) / (2.0 * scale);
        let y = -(f64::from(page) * context.height() / scale);
        cairo.rectangle(0.0, 0.0, context.width(), context.height());
        cairo.clip();
        cairo.scale(scale, scale);
        cairo.set_source_pixbuf(&pixbuf, x, y);
        let _ = cairo.paint();
    });

    send_response(Response::success(&id, Some(json!({ "success": true }))));
    let _ = operation.run(gtk::PrintOperationAction::PrintDialog, None::<&gtk::Window>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use poratake_daemon_common::platform::LinuxBackend;

    fn request(image_base64: &str) -> Request {
        Request {
            id: "print".into(),
            module: PRINT_MODULE.into(),
            method: PrintMethod::Image.id().into(),
            params: serde_json::from_value(json!({ "imageBase64": image_base64 }))
                .expect("print params"),
        }
    }

    #[test]
    fn rejects_invalid_base64_before_starting_gtk() {
        let mut module = PrintModule::new(GtkRuntime::new(LinuxBackend::Headless));
        let Reply::Now(result) = module.handle(&request("not base64")) else {
            panic!("invalid image should respond immediately");
        };
        assert_eq!(result.expect_err("invalid image").0, "INVALID_IMAGE");
    }

    #[test]
    fn rejects_non_png_data_before_starting_gtk() {
        let mut module = PrintModule::new(GtkRuntime::new(LinuxBackend::Headless));
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"not a PNG");
        let Reply::Now(result) = module.handle(&request(&encoded)) else {
            panic!("non-PNG image should respond immediately");
        };
        assert_eq!(result.expect_err("non-PNG image").0, "INVALID_IMAGE");
    }

    #[test]
    fn headless_session_rejects_print_ui() {
        let mut module = PrintModule::new(GtkRuntime::new(LinuxBackend::Headless));
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        let Reply::Now(result) = module.handle(&request(png)) else {
            panic!("headless print should respond immediately");
        };
        assert_eq!(result.expect_err("headless print").0, "UI_ERROR");
    }
}
