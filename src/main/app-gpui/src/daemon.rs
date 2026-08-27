//! Port of `src/main/daemon/index.ts` — spawns the platform daemon
//! (`poratake-daemon`) and speaks the same newline-delimited JSON-RPC
//! protocol over stdin/stdout, with timeouts, event fan-out and restart
//! backoff.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::{json, Value};

const REQUEST_TIMEOUT_MS: u64 = 30_000;
const MAX_RESTART_ATTEMPTS: u32 = 5;
const RESTART_BACKOFF_BASE_MS: u64 = 1_000;

pub type EventHandler = Arc<dyn Fn(&str, &Value) + Send + Sync>;

struct Pending {
    sender: SyncSender<std::result::Result<Value, String>>,
}

#[derive(Clone)]
pub struct DaemonHandle {
    inner: Arc<DaemonInner>,
}

struct DaemonInner {
    lifecycle: Mutex<()>,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<std::process::ChildStdin>>,
    pending: Mutex<HashMap<String, Pending>>,
    next_id: AtomicU64,
    shutting_down: AtomicBool,
    restart_attempts: AtomicU32,
    event_handlers: Mutex<Vec<EventHandler>>,
    binary_path: PathBuf,
}

fn find_daemon_binary() -> PathBuf {
    if let Ok(path) = std::env::var("PORATAKE_DAEMON_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    const BINARY: &str = if cfg!(windows) {
        "poratake-daemon.exe"
    } else {
        "poratake-daemon"
    };

    // Packaged layout: <root>/daemon/poratake-daemon(.exe) next to the app.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let packaged = dir.join("daemon").join(BINARY);
            if packaged.exists() {
                return packaged;
            }
            for ancestors in dir.ancestors().skip(1) {
                let candidate = ancestors
                    .join("src")
                    .join("main")
                    .join("daemon")
                    .join(BINARY);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    PathBuf::from(format!("src/main/daemon/{BINARY}"))
}

impl DaemonHandle {
    pub fn new() -> Self {
        Self::with_binary(find_daemon_binary())
    }

    pub fn with_binary(binary_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(DaemonInner {
                lifecycle: Mutex::new(()),
                child: Mutex::new(None),
                stdin: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                shutting_down: AtomicBool::new(false),
                restart_attempts: AtomicU32::new(0),
                event_handlers: Mutex::new(Vec::new()),
                binary_path,
            }),
        }
    }

    pub fn on_event(&self, handler: EventHandler) {
        self.inner.event_handlers.lock().push(handler);
    }

    pub fn is_running(&self) -> bool {
        self.inner.child.lock().is_some()
    }

    /// Kills stale daemons from previous runs pointing at this exact binary.
    fn kill_stale_processes(binary_path: &Path) {
        if !cfg!(windows) {
            return;
        }
        let script = "$target = [IO.Path]::GetFullPath($env:PORATAKE_DAEMON_PATH); Get-Process -Name 'poratake-daemon' -ErrorAction SilentlyContinue | Where-Object { try { $_.Path -and [string]::Equals([IO.Path]::GetFullPath($_.Path), $target, [StringComparison]::OrdinalIgnoreCase) } catch { $false } } | Stop-Process -Force";
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("PORATAKE_DAEMON_PATH", binary_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    /// Starts the daemon and waits for its `system:ready` event.
    pub fn start(&self) -> Result<()> {
        let _lifecycle = self.inner.lifecycle.lock();
        if self.inner.child.lock().is_some() {
            return Ok(());
        }

        self.inner.shutting_down.store(false, Ordering::SeqCst);
        Self::kill_stale_processes(&self.inner.binary_path);

        let mut child = Command::new(&self.inner.binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| anyhow!("failed to spawn daemon: {error}"))?;

        let stdin = child.stdin.take().expect("daemon stdin");
        let stdout = child.stdout.take().expect("daemon stdout");

        *self.inner.child.lock() = Some(child);
        *self.inner.stdin.lock() = Some(stdin);

        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(8);
        self.on_event(Arc::new(move |event, _| {
            if event == "system:ready" {
                ready_tx.send(()).ok();
            }
        }));

        self.spawn_reader(stdout);
        self.watch_child_by_pid(Self::pid_of_current(&self.inner));

        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(()) => {
                self.inner.restart_attempts.store(0, Ordering::SeqCst);
                Ok(())
            }
            Err(_) => {
                let mut child = self.inner.child.lock().take();
                *self.inner.stdin.lock() = None;
                if let Some(child) = child.as_mut() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                self.inner.reject_all_pending("Daemon ready timeout");
                Err(anyhow!("Daemon ready timeout"))
            }
        }
    }

    fn pid_of_current(inner: &Arc<DaemonInner>) -> Option<u32> {
        inner.child.lock().as_ref().map(|child| child.id())
    }

    fn spawn_reader(&self, stdout: std::process::ChildStdout) {
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("daemon-reader".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) if !line.trim().is_empty() => {
                            inner.handle_line(&line);
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            })
            .ok();
    }

    fn watch_child_by_pid(&self, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("daemon-watchdog".into())
            .spawn(move || {
                // Poll for exit; std has no async wait without blocking a
                // dedicated thread, which is exactly what this is.
                loop {
                    thread::sleep(Duration::from_millis(250));
                    let status = {
                        let mut guard = inner.child.lock();
                        match guard.as_mut() {
                            Some(child) if child.id() == pid => {
                                Some(matches!(child.try_wait(), Ok(Some(_))))
                            }
                            _ => None,
                        }
                    };
                    let Some(exited) = status else {
                        break;
                    };
                    if exited {
                        inner.handle_exit();
                        break;
                    }
                }
            })
            .ok();
    }

    pub fn stop(&self) {
        let _lifecycle = self.inner.lifecycle.lock();
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        self.send_raw(&json!({
            "id": "quit",
            "module": "system",
            "method": "quit"
        }));
        thread::sleep(Duration::from_millis(500));

        let mut guard = self.inner.child.lock();
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
        }
        *guard = None;
        *self.inner.stdin.lock() = None;
        self.inner.reject_all_pending("Daemon stopped");
    }

    /// Calls `module.method`, returning the parsed `result`.
    pub fn call(&self, module: &str, method: &str, params: Option<Value>) -> Result<Value> {
        self.call_with_timeout(
            module,
            method,
            params,
            Duration::from_millis(REQUEST_TIMEOUT_MS),
        )
    }

    pub fn call_with_timeout(
        &self,
        module: &str,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value> {
        let id = format!("req-{}", self.inner.next_id.fetch_add(1, Ordering::SeqCst));
        let request = json!({
            "id": id,
            "module": module,
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });

        let (tx, rx) = std::sync::mpsc::sync_channel::<std::result::Result<Value, String>>(1);
        self.inner
            .pending
            .lock()
            .insert(id.clone(), Pending { sender: tx });

        if let Err(error) = self.send_raw_value(&request) {
            self.inner.pending.lock().remove(&id);
            return Err(anyhow!("Daemon stdin write failed: {error}"));
        }

        match rx.recv_timeout(timeout) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(message)) => Err(anyhow!("{message}")),
            Err(RecvTimeoutError::Timeout) => {
                self.inner.pending.lock().remove(&id);
                Err(anyhow!("Request timeout: {module}.{method}"))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(anyhow!("Request dropped: {module}.{method}"))
            }
        }
    }

    fn send_raw_value(&self, request: &Value) -> Result<()> {
        let mut line = serde_json::to_string(request)?;
        line.push('\n');
        let mut guard = self.inner.stdin.lock();
        match guard.as_mut() {
            Some(stdin) => stdin.write_all(line.as_bytes()).map_err(Into::into),
            None => Err(anyhow!("Daemon stdin not writable")),
        }
    }

    fn send_raw(&self, request: &Value) -> bool {
        self.send_raw_value(request).is_ok()
    }
}

impl DaemonInner {
    fn handle_line(self: &Arc<Self>, line: &str) {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                eprintln!("[daemon] Failed to parse: {line}");
                return;
            }
        };

        if let Some(event) = message.get("event").and_then(Value::as_str) {
            let data = message.get("data").cloned().unwrap_or(Value::Null);
            for handler in self.event_handlers.lock().iter() {
                handler(event, &data);
            }
            return;
        }

        let id = match message.get("id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => return,
        };

        let pending = self.pending.lock().remove(&id);
        if let Some(pending) = pending {
            let outcome = if message.get("success").and_then(Value::as_bool) == Some(true) {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            } else {
                let error = message.get("error");
                let message_text = error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown error")
                    .to_string();
                Err(message_text)
            };
            // Ignore send failures when the caller already timed out.
            match pending.sender.try_send(outcome) {
                Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }

    fn handle_exit(self: &Arc<Self>) {
        *self.child.lock() = None;
        *self.stdin.lock() = None;
        self.reject_all_pending("Daemon exited");

        if self.shutting_down.load(Ordering::SeqCst) {
            return;
        }

        self.schedule_restart();
    }

    fn schedule_restart(self: &Arc<Self>) {
        let attempts = self.restart_attempts.fetch_add(1, Ordering::SeqCst);
        if attempts >= MAX_RESTART_ATTEMPTS {
            eprintln!("[daemon] Max restart attempts reached");
            return;
        }

        let delay = RESTART_BACKOFF_BASE_MS * 2u64.saturating_pow(attempts);
        eprintln!(
            "[daemon] Restarting in {delay}ms (attempt {})",
            attempts + 1
        );
        thread::sleep(Duration::from_millis(delay));

        if self.shutting_down.load(Ordering::SeqCst) || self.child.lock().is_some() {
            return;
        }

        let handle = DaemonHandle {
            inner: self.clone(),
        };
        if let Err(error) = handle.start() {
            eprintln!("[daemon] restart failed: {error}");
            self.schedule_restart();
        } else {
            self.restart_attempts.store(0, Ordering::SeqCst);
        }
    }

    fn reject_all_pending(&self, message: &str) {
        let mut pending = self.pending.lock();
        for (_, entry) in pending.drain() {
            let _ = entry.sender.try_send(Err(message.to_string()));
        }
    }
}
