//! Optional file-backed tracer for field debugging capture pipelines.
//!
//! Enabled by setting `PORATAKE_TRACE_FILE` to a writable path. All writers
//! push onto an unbounded channel; one dedicated logger thread owns the file,
//! so tracing never blocks on filesystem quirks regardless of which thread
//! calls it.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

fn trace_path() -> Option<&'static PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let value = std::env::var_os("PORATAKE_TRACE_FILE")?;
        if value.is_empty() {
            None
        } else {
            Some(PathBuf::from(value))
        }
    })
    .as_ref()
}

fn sender() -> Option<&'static Sender<String>> {
    static SENDER: OnceLock<Option<Sender<String>>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            let path = trace_path()?.clone();
            let (tx, rx) = channel::<String>();
            std::thread::Builder::new()
                .name("trace-logger".into())
                .spawn(move || {
                    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path)
                    else {
                        return;
                    };
                    loop {
                        match rx.recv() {
                            Ok(line) => {
                                let _ = writeln!(file, "{line}");
                                let _ = file.flush();
                            }
                            Err(_) => break,
                        }
                    }
                })
                .ok()?;
            Some(tx)
        })
        .as_ref()
}

/// Appends one line to the trace file when tracing is enabled.
pub fn trace(message: &str) {
    let Some(tx) = sender() else {
        return;
    };
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let _ = tx.send(format!("{millis} {}", message));
}
