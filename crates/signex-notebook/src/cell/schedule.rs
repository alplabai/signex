//! When cells run, and the state each one shows (mockup row 200). Cells run
//! only on Run (one cell), Run all and PDF export, never as the text is
//! typed; a cell whose source or read values changed since its last run is
//! marked out of date until it runs again.

use serde::Serialize;

use super::cache::{CachedCell, CellCache, source_hash};
use super::inputs::{DesignValues, Field, InputValue, ValueRead, current_inputs, value_reads};
use super::model::PythonCell;
use super::runner::{CellError, CellRun, CellRunner, RuntimeStatus};
use super::sandbox::{Refusal, SandboxPolicy};
use super::trust::Trust;

/// What asked the cells to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Run cell: the cell labelled so runs, cached or not.
    Cell(String),
    /// Run all: every cell with no result or an out-of-date one runs.
    All,
    /// A PDF export: as Run all, so the pages print current results.
    Export,
}

/// One value a cell read that is not what it was when the cell ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputChange {
    pub designator: String,
    pub field: Field,
    /// The value as the cell read it, as printed.
    pub was: String,
    /// The value now, as printed.
    pub now: String,
}

/// What a cell shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CellState {
    /// It has no result yet.
    NeverRun,
    /// It ran just now.
    Ran { duration_ms: u64, ran_at: String },
    /// Its result is current: nothing it depends on changed since `ran_at`.
    Cached { ran_at: String },
    /// Its result is from before a change; it stays shown, greyed.
    OutOfDate {
        source_changed: bool,
        runtime_changed: bool,
        changes: Vec<InputChange>,
    },
    /// The sandbox refused something on file line `line`.
    Blocked { line: u32, refusal: Refusal },
    /// It raised an error, on file line `line` when known, or was stopped.
    Failed {
        line: Option<u32>,
        kind: String,
        message: String,
    },
    /// The slim install: nothing runs; a cached result from `cached_at`
    /// stays shown, greyed.
    NoRuntime { cached_at: Option<String> },
    /// A notebook from outside the project: it waits for the person's
    /// answer before any cell runs.
    AwaitingTrust,
    /// The person did not allow this notebook's cells to run.
    TrustRefused,
}

/// A cell and its state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CellStatus {
    pub label: String,
    pub file: String,
    pub line: u32,
    #[serde(flatten)]
    pub state: CellState,
}

/// The counts the status bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct CellSummary {
    pub cells: usize,
    pub ran: usize,
    pub cached: usize,
    pub out_of_date: usize,
    pub blocked: usize,
    pub failed: usize,
    pub not_run: usize,
}

impl CellSummary {
    pub fn of(statuses: &[CellStatus]) -> Self {
        let mut summary = CellSummary {
            cells: statuses.len(),
            ..CellSummary::default()
        };
        for status in statuses {
            let count = match status.state {
                CellState::Ran { .. } => &mut summary.ran,
                CellState::Cached { .. } => &mut summary.cached,
                CellState::OutOfDate { .. } => &mut summary.out_of_date,
                CellState::Blocked { .. } => &mut summary.blocked,
                CellState::Failed { .. } => &mut summary.failed,
                CellState::NeverRun
                | CellState::NoRuntime { .. }
                | CellState::AwaitingTrust
                | CellState::TrustRefused => &mut summary.not_run,
            };
            *count += 1;
        }
        summary
    }
}

/// What a run did: the JSON result of the run commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunReport {
    pub trigger: Trigger,
    #[serde(flatten)]
    pub runtime: RuntimeStatus,
    /// True when nothing ran because the notebook is from outside the
    /// project and the person has not answered.
    pub needs_trust: bool,
    /// The label a Run cell named when no cell has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown_cell: Option<String>,
    pub cells: Vec<CellStatus>,
    pub summary: CellSummary,
}

/// What a run works with.
pub struct RunContext<'a> {
    pub cells: &'a [PythonCell],
    pub design: &'a dyn DesignValues,
    pub runner: &'a dyn CellRunner,
    pub policy: &'a SandboxPolicy,
    pub trust: Trust,
    /// The time the run is stamped with, from the caller's clock.
    pub now: &'a str,
}

/// The state of every cell without running any: what the page shows when
/// the notebook opens or its text changes.
pub fn cell_states(
    cells: &[PythonCell],
    cache: &CellCache,
    design: &dyn DesignValues,
    runtime: &RuntimeStatus,
) -> Vec<CellStatus> {
    cells
        .iter()
        .map(|cell| status(cell, state_of(cell, cache, design, runtime)))
        .collect()
}

/// The state cell `cell` shows now, from its cached result.
pub fn state_of(
    cell: &PythonCell,
    cache: &CellCache,
    design: &dyn DesignValues,
    runtime: &RuntimeStatus,
) -> CellState {
    let cached = cache.get(&cell.label);
    let version = match runtime {
        RuntimeStatus::Ready { version } => version,
        RuntimeStatus::NotInstalled => {
            let cached_at = cached.map(|cached| cached.ran_at.clone());
            return CellState::NoRuntime { cached_at };
        }
    };
    let Some(cached) = cached else {
        return CellState::NeverRun;
    };
    let source_changed = cached.source_hash != source_hash(&cell.source);
    let runtime_changed = cached.runtime != *version;
    let changes = input_changes(&cached.inputs, design);
    if source_changed || runtime_changed || !changes.is_empty() {
        return CellState::OutOfDate {
            source_changed,
            runtime_changed,
            changes,
        };
    }
    result_state(cell, cached)
}

/// Run the cells `trigger` asks for, keep their results in `cache` and
/// report the state of every cell.
pub fn run_cells(context: &RunContext<'_>, cache: &mut CellCache, trigger: Trigger) -> RunReport {
    let runtime = context.runner.status();
    let unknown_cell = match &trigger {
        Trigger::Cell(label) if !context.cells.iter().any(|cell| cell.label == *label) => {
            Some(label.clone())
        }
        _ => None,
    };
    let ready = matches!(runtime, RuntimeStatus::Ready { .. });
    let mut needs_trust = false;
    let mut statuses = Vec::with_capacity(context.cells.len());
    for cell in context.cells {
        let current = state_of(cell, cache, context.design, &runtime);
        let wanted = match &trigger {
            Trigger::Cell(label) => cell.label == *label,
            Trigger::All | Trigger::Export => {
                matches!(current, CellState::NeverRun | CellState::OutOfDate { .. })
            }
        };
        let state = match (wanted && ready, context.trust) {
            (false, _) => current,
            (true, Trust::Unasked) => {
                needs_trust = true;
                CellState::AwaitingTrust
            }
            (true, Trust::Refused) => CellState::TrustRefused,
            (true, Trust::Trusted) => run_one(context, cache, cell, &runtime),
        };
        statuses.push(status(cell, state));
    }
    let keep = context.cells.iter().map(|cell| cell.label.as_str());
    cache.retain_labels(keep);
    let summary = CellSummary::of(&statuses);
    RunReport {
        trigger,
        runtime,
        needs_trust,
        unknown_cell,
        cells: statuses,
        summary,
    }
}

fn run_one(
    context: &RunContext<'_>,
    cache: &mut CellCache,
    cell: &PythonCell,
    runtime: &RuntimeStatus,
) -> CellState {
    let reads: Vec<ValueRead> = value_reads(&cell.source);
    let inputs = current_inputs(&reads, context.design);
    let output = context.runner.run(CellRun {
        cell,
        inputs: &inputs,
        policy: context.policy,
    });
    let duration_ms = output.duration_ms;
    // A stopped run has no result to keep; the last one stays.
    if let Some(CellError::Stopped | CellError::TimedOut { .. }) = &output.error {
        return error_state(cell, output.error.as_ref());
    }
    let version = match runtime {
        RuntimeStatus::Ready { version } => version.clone(),
        RuntimeStatus::NotInstalled => String::new(),
    };
    let inputs = if output.reads.is_empty() {
        inputs
    } else {
        output.reads.clone()
    };
    let cached = CachedCell {
        source_hash: source_hash(&cell.source),
        runtime: version,
        inputs,
        output,
        ran_at: context.now.to_string(),
    };
    let state = match &cached.output.error {
        None => CellState::Ran {
            duration_ms,
            ran_at: context.now.to_string(),
        },
        Some(_) => result_state(cell, &cached),
    };
    cache.insert(&cell.label, cached);
    state
}

/// The state a current result gives: cached, or the error it ended with.
fn result_state(cell: &PythonCell, cached: &CachedCell) -> CellState {
    match &cached.output.error {
        None => CellState::Cached {
            ran_at: cached.ran_at.clone(),
        },
        error => error_state(cell, error.as_ref()),
    }
}

fn error_state(cell: &PythonCell, error: Option<&CellError>) -> CellState {
    let failed = |kind: &str, message: String| CellState::Failed {
        line: None,
        kind: kind.to_string(),
        message,
    };
    match error {
        None => CellState::NeverRun,
        Some(CellError::Refused { line, refusal }) => CellState::Blocked {
            line: cell.file_line(*line),
            refusal: refusal.clone(),
        },
        Some(CellError::Raised {
            line,
            kind,
            message,
        }) => CellState::Failed {
            line: line.map(|line| cell.file_line(line)),
            kind: kind.clone(),
            message: message.clone(),
        },
        Some(CellError::TimedOut { seconds }) => failed(
            "TimeoutError",
            format!("the cell ran longer than {seconds} s and was stopped"),
        ),
        Some(CellError::Stopped) => failed("KeyboardInterrupt", "the cell was stopped".into()),
    }
}

/// The values of `read` that are not what they are in `design` now.
fn input_changes(read: &[InputValue], design: &dyn DesignValues) -> Vec<InputChange> {
    let reads: Vec<ValueRead> = read
        .iter()
        .map(|input| ValueRead {
            designator: input.designator.clone(),
            field: input.field.clone(),
            symbol: input.symbol.clone(),
            cell_line: 0,
        })
        .collect();
    let now = current_inputs(&reads, design);
    read.iter()
        .zip(now)
        .filter(|(was, now)| was.written != now.written)
        .map(|(was, now)| InputChange {
            designator: was.designator.clone(),
            field: was.field.clone(),
            was: was.display(),
            now: now.display(),
        })
        .collect()
}

fn status(cell: &PythonCell, state: CellState) -> CellStatus {
    CellStatus {
        label: cell.label.clone(),
        file: cell.file.clone(),
        line: cell.line,
        state,
    }
}
