//! One `library` run of a converter (contract §2.2, §3): spawn it, read its
//! progress as it comes, stop it when it goes silent or is cancelled, and
//! map its summary and exit code. Blocking; call from `spawn_blocking`.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::confine::{ConfineError, confine};
use super::error::ImportError;
use super::events::{ExitKind, ProgressEvent, StreamFold, Summary};
use super::report::{ImportReport, read_report};
use super::stream::{Line, LineSplitter};

/// How long a run may go without an event before it is stopped (§3.1).
pub const SILENCE_LIMIT: Duration = Duration::from_secs(120);

/// How long `describe` may take.
pub const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound on the stderr tail kept for the error message.
const STDERR_TAIL: usize = 4 << 10;

/// The program to run and what goes before the contract's own arguments
/// (nothing for an installed converter; the script for a test's `sh`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConverterCommand {
    pub program: PathBuf,
    pub leading_args: Vec<OsString>,
}

impl ConverterCommand {
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            leading_args: Vec::new(),
        }
    }
}

/// `--models`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelsMode {
    #[default]
    Include,
    Skip,
}

/// What the run is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// `--dry-run`: only the report is written, for the preview.
    Preview,
    /// A new library in an absent or empty folder.
    Import,
    /// `--update` over a library this converter wrote from the same source.
    Update,
}

/// One `library` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRunRequest {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub name: Option<String>,
    /// A file path, a connection string or a CSV folder, passed as given.
    pub db: Option<String>,
    pub search_paths: Vec<PathBuf>,
    pub models: ModelsMode,
    pub mode: RunMode,
}

impl LibraryRunRequest {
    /// Refuse a relative input or folder: an absolute path can never be
    /// taken for an option.
    pub fn validate(&self) -> Result<(), ImportError> {
        let paths = self
            .inputs
            .iter()
            .chain(&self.search_paths)
            .chain(std::iter::once(&self.output));
        for path in paths {
            if !path.is_absolute() {
                return Err(ImportError::PathNotAbsolute { path: path.clone() });
            }
        }
        Ok(())
    }

    /// The contract arguments, after `library`.
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec!["library".into()];
        args.extend(self.inputs.iter().map(OsString::from));
        args.push("--output".into());
        args.push(self.output.clone().into());
        if let Some(name) = &self.name {
            args.push("--name".into());
            args.push(name.into());
        }
        if let Some(db) = &self.db {
            args.push("--db".into());
            args.push(db.into());
        }
        for dir in &self.search_paths {
            args.push("--search-path".into());
            args.push(dir.into());
        }
        args.push("--models".into());
        args.push(match self.models {
            ModelsMode::Include => "include".into(),
            ModelsMode::Skip => "skip".into(),
        });
        match self.mode {
            RunMode::Preview => args.push("--dry-run".into()),
            RunMode::Update => args.push("--update".into()),
            RunMode::Import => {}
        }
        args.push("--progress".into());
        args
    }
}

/// The timing rules of a run; tests shorten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunLimits {
    pub silence: Duration,
    /// How often the run checks for cancel and silence.
    pub poll: Duration,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            silence: SILENCE_LIMIT,
            poll: Duration::from_millis(50),
        }
    }
}

/// The silence rule on its own, so it can be tested against made-up time.
#[derive(Debug, Clone, Copy)]
pub struct SilenceWatch {
    limit: Duration,
    last_heard: Instant,
}

impl SilenceWatch {
    pub fn new(limit: Duration, now: Instant) -> Self {
        Self {
            limit,
            last_heard: now,
        }
    }

    pub fn heard(&mut self, now: Instant) {
        self.last_heard = now;
    }

    pub fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_heard) >= self.limit
    }
}

/// A run that wrote a library (exit 0 or 2), or a preview's report.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    pub exit: ExitKind,
    pub summary: Summary,
    /// The library folder, confined to the output (`None` for a preview).
    pub library: Option<PathBuf>,
    pub report: Option<ImportReport>,
    /// Why the report could not be read, when it could not.
    pub report_problem: Option<String>,
    pub stderr_tail: String,
}

/// A run that did not produce a usable library.
#[derive(Debug, Clone, PartialEq)]
pub struct RunFailure {
    pub error: ImportError,
    /// The report, when the run got far enough to write one.
    pub report: Option<ImportReport>,
    pub stderr_tail: String,
    /// The output folder this run created was removed.
    pub removed_output: bool,
}

impl RunFailure {
    pub fn new(error: ImportError) -> Self {
        Self {
            error,
            report: None,
            stderr_tail: String::new(),
            removed_output: false,
        }
    }
}

/// Run the converter's `library` command. `on_event` sees every progress
/// event as it arrives. A folder this run created is removed when it fails,
/// is cancelled or goes silent; a folder that existed before never is.
pub fn run_library(
    command: &ConverterCommand,
    request: &LibraryRunRequest,
    limits: RunLimits,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<RunOutcome, RunFailure> {
    request.validate().map_err(RunFailure::new)?;
    let created_output = !request.output.exists();
    let mut args = command.leading_args.clone();
    args.extend(request.args());
    let streamed = stream_run(command, &args, limits, cancel, on_event);
    let result = streamed.and_then(|finished| finish(request, finished));
    result.map_err(|mut failure| {
        if created_output && request.output.exists() {
            match std::fs::remove_dir_all(&request.output) {
                Ok(()) => failure.removed_output = true,
                Err(e) => log::warn!(
                    "library import: could not remove {}: {e}",
                    request.output.display()
                ),
            }
        }
        failure
    })
}

/// A run whose process ended on its own.
struct Finished {
    status: ExitStatus,
    summary: Option<Summary>,
    stderr_tail: String,
}

fn spawn(command: &ConverterCommand, args: &[OsString]) -> Result<Child, ImportError> {
    #[allow(
        clippy::disallowed_methods,
        reason = "runs an external library converter, not a browser or viewer"
    )]
    Command::new(&command.program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ImportError::StartFailed {
            program: command.program.clone(),
            detail: e.to_string(),
        })
}

fn stream_run(
    command: &ConverterCommand,
    args: &[OsString],
    limits: RunLimits,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<Finished, RunFailure> {
    let mut child = spawn(command, args).map_err(RunFailure::new)?;
    let stderr = drain_tail(child.stderr.take());
    let lines = read_lines(child.stdout.take());
    let mut fold = StreamFold::default();
    let mut watch = SilenceWatch::new(limits.silence, Instant::now());
    let mut stdout_open = true;
    // A stopped run's stderr is not waited for: a process the converter
    // started may still hold the pipe open.
    let stopped = |child: &mut Child, error: ImportError| {
        stop(child);
        RunFailure::new(error)
    };
    loop {
        if stdout_open {
            match lines.recv_timeout(limits.poll) {
                Ok(line) => {
                    let (was_event, event) = fold.take(line);
                    if was_event {
                        watch.heard(Instant::now());
                    }
                    if let Some(event) = event {
                        on_event(&event);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => stdout_open = false,
            }
        } else {
            std::thread::sleep(limits.poll);
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(stopped(&mut child, ImportError::Cancelled));
        }
        match child.try_wait() {
            Ok(Some(status)) if !stdout_open => {
                return Ok(Finished {
                    status,
                    summary: fold.into_summary(),
                    stderr_tail: tail_text(stderr.join().unwrap_or_default()),
                });
            }
            Ok(_) => {}
            Err(e) => {
                let error = ImportError::WaitFailed {
                    detail: e.to_string(),
                };
                return Err(stopped(&mut child, error));
            }
        }
        if watch.expired(Instant::now()) {
            let error = ImportError::Silent {
                seconds: limits.silence.as_secs(),
            };
            return Err(stopped(&mut child, error));
        }
    }
}

/// Map exit code and summary (§3.3), then confine and read what it names.
fn finish(request: &LibraryRunRequest, finished: Finished) -> Result<RunOutcome, RunFailure> {
    let Finished {
        status,
        summary,
        stderr_tail,
    } = finished;
    let fail = |error: ImportError, report: Option<ImportReport>| RunFailure {
        error,
        report,
        stderr_tail: stderr_tail.clone(),
        removed_output: false,
    };
    let Some(summary) = summary else {
        return Err(fail(
            ImportError::NoSummary {
                status: status.to_string(),
                stderr: stderr_tail.trim().to_string(),
            },
            None,
        ));
    };
    let exit = match (status.code(), summary.exit, summary.ok) {
        (Some(0), ExitKind::Ok, true) => ExitKind::Ok,
        (Some(2), ExitKind::Partial, true) => ExitKind::Partial,
        (_, ExitKind::Failed, _) | (Some(1), _, _) | (_, _, false) => {
            let report = confined_report(&summary, &request.output).and_then(|p| read_report(&p).ok());
            let message = summary
                .error
                .clone()
                .unwrap_or_else(|| status.to_string());
            return Err(fail(ImportError::ConverterFailed { message }, report));
        }
        (code, exit, _) => {
            return Err(fail(
                ImportError::ExitMismatch {
                    code: code.map_or_else(|| status.to_string(), |c| c.to_string()),
                    exit: format!("{exit:?}").to_lowercase(),
                },
                None,
            ));
        }
    };
    let library = match (&summary.library, request.mode) {
        (_, RunMode::Preview) => None,
        (Some(reported), _) => Some(confine_reported(reported, &request.output).map_err(|e| fail(e, None))?),
        (None, _) => {
            return Err(fail(
                ImportError::NoLibraryFile {
                    dir: request.output.clone(),
                },
                None,
            ));
        }
    };
    let (report, report_problem) = match &summary.report {
        None => (None, Some("the summary names no report".to_string())),
        Some(reported) => match confine_reported(reported, &request.output) {
            Err(e) => return Err(fail(e, None)),
            Ok(path) => match read_report(&path) {
                Ok(report) => (Some(report), None),
                Err(why) => (None, Some(why)),
            },
        },
    };
    Ok(RunOutcome {
        exit,
        summary,
        library,
        report,
        report_problem,
        stderr_tail,
    })
}

fn confine_reported(reported: &Path, output: &Path) -> Result<PathBuf, ImportError> {
    confine(reported, output).map_err(|e| match e {
        ConfineError::Outside => ImportError::PathOutside {
            path: reported.to_path_buf(),
            output: output.to_path_buf(),
        },
        ConfineError::Unreadable { joined, error } => ImportError::PathUnreadable {
            path: joined,
            detail: error.to_string(),
        },
    })
}

fn confined_report(summary: &Summary, output: &Path) -> Option<PathBuf> {
    confine(summary.report.as_deref()?, output).ok()
}

/// Run `describe --json` and parse it. A run past `timeout` is stopped.
pub fn run_describe(
    command: &ConverterCommand,
    converter: &str,
    timeout: Duration,
) -> Result<super::describe::ConverterDescription, ImportError> {
    use super::describe::{DescribeError, parse_describe};
    let failed = |detail: String| ImportError::DescribeFailed {
        converter: converter.to_string(),
        detail,
    };
    let mut args = command.leading_args.clone();
    args.extend(["describe".into(), "--json".into()]);
    let mut child = spawn(command, &args)?;
    let stderr = drain_tail(child.stderr.take());
    let lines = read_lines(child.stdout.take());
    let deadline = Instant::now() + timeout;
    let mut text = String::new();
    loop {
        match lines.recv_timeout(Duration::from_millis(20)) {
            Ok(Line::Text(line)) => text.push_str(&line),
            Ok(Line::Oversized(len)) => {
                stop(&mut child);
                return Err(failed(format!("a {len}-byte line")));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if Instant::now() >= deadline {
            stop(&mut child);
            return Err(failed(format!("no answer after {} s", timeout.as_secs())));
        }
    }
    let status = child.wait().map_err(|e| failed(e.to_string()))?;
    if !status.success() {
        let tail = tail_text(stderr.join().unwrap_or_default());
        return Err(failed(format!("{status} {}", tail.trim())));
    }
    parse_describe(&text).map_err(|e| match e {
        DescribeError::Malformed(why) => failed(why),
        DescribeError::UnsupportedContract(contract) => ImportError::UnsupportedContract {
            converter: converter.to_string(),
            contract,
        },
    })
}

/// Kill and reap the converter. It may have exited in the meantime.
fn stop(child: &mut Child) {
    if let Err(e) = child.kill() {
        log::debug!("library import: kill: {e}");
    }
    if let Err(e) = child.wait() {
        log::debug!("library import: reap: {e}");
    }
}

/// Read stdout on a thread, line by line, into a channel that closes at
/// end of stream.
fn read_lines<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Line> {
    let (tx, rx) = mpsc::channel();
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
        loop {
            let n = match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            splitter.push(&chunk[..n], &mut lines);
            for line in lines.drain(..) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        }
        if let Some(last) = splitter.finish() {
            let _receiver_gone = tx.send(last);
        }
    });
    rx
}

/// Read stderr to its end on a thread, keeping the last bytes.
fn drain_tail<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    // ast-grep-ignore: no-raw-spawn
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let Some(mut pipe) = pipe else {
            return kept;
        };
        let mut chunk = [0u8; 8192];
        loop {
            let n = match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            kept.extend_from_slice(&chunk[..n]);
            let excess = kept.len().saturating_sub(STDERR_TAIL);
            kept.drain(..excess);
        }
        kept
    })
}

fn tail_text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}
