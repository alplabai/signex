//! The seam between the cells and the Python runtime: one [`CellRunner`]
//! trait. [`NoRuntime`] is the slim install, which runs nothing; the
//! embedded runtime is another runner behind the same trait. A
//! [`ScriptedRunner`] stands in for it in tests and replays what each cell
//! does, its file and network attempts going through the real
//! [`SandboxPolicy`].

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::inputs::InputValue;
use super::model::PythonCell;
use super::sandbox::{Access, Refusal, SandboxPolicy};

/// Whether this copy of Signex can run cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "runtime", rename_all = "snake_case")]
pub enum RuntimeStatus {
    /// The runtime is installed; `version` names it (`Python 3.12`) and
    /// is part of each cached result.
    Ready { version: String },
    /// The slim install: there is no Python runtime.
    NotInstalled,
}

/// A figure a cell drew: a vector plot, numbered on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Figure {
    /// The plot as SVG.
    pub svg: String,
    /// Its caption; empty for none.
    #[serde(default)]
    pub caption: String,
}

/// Why a cell stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CellError {
    /// The sandbox refused an access on `line` of the cell source.
    Refused { line: u32, refusal: Refusal },
    /// The cell raised `kind` (`ValueError`, `DimensionalityError`), on
    /// `line` of its source when the traceback names one.
    Raised {
        line: Option<u32>,
        kind: String,
        message: String,
    },
    /// The cell ran past its time limit and was stopped.
    TimedOut { seconds: u32 },
    /// Esc stopped it.
    Stopped,
}

/// What one run of a cell gave.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RunOutput {
    /// What it printed.
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub figures: Vec<Figure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CellError>,
    pub duration_ms: u64,
    /// The values it read through the bridge, when the runtime recorded
    /// them; empty: the values found in its source are kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<InputValue>,
}

/// One cell to run, with the values it reads and the rules it runs under.
#[derive(Debug, Clone, Copy)]
pub struct CellRun<'a> {
    pub cell: &'a PythonCell,
    pub inputs: &'a [InputValue],
    pub policy: &'a SandboxPolicy,
}

/// A Python runtime. `run` blocks until the cell ends, so the app calls it
/// from a task, never from `update`.
pub trait CellRunner: Send + Sync {
    fn status(&self) -> RuntimeStatus;
    fn run(&self, run: CellRun<'_>) -> RunOutput;
}

/// The slim install: no runtime, nothing runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRuntime;

impl CellRunner for NoRuntime {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus::NotInstalled
    }

    fn run(&self, _run: CellRun<'_>) -> RunOutput {
        RunOutput::default()
    }
}

/// What a scripted cell does when it runs.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// What it gives when nothing is refused.
    pub output: RunOutput,
    /// What it tries, in order, each on a line of its source.
    pub attempts: Vec<(u32, Access)>,
}

/// A runner that replays a [`Script`] per cell label, for tests and for
/// checking the scheduler without Python. Every attempt goes through the
/// run's [`SandboxPolicy`]; the first refusal ends the cell with nothing
/// printed, as the runtime does.
#[derive(Debug, Default)]
pub struct ScriptedRunner {
    version: String,
    scripts: BTreeMap<String, Script>,
    log: Mutex<Vec<String>>,
}

impl ScriptedRunner {
    /// A runner that reports runtime `version` as ready.
    pub fn new(version: &str) -> Self {
        Self {
            version: version.to_string(),
            ..Self::default()
        }
    }

    /// The runner with `script` for the cell labelled `label`.
    pub fn with(mut self, label: &str, script: Script) -> Self {
        self.scripts.insert(label.to_string(), script);
        self
    }

    /// The labels of the cells run so far, in order.
    pub fn runs(&self) -> Vec<String> {
        self.log.lock().map(|log| log.clone()).unwrap_or_default()
    }
}

impl CellRunner for ScriptedRunner {
    fn status(&self) -> RuntimeStatus {
        RuntimeStatus::Ready {
            version: self.version.clone(),
        }
    }

    fn run(&self, run: CellRun<'_>) -> RunOutput {
        if let Ok(mut log) = self.log.lock() {
            log.push(run.cell.label.clone());
        }
        let script = self
            .scripts
            .get(&run.cell.label)
            .cloned()
            .unwrap_or_default();
        for (line, access) in &script.attempts {
            if let Err(refusal) = run.policy.check(access) {
                return RunOutput {
                    error: Some(CellError::Refused {
                        line: *line,
                        refusal,
                    }),
                    duration_ms: script.output.duration_ms,
                    ..RunOutput::default()
                };
            }
        }
        script.output
    }
}
