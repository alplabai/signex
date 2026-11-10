//! The `run` subcommand (#1923): read commands, print one result line each.
//! The commands run in a Signex that has the project open (#1924), or in a
//! headless session booted here (#1923) that holds the project for the run.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use signex_app::config_root::set_config_root_override;
use signex_app::diagnostics;
use signex_app::fonts::catalog::{FontSet, install_before_boot};

use crate::attach::Attached;
use crate::capability::Capability;
use crate::cli::RunOptions;
use crate::confine::canonical;
use crate::input::{Line, read_line};
use crate::policy::Policy;
use crate::protocol::{Request, ResultLine, parse_request, request_from_args};
use crate::route::{Placement, place};
use crate::session::Session;

/// `--dry-run`: `request` with `dry_run=true`, or why it cannot be one: only a
/// command with a `dry_run` operand can promise to change nothing. A read-only
/// query and `open_document` (it opens a sheet, changes none) run as given.
fn with_dry_run(mut request: Request, dry_run: bool) -> Result<Request, String> {
    if !dry_run {
        return Ok(request);
    }
    let harmless = request.id == "open_document"
        || signex_app::keymap::AppCommandId::new(request.id.as_str())
            .is_ok_and(|id| signex_app::headless::is_query(&id));
    if harmless {
        return Ok(request);
    }
    let takes_it = signex_app::keymap::metadata_for_str(&request.id)
        .is_some_and(|meta| meta.operands.iter().any(|op| op.name == "dry_run"));
    if !takes_it {
        return Err(format!(
            "--dry-run needs a command with a `dry_run` operand; `{}` has none, so it would change the document",
            request.id
        ));
    }
    request.operands.retain(|(name, _)| name != "dry_run");
    request.operands.push(("dry_run".into(), "true".into()));
    Ok(request)
}

/// What a finished run did.
#[derive(Debug)]
pub struct RunReport {
    /// Command lines read.
    pub commands: u64,
    /// Lines whose status was not ok.
    pub not_ok: u64,
}

/// Where the commands of a run go.
trait Runner {
    fn execute(&mut self, seq: u64, request: &Request, raw: &str) -> ResultLine;
    fn reject(&self, seq: u64, raw: &str, reason: String) -> ResultLine;
}

impl Runner for Session {
    fn execute(&mut self, seq: u64, request: &Request, raw: &str) -> ResultLine {
        Session::execute(self, seq, request, raw)
    }
    fn reject(&self, seq: u64, raw: &str, reason: String) -> ResultLine {
        Session::reject(self, seq, raw, reason)
    }
}

impl Runner for Attached {
    fn execute(&mut self, seq: u64, request: &Request, _raw: &str) -> ResultLine {
        Attached::execute(self, seq, request)
    }
    fn reject(&self, seq: u64, _raw: &str, reason: String) -> ResultLine {
        Attached::reject(self, seq, reason)
    }
}

/// Run `options` once per process. A headless run sets up what is
/// process-wide (the config root override, the font set, the logger) here.
/// Result lines go to `out`.
pub fn run(
    options: RunOptions,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<RunReport, String> {
    let project = canonical(&options.project)
        .map_err(|error| format!("{}: {error}", options.project.display()))?;
    let size = std::fs::metadata(&project)
        .map_err(|error| format!("{}: {error}", project.display()))?
        .len();
    if !project.is_file() || size > options.limits.max_file_bytes {
        return Err(format!(
            "{} is not a project file within --max-file-bytes",
            project.display()
        ));
    }
    let allow_paths = options
        .allow_paths
        .iter()
        .map(|dir| {
            canonical(dir)
                .ok()
                .filter(|resolved| resolved.is_dir())
                .ok_or_else(|| format!("--allow-path {} is not a folder", dir.display()))
        })
        .collect::<Result<Vec<PathBuf>, _>>()?;
    let policy = Policy {
        capabilities: options.capabilities.clone(),
        allow_paths,
        limits: options.limits,
    };
    let run_id = uuid::Uuid::now_v7().to_string();

    // The hold lives to the end of the run, however it ends.
    match place(options.route, &project)? {
        Placement::Attach(peer) => {
            let connection = peer.connect()?;
            let mut attached = Attached::new(connection, policy, run_id, &project)?;
            drive(&mut attached, &options, input, out)
        }
        Placement::Headless(_hold) => run_headless(project, policy, run_id, &options, input, out),
    }
}

fn run_headless(
    project: PathBuf,
    policy: Policy,
    run_id: String,
    options: &RunOptions,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<RunReport, String> {
    if let Err(error) = diagnostics::init_logging() {
        eprintln!("[signex-cli] logging was already set up: {error:#}");
    }
    // A throwaway config root: the run never reads or writes the user's.
    let config_root = tempfile::Builder::new()
        .prefix("signex-cli-")
        .tempdir()
        .map_err(|error| format!("creating the config root: {error}"))?;
    set_config_root_override(config_root.path().to_path_buf())
        .map_err(|error| error.to_string())?;
    install_before_boot(FontSet::BundledOnly).map_err(|_| "fonts were already installed")?;

    let mut session = Session::start(project, policy, run_id.clone())?;
    let report = drive(&mut session, options, input, out)?;

    let unsaved = session.finish()?;
    let saves = options.capabilities.contains(&Capability::Save);
    let note = if unsaved.is_empty() {
        "no unsaved changes".to_string()
    } else {
        let names: Vec<_> = unsaved.iter().map(|p| p.display().to_string()).collect();
        let why = if saves {
            "no save command ran for them"
        } else {
            "--save was not given"
        };
        format!("changes NOT written ({why}): {}", names.join(", "))
    };
    diagnostics::log_info(format!(
        "[run {run_id}] done: {} command(s), {} not ok; {note}",
        report.commands, report.not_ok
    ));
    Ok(report)
}

/// Read the commands (one, or a line each from `input`) and print one result
/// line for each, stopping at the first that is not ok unless `--keep-going`.
fn drive(
    runner: &mut impl Runner,
    options: &RunOptions,
    input: &mut impl BufRead,
    out: &mut impl Write,
) -> Result<RunReport, String> {
    let mut report = RunReport {
        commands: 0,
        not_ok: 0,
    };
    let mut emit_line = |line: &ResultLine| -> Result<(), String> {
        report.commands += 1;
        report.not_ok += u64::from(!line.status.is_ok());
        serde_json::to_writer(&mut *out, line)
            .map_err(|error| error.to_string())
            .and_then(|()| out.write_all(b"\n").map_err(|error| error.to_string()))
            .and_then(|()| out.flush().map_err(|error| error.to_string()))
            .map_err(|error| format!("writing a result line: {error}"))
    };

    if let Some((id, args)) = &options.single {
        let line = match request_from_args(id, args).and_then(|r| with_dry_run(r, options.dry_run))
        {
            Ok(request) => runner.execute(1, &request, id),
            Err(reason) => runner.reject(1, id, reason),
        };
        emit_line(&line)?;
    } else {
        let mut seq = 0u64;
        while let Some(read) = read_line(input, options.limits.max_line_bytes)
            .map_err(|error| format!("reading commands: {error}"))?
        {
            if matches!(&read, Line::Text(text) if text.trim().is_empty()) {
                continue;
            }
            seq += 1;
            let over_limit = seq > options.limits.max_commands;
            let text = match read {
                Line::Text(text) => Ok(text),
                Line::TooLong => Err(format!(
                    "the line is longer than {} bytes (--max-line-bytes)",
                    options.limits.max_line_bytes
                )),
                Line::NotUtf8 => Err("the line is not valid UTF-8".to_string()),
            };
            let raw = text.as_deref().unwrap_or_default().to_string();
            let line = if over_limit {
                let reason = format!(
                    "more than {} commands in one run (--max-commands)",
                    options.limits.max_commands
                );
                runner.reject(seq, &raw, reason)
            } else {
                match text
                    .and_then(|text| parse_request(&text))
                    .and_then(|request| with_dry_run(request, options.dry_run))
                {
                    Ok(request) => runner.execute(seq, &request, &raw),
                    Err(reason) => runner.reject(seq, &raw, reason),
                }
            };
            emit_line(&line)?;
            if over_limit || (!line.status.is_ok() && !options.keep_going) {
                break;
            }
        }
    }
    Ok(report)
}
