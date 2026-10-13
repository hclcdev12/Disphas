//! Client side of the UI ↔ helper protocol (`docs/protocole_ipc.md`).
//!
//! The app runs as the normal user. It launches the root helper once per
//! session, writes requests on its stdin and reads responses and events on
//! its stdout. It never touches a device and never decides anything.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command as Process, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use disphas_protocol::command::Command;
use disphas_protocol::response::{ErrorBody, ErrorCode};
use disphas_protocol::{Event, MAX_MESSAGE_BYTES, Request, Response, ServerMessage};
use tokio::sync::oneshot;

/// Longest wait for one response. Erasures run as jobs: no command blocks long.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

/// Installed location of the helper, launched through pkexec in release builds.
#[cfg(not(debug_assertions))]
const INSTALLED_HELPER: &str = "/usr/libexec/disphas/disphas-helper";

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>>;

/// A running helper.
pub struct Helper {
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    child: Mutex<Child>,
}

fn failure(code: ErrorCode, detail: impl Into<String>) -> ErrorBody {
    ErrorBody {
        code,
        detail: Some(detail.into()),
    }
}

fn unavailable(detail: impl Into<String>) -> ErrorBody {
    // UNAVAILABLE is app-side only; the closest protocol code is INTERNAL.
    failure(ErrorCode::Internal, detail)
}

/// How the helper is launched.
///
/// Development builds start it directly, as the current user, from
/// `DISPHAS_HELPER` or the workspace `target/debug` directory: never through
/// pkexec, so a developer is never asked for root by surprise.
/// Release builds always go through pkexec and the installed path.
fn launcher() -> Process {
    #[cfg(debug_assertions)]
    {
        let path = std::env::var_os("DISPHAS_HELPER")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/disphas-helper")
            });
        Process::new(path)
    }
    #[cfg(not(debug_assertions))]
    {
        let mut process = Process::new("pkexec");
        process.arg(INSTALLED_HELPER);
        process
    }
}

impl Helper {
    /// Launches the helper; `on_event` receives its spontaneous events.
    pub fn spawn(on_event: impl Fn(Event) + Send + 'static) -> Result<Self, String> {
        let mut child = launcher()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("cannot launch the helper: {error}"))?;
        let stdin = child.stdin.take().ok_or("helper stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("helper stdout unavailable")?;

        let pending: Pending = Arc::default();
        let reader_pending = Arc::clone(&pending);
        std::thread::spawn(move || read_loop(stdout, &reader_pending, on_event));

        Ok(Self {
            stdin: Mutex::new(Some(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            child: Mutex::new(child),
        })
    }

    /// Sends `command` and waits for its response.
    pub async fn request(&self, command: Command) -> Result<Response, ErrorBody> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        // An `Unlock` line holds the ATA password: wiped once written.
        let line = zeroize::Zeroizing::new(
            Request { id, command }
                .encode()
                .map_err(|error| failure(ErrorCode::BadRequest, error.to_string()))?,
        );
        let (sender, receiver) = oneshot::channel();
        lock(&self.pending)?.insert(id, sender);

        let written = {
            let mut stdin = lock(&self.stdin)?;
            match stdin.as_mut() {
                Some(stdin) => writeln!(stdin, "{}", line.as_str()).and_then(|()| stdin.flush()),
                None => Err(std::io::Error::other("helper closed")),
            }
        };
        if let Err(error) = written {
            lock(&self.pending)?.remove(&id);
            return Err(unavailable(format!("helper unreachable: {error}")));
        }

        match tokio::time::timeout(RESPONSE_TIMEOUT, receiver).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(unavailable("helper stopped")),
            Err(_) => {
                lock(&self.pending)?.remove(&id);
                Err(unavailable("helper did not answer"))
            }
        }
    }
}

impl Drop for Helper {
    /// Closing stdin tells the helper the UI is gone (§3): it stops, or
    /// finishes a running job first. It is never killed.
    fn drop(&mut self) {
        if let Ok(mut stdin) = self.stdin.lock() {
            stdin.take();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.try_wait();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, ErrorBody> {
    mutex
        .lock()
        .map_err(|_| unavailable("helper client state poisoned"))
}

/// Reads the helper's stdout until it closes. Responses go to their waiting
/// request, events to `on_event`. Anything malformed ends the session.
fn read_loop(stdout: impl Read, pending: &Pending, on_event: impl Fn(Event)) {
    let mut reader = BufReader::new(stdout);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_MESSAGE_BYTES as u64)
            .read_until(b'\n', &mut line);
        match read {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let message = std::str::from_utf8(&line)
            .ok()
            .and_then(|text| ServerMessage::decode(text).ok());
        match message {
            Some(ServerMessage::Response(response)) => {
                let waiting = pending
                    .lock()
                    .ok()
                    .and_then(|mut map| map.remove(&response.id));
                if let Some(sender) = waiting {
                    let _ = sender.send(response);
                }
            }
            Some(ServerMessage::Event(event)) => on_event(event),
            None => {
                eprintln!("disphas-app: malformed message from the helper, closing");
                break;
            }
        }
    }
    // Dropping the senders wakes every waiting request with "helper stopped".
    if let Ok(mut map) = pending.lock() {
        map.clear();
    }
}
