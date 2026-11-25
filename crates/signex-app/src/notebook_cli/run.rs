//! One `signex notebook generate` run: a headless session with a throwaway
//! config root and the bundled fonts, the project open, the catalog command
//! run through the headless entry, and the summary printed.

use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};
use signex_types::project::{ErcRules, RuleProfile};

use super::{GenerateCommand, finish_answer, summary_line};
use crate::app::Signex;
use crate::config_root::set_config_root_override;
use crate::fonts::catalog::{FontSet, install_before_boot};
use crate::headless::outcome_verdict;
use crate::keymap::AppCommandId;
use crate::task_driver::TaskDriver;

/// The command a run makes.
const COMMAND: &str = "generate_design_notebook";
/// Exit codes: the report was made and no `--fail-on` condition held; a
/// condition held, or the command refused (a report already there); the
/// command line or the project could not be used.
const EXIT_NOT_OK: u8 = 1;
const EXIT_ERROR: u8 = 2;

/// Why a run stopped before the command answered.
enum Stop {
    /// The command refused, with the reason.
    Refused(String),
    /// The project or the session could not be set up.
    Error(String),
}

/// Run `command`, printing to `out` (the summary) and `err` (what went
/// wrong); the exit code.
pub fn run(command: &GenerateCommand, out: &mut impl Write, err: &mut impl Write) -> u8 {
    let printed = match answer(command) {
        Ok(mut value) => {
            let code = finish_answer(&mut value, &command.fail_on);
            let text = if command.json {
                serde_json::to_string_pretty(&value).unwrap_or_default()
            } else {
                summary_line(&value)
            };
            writeln!(out, "{text}").map(|()| code)
        }
        Err(Stop::Refused(reason)) if command.json => {
            let refusal = json!({
                "command": signex_notebook::generate::GENERATE_COMMAND,
                "status": "refused",
                "written": false,
                "reason": reason,
            });
            let text = serde_json::to_string_pretty(&refusal).unwrap_or_default();
            writeln!(out, "{text}").map(|()| EXIT_NOT_OK)
        }
        Err(Stop::Refused(reason)) => {
            writeln!(err, "signex notebook generate: {reason}").map(|()| EXIT_NOT_OK)
        }
        Err(Stop::Error(reason)) => {
            writeln!(err, "signex notebook generate: {reason}").map(|()| EXIT_ERROR)
        }
    };
    printed.unwrap_or(EXIT_ERROR)
}

/// The summary the command answered with.
fn answer(command: &GenerateCommand) -> Result<Value, Stop> {
    let project = &command.project;
    if !project.is_file() {
        return Err(Stop::Error(format!(
            "{} is not a project file",
            project.display()
        )));
    }
    if let Err(error) = crate::diagnostics::init_logging() {
        eprintln!("[signex] logging was already set up: {error:#}");
    }
    // A throwaway config root: the run never reads or writes the user's.
    let config_root = tempfile::Builder::new()
        .prefix("signex-notebook-")
        .tempdir()
        .map_err(|error| Stop::Error(format!("creating the config root: {error}")))?;
    set_config_root_override(config_root.path().to_path_buf())
        .map_err(|error| Stop::Error(error.to_string()))?;
    install_before_boot(FontSet::BundledOnly)
        .map_err(|_| Stop::Error("fonts were already installed".to_string()))?;
    let rules = RuleProfile::new("Signex headless default", "1", ErcRules::default());
    let (mut app, boot) = Signex::boot_headless(vec![project.clone()], &rules)
        .map_err(|error| Stop::Error(error.to_string()))?;
    let mut driver = TaskDriver::new();
    driver
        .settle(&mut app, boot)
        .map_err(|error| Stop::Error(format!("opening {}: {error}", project.display())))?;
    if app.document_state.projects.is_empty() {
        return Err(Stop::Error(format!("could not open {}", project.display())));
    }
    let answered = run_command(&mut app, &mut driver, command, project);
    let end = app.end_headless_session();
    if let Err(error) = driver.settle(&mut app, end) {
        eprintln!("[signex] ending the session: {error}");
    }
    answered
}

fn run_command(
    app: &mut Signex,
    driver: &mut TaskDriver,
    command: &GenerateCommand,
    project: &Path,
) -> Result<Value, Stop> {
    let id = AppCommandId::new(COMMAND).map_err(Stop::Error)?;
    let mut operands = vec![
        ("document".to_string(), project.display().to_string()),
        ("save".to_string(), "true".to_string()),
    ];
    if command.force {
        operands.push(("update".to_string(), "true".to_string()));
    }
    operands.extend(command.operands.iter().cloned());
    let args = crate::app::command::command_line_args(&id, &operands).map_err(Stop::Error)?;
    let (outcome, task) = app.dispatch_command_headless(&id, args);
    driver
        .settle(app, task)
        .map_err(|error| Stop::Error(error.to_string()))?;
    let (_, reason, data) = outcome_verdict(COMMAND, outcome, None);
    data.ok_or_else(|| Stop::Refused(reason.unwrap_or_else(|| "no answer".to_string())))
}
