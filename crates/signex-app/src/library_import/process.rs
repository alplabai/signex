//! A converter process and its pipes: started in its own process group so
//! a stop kills everything it started, with stdout read line by line into a
//! bounded queue and stderr's tail kept, neither ever waited on forever.
//!
//! On Windows a Job object stands in for the process group (`job.rs`);
//! when the system refuses one, only the converter itself is killed
//! (`docs/LIBRARY_CONVERTER_CONTRACT.md` §6).

use std::ffi::OsString;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::error::ImportError;
#[cfg(windows)]
use super::job::Job;
use super::run::ConverterCommand;
use super::stream::{Line, LineSplitter};

/// Lines queued between the reader thread and the run loop.
const LINE_QUEUE: usize = 256;

/// Bound on the stderr tail kept for the error message.
const STDERR_TAIL: usize = 4 << 10;

/// How long the pipes may stay open after the converter exited.
pub const PIPE_GRACE: Duration = Duration::from_secs(2);

/// The process group of the converter a run has started, `0` while none
/// runs; the app holds a clone so quitting can kill it
/// ([`ProcessGroup::kill`]). It is cleared before the converter is reaped,
/// so a quit never signals a group id the system has handed out again.
/// On Windows it also carries the run's Job object, if it has one.
#[derive(Debug, Clone, Default)]
pub struct ProcessGroup {
    id: Arc<std::sync::atomic::AtomicU32>,
    #[cfg(windows)]
    job: Arc<Mutex<Option<Arc<Job>>>>,
}

impl ProcessGroup {
    fn set(&self, group: u32) {
        self.id.store(group, Ordering::Release);
    }

    #[cfg(windows)]
    fn set_job(&self, job: Option<Arc<Job>>) {
        *self.job.lock().unwrap_or_else(PoisonError::into_inner) = job;
    }

    /// Nothing runs any more: forget the group (and the job).
    pub(super) fn clear(&self) {
        self.set(0);
        #[cfg(windows)]
        self.set_job(None);
    }

    /// The group of the converter running now, if one is.
    pub fn current(&self) -> Option<u32> {
        Some(self.id.load(Ordering::Acquire)).filter(|&g| g != 0)
    }

    /// Kill the running converter's whole group (its Job object on
    /// Windows); nothing when none runs. For quitting the app, where the
    /// run's own stop never comes. Only sends the kill, never waits.
    pub fn kill(&self) {
        #[cfg(unix)]
        if let Some(group) = self.current() {
            kill_group_id(group);
        }
        #[cfg(windows)]
        self.terminate_job();
    }

    /// A job handle stays valid while held, so unlike a group id it can
    /// never name someone else's processes.
    #[cfg(windows)]
    fn terminate_job(&self) {
        if let Some(job) = &*self.job.lock().unwrap_or_else(PoisonError::into_inner) {
            job.terminate();
        }
    }
}

#[cfg(unix)]
fn kill_group_id(group: u32) {
    let Ok(group) = libc::pid_t::try_from(group) else {
        return;
    };
    // SAFETY: killpg only sends a signal, to the group the converter was
    // started in (`process_group(0)`).
    if unsafe { libc::killpg(group, libc::SIGKILL) } != 0 {
        log::debug!(
            "library import: killpg: {}",
            std::io::Error::last_os_error()
        );
    }
}

/// A started converter.
pub struct Running {
    child: Child,
    lines: mpsc::Receiver<Line>,
    stdout_open: bool,
    stderr: Arc<Mutex<Vec<u8>>>,
    stderr_done: Arc<AtomicBool>,
    /// Where the converter's group id is published while it is unreaped.
    published: Option<ProcessGroup>,
    /// The job the converter and what it starts run in; dropping the last
    /// handle kills them. `None` when the system refused one.
    #[cfg(windows)]
    job: Option<Arc<Job>>,
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
    /// `cwd` is a neutral folder, never the app's own working directory.
    pub fn spawn(
        command: &ConverterCommand,
        args: &[OsString],
        cwd: &std::path::Path,
        stdout_cap: usize,
    ) -> Result<Self, ImportError> {
        #[allow(
            clippy::disallowed_methods,
            reason = "runs an external library converter, not a browser or viewer"
        )]
        let mut builder = Command::new(&command.program);
        builder
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            builder.process_group(0);
        }
        // Started suspended, so nothing runs before it is in its job.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            builder.creation_flags(super::job::CREATE_SUSPENDED);
        }
        let mut child = builder.spawn().map_err(|e| ImportError::StartFailed {
            program: command.program.clone(),
            detail: e.to_string(),
        })?;
        #[cfg(windows)]
        let job = Job::holding(&child).map(Arc::new);
        #[cfg(windows)]
        if let Err(e) = super::job::resume(&child) {
            // A converter that cannot be resumed would hang the run.
            if let Err(why) = child.kill() {
                log::warn!("library import: the suspended converter was not killed: {why}");
            }
            if let Err(why) = child.wait() {
                log::warn!("library import: the suspended converter was not reaped: {why}");
            }
            return Err(ImportError::StartFailed {
                program: command.program.clone(),
                detail: e.to_string(),
            });
        }
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
            published: None,
            #[cfg(windows)]
            job,
        })
    }

    /// Publish the converter's group id in `group` until it is reaped
    /// ([`Self::try_wait`] seeing it exit, or [`Self::stop`]).
    pub fn publish(&mut self, group: &ProcessGroup) {
        group.set(self.child.id());
        #[cfg(windows)]
        group.set_job(self.job.clone());
        self.published = Some(group.clone());
    }

    /// Clear the published group id: the converter is about to be, or has
    /// been, reaped, after which its id may be reused.
    fn unpublish(&mut self) {
        if let Some(group) = self.published.take() {
            group.clear();
        }
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

    /// The exit status once the converter has ended; seeing it reaps the
    /// converter, so the published group id is cleared first thing.
    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait();
        if matches!(status, Ok(Some(_))) {
            self.unpublish();
        }
        status
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
            #[cfg(windows)]
            self.wait_job_empty();
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
        // Killed but not yet reaped: the id is still ours until `wait`.
        self.unpublish();
        if let Err(e) = self.child.wait() {
            log::debug!("library import: reap: {e}");
        }
        #[cfg(windows)]
        self.wait_job_empty();
    }

    /// What the converter started holds files open until it has ended, and
    /// the run removes or restores the output next: wait for the job to
    /// empty, for at most [`PIPE_GRACE`].
    #[cfg(windows)]
    fn wait_job_empty(&self) {
        self.job.iter().for_each(|job| job.wait_empty(PIPE_GRACE));
    }

    /// Kill whatever is left in the converter's process group (its Job
    /// object on Windows). After the converter exited and was reaped the
    /// group id cannot be reused while a member lives, so this only reaches
    /// what it started.
    pub fn kill_group(&self) {
        #[cfg(unix)]
        kill_group_id(self.child.id());
        #[cfg(windows)]
        self.job.iter().for_each(|job| job.terminate());
    }
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
