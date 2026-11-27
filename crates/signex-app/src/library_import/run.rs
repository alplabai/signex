//! One `library` run of a converter (contract §2.2, §3): spawn it, read its
//! progress as it comes, stop it when it goes silent or is cancelled, and
//! map its summary and exit code. Blocking; call from `spawn_blocking`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::confine::{ConfineError, confine};
use super::error::ImportError;
use super::events::{ExitKind, ProgressEvent, StreamFold, Summary};
use super::process::{Polled, ProcessGroup, Running};
use super::report::{ImportReport, read_report};
use super::stream::Line;

/// How long a run may go without an event before it is stopped (§3.1).
pub const SILENCE_LIMIT: Duration = Duration::from_secs(120);

/// How long `describe` may take.
pub const DESCRIBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Bound on everything a `library` run may print on stdout.
const RUN_STDOUT_CAP: usize = 256 << 20;

/// Bound on what `describe` may print.
const DESCRIBE_STDOUT_CAP: usize = 1 << 20;

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
    /// Boxed so the failure stays small: a `Result` carrying it is returned
    /// along every step of a run.
    pub error: Box<ImportError>,
    /// The report, when the run got far enough to write one.
    pub report: Option<Box<ImportReport>>,
    pub stderr_tail: String,
    /// The output folder this run created was removed.
    pub removed_output: bool,
    /// For an update over a clean tree: whether the tree was put back to
    /// its last import.
    pub restored: Option<Result<(), String>>,
    /// Refused before the converter started, so nothing on disk changed.
    pub refused: bool,
}

impl RunFailure {
    pub fn new(error: ImportError) -> Self {
        Self {
            error: Box::new(error),
            report: None,
            stderr_tail: String::new(),
            removed_output: false,
            restored: None,
            refused: false,
        }
    }

    /// A refusal before the converter started: nothing was written.
    pub fn refused(error: ImportError) -> Self {
        Self {
            refused: true,
            ..Self::new(error)
        }
    }
}

/// Run the converter's `library` command. `on_event` sees every progress
/// event as it arrives. A new import or a dry run creates `--output` itself
/// ([`claim_output`]) and removes it when it fails, is cancelled or goes
/// silent; an update's folder is never removed.
pub fn run_library(
    command: &ConverterCommand,
    request: &LibraryRunRequest,
    limits: RunLimits,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<RunOutcome, RunFailure> {
    let group = ProcessGroup::default();
    run_library_in(command, request, limits, cancel, &group, on_event)
}

/// [`run_library`], with the converter's process group published in
/// `group` while it runs.
pub fn run_library_in(
    command: &ConverterCommand,
    request: &LibraryRunRequest,
    limits: RunLimits,
    cancel: &AtomicBool,
    group: &ProcessGroup,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<RunOutcome, RunFailure> {
    request.validate().map_err(RunFailure::refused)?;
    let owned = claim_output(request).map_err(RunFailure::refused)?;
    let mut args = command.leading_args.clone();
    args.extend(request.args());
    let cwd = neutral_dir(Some(&request.output));
    let streamed = stream_run(command, &args, &cwd, limits, cancel, group, on_event);
    let result = streamed.and_then(|finished| finish(request, finished));
    result.map_err(|mut failure| {
        if owned {
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

/// Take `--output` for a new import or a dry run: this run creates the
/// folder (and its parents), so a failed run removes only a folder it made.
/// An empty folder already there is replaced by one this run creates; one
/// that holds anything, a symbolic link, or anything else that is not a
/// folder, is refused before the converter starts. `false` for an update,
/// whose folder is never this run's to remove.
pub fn claim_output(request: &LibraryRunRequest) -> Result<bool, ImportError> {
    if request.mode == RunMode::Update {
        return Ok(false);
    }
    let out = &request.output;
    let unusable = |e: std::io::Error| ImportError::OutputUnusable {
        path: out.clone(),
        detail: e.to_string(),
    };
    let taken = || ImportError::OutputNotEmpty { path: out.clone() };
    match std::fs::symlink_metadata(out) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(unusable)?;
            }
        }
        Err(e) => return Err(unusable(e)),
        // remove_dir refuses a folder that is not empty, so nothing put in
        // it meanwhile is lost.
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(ImportError::OutputIsLink { path: out.clone() });
        }
        Ok(meta) if meta.is_dir() => std::fs::remove_dir(out).map_err(|e| {
            if e.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                taken()
            } else {
                unusable(e)
            }
        })?,
        Ok(_) => return Err(taken()),
    }
    // create_dir fails when anything appeared there since.
    std::fs::create_dir(out).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            taken()
        } else {
            unusable(e)
        }
    })?;
    Ok(true)
}

/// A run whose process ended on its own.
struct Finished {
    status: ExitStatus,
    summary: Option<Summary>,
    stderr_tail: String,
}

/// The working directory a converter runs in: the output's parent when it
/// exists, else the system temp folder. Never the app's own cwd.
pub fn neutral_dir(output: Option<&Path>) -> PathBuf {
    output
        .and_then(Path::parent)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .map_or_else(std::env::temp_dir, Path::to_path_buf)
}

fn stream_run(
    command: &ConverterCommand,
    args: &[OsString],
    cwd: &Path,
    limits: RunLimits,
    cancel: &AtomicBool,
    group: &ProcessGroup,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<Finished, RunFailure> {
    let mut running =
        Running::spawn(command, args, cwd, RUN_STDOUT_CAP).map_err(RunFailure::new)?;
    // Cleared by `Running` itself as the converter is reaped, before the
    // pipe drain that may follow for up to `PIPE_GRACE`.
    running.publish(group);
    let result = stream_loop(&mut running, limits, cancel, on_event);
    group.set(0);
    result
}

fn stream_loop(
    running: &mut Running,
    limits: RunLimits,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(&ProgressEvent),
) -> Result<Finished, RunFailure> {
    let mut fold = StreamFold::default();
    let mut watch = SilenceWatch::new(limits.silence, Instant::now());
    let mut take = |line: Line, watch: &mut SilenceWatch| {
        let (was_event, event) = fold.take(line);
        if was_event {
            watch.heard(Instant::now());
        }
        if let Some(event) = event {
            on_event(&event);
        }
    };
    // A stopped run's stderr is not waited for.
    let stopped = |running: &mut Running, error: ImportError| {
        running.stop();
        RunFailure::new(error)
    };
    let status = loop {
        if let Polled::Line(line) = running.poll_line(limits.poll) {
            take(line, &mut watch);
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(stopped(running, ImportError::Cancelled));
        }
        match running.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let error = ImportError::WaitFailed {
                    detail: e.to_string(),
                };
                return Err(stopped(running, error));
            }
        }
        if watch.expired(Instant::now()) {
            let error = ImportError::Silent {
                seconds: limits.silence.as_secs(),
            };
            return Err(stopped(running, error));
        }
    };
    // Exited: what is still in the pipes is read for a bounded time; a
    // process it left holding them cannot hold the run.
    running.drain_after_exit(&mut |line| take(line, &mut watch));
    Ok(Finished {
        status,
        summary: fold.into_summary(),
        stderr_tail: running.stderr_tail(),
    })
}

/// Map exit code and summary (§3.3), then confine and read what it names.
fn finish(request: &LibraryRunRequest, finished: Finished) -> Result<RunOutcome, RunFailure> {
    let Finished {
        status,
        summary,
        stderr_tail,
    } = finished;
    let fail = |error: ImportError, report: Option<ImportReport>| RunFailure {
        error: Box::new(error),
        report: report.map(Box::new),
        stderr_tail: stderr_tail.clone(),
        removed_output: false,
        restored: None,
        refused: false,
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
            let report =
                confined_report(&summary, &request.output).and_then(|p| read_report(&p).ok());
            let message = summary.error.clone().unwrap_or_else(|| status.to_string());
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
        (Some(reported), _) => {
            let library = confine_reported(reported, &request.output).map_err(|e| fail(e, None))?;
            if !same_folder(&library, &request.output) {
                return Err(fail(
                    ImportError::LibraryNotOutput {
                        library,
                        output: request.output.clone(),
                    },
                    None,
                ));
            }
            Some(library)
        }
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

/// The library is `--output` itself, so the folder checked before an
/// update is the one committed after it.
fn same_folder(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
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
    let mut running = Running::spawn(command, &args, &neutral_dir(None), DESCRIBE_STDOUT_CAP)?;
    let deadline = Instant::now() + timeout;
    let mut text = String::new();
    let status = loop {
        match running.poll_line(Duration::from_millis(20)) {
            Polled::Line(Line::Text(line)) => text.push_str(&line),
            Polled::Line(Line::Oversized(len)) => {
                running.stop();
                return Err(failed(format!("a {len}-byte line")));
            }
            Polled::Nothing | Polled::Closed => {}
        }
        if !running.stdout_open() {
            match running.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(e) => {
                    running.stop();
                    return Err(failed(e.to_string()));
                }
            }
        }
        if Instant::now() >= deadline {
            running.stop();
            return Err(failed(format!("no answer after {} s", timeout.as_secs())));
        }
    };
    running.drain_after_exit(&mut |_| {});
    // Nothing `describe` started may outlive it.
    running.kill_group();
    if !status.success() {
        return Err(failed(format!("{status} {}", running.stderr_tail().trim())));
    }
    parse_describe(&text).map_err(|e| match e {
        DescribeError::Malformed(why) => failed(why),
        DescribeError::UnsupportedContract(contract) => ImportError::UnsupportedContract {
            converter: converter.to_string(),
            contract,
        },
    })
}
