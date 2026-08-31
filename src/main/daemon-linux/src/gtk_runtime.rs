use std::sync::{Arc, Mutex};
use std::time::Duration;

use glib::ControlFlow;
use gtk::gdk;
use poratake_daemon_common::platform::LinuxBackend;

type GtkTask = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone)]
pub struct GtkRuntime {
    backend: LinuxBackend,
    sender: Arc<Mutex<Option<glib::Sender<GtkTask>>>>,
}

impl GtkRuntime {
    pub fn new(backend: LinuxBackend) -> Self {
        Self {
            backend,
            sender: Arc::new(Mutex::new(None)),
        }
    }

    pub fn dispatch(&self, task: impl FnOnce() + Send + 'static) -> Result<(), String> {
        self.sender()?
            .send(Box::new(task))
            .map_err(|_| "GTK runtime stopped".to_string())
    }

    pub fn backend(&self) -> LinuxBackend {
        self.backend
    }

    fn sender(&self) -> Result<glib::Sender<GtkTask>, String> {
        let mut state = self
            .sender
            .lock()
            .map_err(|_| "GTK runtime state is unavailable".to_string())?;
        if let Some(sender) = state.as_ref() {
            return Ok(sender.clone());
        }
        let sender = start(self.backend)?;
        *state = Some(sender.clone());
        Ok(sender)
    }
}

#[expect(
    deprecated,
    reason = "GLib channels provide an event-driven GTK source"
)]
fn start(backend: LinuxBackend) -> Result<glib::Sender<GtkTask>, String> {
    if backend == LinuxBackend::Headless {
        return Err("GTK needs a graphical Linux session".into());
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
    std::thread::Builder::new()
        .name("linux-gtk-ui".into())
        .spawn(move || {
            gdk::set_allowed_backends(backend.id());
            if let Err(error) = gtk::init() {
                let _ = ready_tx.send(Err(format!("could not initialize GTK: {error}")));
                return;
            }
            let (sender, receiver) =
                glib::MainContext::channel::<GtkTask>(glib::Priority::default());
            receiver.attach(None, |task| {
                task();
                ControlFlow::Continue
            });
            if ready_tx.send(Ok(sender)).is_err() {
                return;
            }
            gtk::main();
        })
        .map_err(|error| format!("could not start GTK: {error}"))?;
    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "GTK initialization timed out".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_runtime_rejects_ui_work() {
        let runtime = GtkRuntime::new(LinuxBackend::Headless);
        assert_eq!(
            runtime.dispatch(|| {}).expect_err("headless GTK"),
            "GTK needs a graphical Linux session"
        );
    }
}
