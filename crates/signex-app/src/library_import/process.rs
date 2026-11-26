//! A converter process and its pipes: started in its own process group so
//! a stop kills everything it started, with stdout read line by line into a
//! bounded queue and stderr's tail kept, neither ever waited on forever.
//!
//! On Windows only the converter itself is killed; a Job object that takes
//! its children with it is a follow-up (`docs/LIBRARY_CONVERTER_CONTRACT.md`
//! §6).

use std::ffi::OsString;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::error::ImportError;
use super::run::ConverterCommand;
use super::stream::{Line, LineSplitter};

/// Lines queued between the reader thread and the run loop.
const LINE_QUEUE: usize = 256;

/// Bound on the stderr tail kept for the error message.
const STDERR_TAIL: usize = 4 << 10;

/// How long the pipes may stay open after the converter exited.
pub const PIPE_GRACE: Duration = Duration::from_secs(2);

/// A started converter.
pub struct Running {
    child: Child,
    lines: mpsc::Receiver<Line>,
    stdout_open: bool,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_done: Arc<AtomicBool>,
}

/// What one poll of stdout gave.
pub enum Polled {
    Line(Line),
    Nothing,
    /// Stdout reached its end (or its total cap).
    Closed,
}

impl Running {
    /// Start `command` with `args`; stdout stops being read after
    /// `stdout_cap` bytes in total.
    pub fn spawn(
        command: &ConverterCommand,
        args: &[OsString],
        stdout_cap: usize,
    ) -> Result<Self, ImportError> {
        #[allow(
            clippy::disallowed_methods,
            reason = "runs an external library converter, not a browser or viewer"
        )]
        let mut builder = Command::new(&command.program);
        builder
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            builder.process_group(0);
        }
        let mut child = builder.spawn().map_err(|e| ImportError::StartFailed {
            program: command.program.clone(),
            detail: e.to_string(),
        })?;
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let stderr_done = Arc::new(AtomicBool::new(false));
        drain_tail(child.stderr.take(), stderr.clone(), stderr_done.clone());
        let lines = read_lines(child.stdout.take(), stdout_cap);
        Ok(Self {
            child,
            lines,
            stdout_open: true,
            stderr,
            stderr_done,
        })
    }

    pub fn stdout_open(&self) -> bool {
        self.stdout_open
    }

    /// Wait up to `timeout` for the next stdout line.
    pub fn poll_line(&mut self, timeout: Duration) -> Polled {
        if !self.stdout_open {
            std::thread::sleep(timeout);
            return Polled::Closed;
        }
        match self.lines.recv_timeout(timeout) {
            Ok(line) => Polled::Line(line),
            Err(mpsc::RecvTimeoutError::Timeout) => Polled::Nothing,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.stdout_open = false;
                Polled::Closed
            }
        }
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// After the converter exited: hand over the lines still in its pipe for
    /// at most [`PIPE_GRACE`], then stop whatever it left running.
    pub fn drain_after_exit(&mut self, on_line: &mut dyn FnMut(Line)) {
        let deadline = Instant::now() + PIPE_GRACE;
        while self.stdout_open {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            if let Polled::Line(line) = self.poll_line(left.min(Duration::from_millis(50))) {
                on_line(line);
            }
        }
        while !self.stderr_done.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.stdout_open || !self.stderr_done.load(Ordering::Acquire) {
            log::warn!("library import: the converter left a process holding its output; stopped");
            self.kill_group();
        }
    }

    /// The last bytes of stderr read so far.
    pub fn stderr_tail(&self) -> String {
        let kept = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&kept).into_owned()
    }

    /// Kill the converter and everything it started, and reap it.
    pub fn stop(&mut self) {
        self.kill_group();
        if let Err(e) = self.child.kill() {
            log::debug!("library import: kill: {e}");
        }
        if let Err(e) = self.child.wait() {
            log::debug!("library import: reap: {e}");
        }
    }

    #[cfg(unix)]
    fn kill_group(&self) {
        let Ok(group) = libc::pid_t::try_from(self.child.id()) else {
            return;
        };
        // SAFETY: killpg only sends a signal; the group is the one the
        // converter was started in (`process_group(0)`).
        if unsafe { libc::killpg(group, libc::SIGKILL) } != 0 {
            log::debug!(
                "library import: killpg: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    #[cfg(not(unix))]
    fn kill_group(&self) {}
}

/// Read stdout on a thread into a bounded queue that closes at the end of
/// the stream, or once `cap` bytes were read.
fn read_lines<R: Read + Send + 'static>(pipe: Option<R>, cap: usize) -> mpsc::Receiver<Line> {
    let (tx, rx) = mpsc::sync_channel(LINE_QUEUE);
    // A blocking pipe read of a child process inside the import worker: the
    // child would stall on a full pipe if nobody drained it.
    // ast-grep-ignore: no-raw-spawn
    std::thread::spawn(move || {
        let Some(mut pipe) = pipe else {
            return;
        };
        let mut splitter = LineSplitter::default();
        let mut chunk = [0u8; 8192];
        let mut lines = Vec::new();
        let mut total = 0usize;
        loop {
            let n = match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            total += n;
            if total > cap {
                log::warn!("library import: converter output passed {cap} bytes; stopped reading");
                return;
            }
            splitter.push(&chunk[..n], &mut lines);
            for line in lines.drain(..) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        }
        if let Some(last) = splitter.finish()
            && tx.send(last).is_err()
        {
            log::debug!("library import: the run stopped listening");
        }
    });
    rx
}

/// Read stderr to its end on a thread, keeping the last bytes in `kept`.
fn drain_tail<R: Read + Send + 'static>(
    pipe: Option<R>,
    kept: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
) {
    // ast-grep-ignore: no-raw-spawn
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 8192];
            loop {
                let n = match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                let mut kept = kept.lock().unwrap_or_else(PoisonError::into_inner);
                kept.extend_from_slice(&chunk[..n]);
                let excess = kept.len().saturating_sub(STDERR_TAIL);
                kept.drain(..excess);
            }
        }
        done.store(true, Ordering::Release);
    });
}
