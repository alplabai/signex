//! The Export Job file (`.snxout`, #1299; mockup rows 167 and 193b): the
//! outputs one run writes, the variant they show and the folder they go to.
//! A run writes every enabled output, or the one it names. The file is TOML
//! under the token `snxout/1`; keys this build does not know are kept. See
//! `docs/OUTPUT_JOBS.md`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::format::UnknownFields;
use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};

/// The extension of an Export Job file.
pub const OUTPUT_JOB_EXTENSION: &str = "snxout";

/// The Export Job format token.
pub const SNXOUT_FORMAT_V1: &str = "snxout/1";

/// `.snxout` version policy: a newer token is refused, so a job this build
/// cannot represent is never run or rewritten.
pub const SNXOUT_VERSION: FormatVersion = FormatVersion {
    current: SNXOUT_FORMAT_V1,
    readable: &[SNXOUT_FORMAT_V1],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

/// The output folder of a job that names none, beside the job file.
pub const DEFAULT_OUTPUT_FOLDER: &str = "out";

/// The resolution of a PNG output that names none.
pub const DEFAULT_PNG_DPI: u32 = 300;

/// The lowest and highest resolution a PNG output takes.
pub const MIN_PNG_DPI: u32 = 36;
pub const MAX_PNG_DPI: u32 = 1200;

/// The largest job file a run reads.
pub const MAX_OUTPUT_JOB_BYTES: u64 = 1024 * 1024;

/// What one output writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// Every page of the project as one PDF.
    SchematicPdf,
    /// Every page of the project as a PNG file each.
    SchematicPng,
    /// The bill of materials; CSV, HTML or XLSX by the file's extension.
    Bom,
    /// The `.NET` netlist.
    Netlist,
}

impl OutputKind {
    pub const ALL: [Self; 4] = [
        Self::SchematicPdf,
        Self::SchematicPng,
        Self::Bom,
        Self::Netlist,
    ];

    /// The id the file and the answers spell.
    pub fn id(self) -> &'static str {
        match self {
            Self::SchematicPdf => "schematic_pdf",
            Self::SchematicPng => "schematic_png",
            Self::Bom => "bom",
            Self::Netlist => "netlist",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id.trim())
    }

    /// The file an output of this kind writes when it names none.
    pub fn default_file(self, stem: &str) -> String {
        match self {
            Self::SchematicPdf => format!("{stem}.pdf"),
            Self::SchematicPng => format!("{stem}.png"),
            Self::Bom => format!("{stem}-bom.csv"),
            Self::Netlist => format!("{stem}.NET"),
        }
    }
}

/// The file page `page` of a PNG output named `file` writes:
/// `board.png` gives `board-1.png`, `board-2.png`, ...
pub fn png_page_file(file: &str, page: usize) -> String {
    let stem = Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file);
    format!("{stem}-{page}.png")
}

fn enabled_by_default() -> bool {
    true
}

fn default_folder() -> String {
    DEFAULT_OUTPUT_FOLDER.to_string()
}

/// One output of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobOutput {
    /// Unique in the job; a run names one output by it.
    pub name: String,
    /// An [`OutputKind`] id. A kind this build does not know is kept on
    /// save and left out of a run, which says so.
    pub kind: String,
    /// Off: no run writes it.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    /// The file name in the job's folder; the kind's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// A PNG output's resolution; [`DEFAULT_PNG_DPI`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u32>,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl JobOutput {
    /// A new enabled output of `kind` with the kind's default file.
    pub fn new(name: &str, kind: OutputKind) -> Self {
        Self {
            name: name.to_string(),
            kind: kind.id().to_string(),
            enabled: true,
            file: None,
            dpi: None,
            unknown: UnknownFields::default(),
        }
    }
}

/// The content of a `.snxout` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputJob {
    /// Where the files go, relative to the job file's folder.
    #[serde(default = "default_folder")]
    pub folder: String,
    /// The variant every output shows; the project's active variant when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default)]
    pub outputs: Vec<JobOutput>,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl Default for OutputJob {
    fn default() -> Self {
        Self {
            folder: default_folder(),
            variant: None,
            outputs: Vec::new(),
            unknown: UnknownFields::default(),
        }
    }
}

/// Why a job cannot be run or saved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutputJobFault {
    #[error("the folder {0:?} must be a folder beside the job file, with no `..` and no drive")]
    BadFolder(String),
    #[error("an output needs a name")]
    NoName,
    #[error("two outputs are named {0:?}")]
    NameTwice(String),
    #[error("output {name:?}: {file:?} must be a plain file name")]
    BadFile { name: String, file: String },
    #[error("output {name:?}: {dpi} dpi is outside 36 to 1200")]
    BadDpi { name: String, dpi: u32 },
}

/// Why a job file could not be read or written.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OutputJobError {
    #[error(transparent)]
    Version(#[from] FormatVersionError),
    #[error("the Export Job is not valid: {0}")]
    Parse(String),
    #[error(transparent)]
    Fault(#[from] OutputJobFault),
    #[error("{0}")]
    Io(String),
}

/// Why a run of a job writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanProblem {
    #[error(transparent)]
    Fault(#[from] OutputJobFault),
    #[error("the job has no output named {0:?}")]
    NoSuchOutput(String),
    #[error("output {0:?} is switched off")]
    Disabled(String),
    #[error("output {name:?} is of a kind this Signex does not write ({kind:?})")]
    UnknownKind { name: String, kind: String },
    #[error("the job has no enabled output")]
    Nothing,
    #[error("outputs {first:?} and {second:?} write the same file {file:?}")]
    SameFile {
        first: String,
        second: String,
        file: String,
    },
}

/// One output a run writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedOutput {
    pub name: String,
    pub kind: OutputKind,
    /// The file name in the job's folder; a PNG output writes
    /// [`png_page_file`] names from it, one per page.
    pub file: String,
    pub dpi: u32,
}

/// What a run writes, and the outputs it leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobPlan {
    pub outputs: Vec<PlannedOutput>,
    /// Enabled outputs of a kind this build does not know, by name.
    pub skipped: Vec<String>,
}

/// A file or folder name with no separator, drive or dot step.
fn plain_name(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part.trim() == part
        && !part.contains([':', '/', '\\'])
        && !part.chars().any(char::is_control)
}

/// `text` as a folder below the job file's folder: empty is that folder.
fn relative_folder(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return Some(PathBuf::new());
    }
    let mut out = PathBuf::new();
    for part in text.split(['/', '\\']) {
        if !plain_name(part) {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

impl OutputJob {
    /// What keeps the job from running or being saved; `None` for a good one.
    pub fn fault(&self) -> Option<OutputJobFault> {
        if relative_folder(&self.folder).is_none() {
            return Some(OutputJobFault::BadFolder(self.folder.clone()));
        }
        let mut names = BTreeSet::new();
        for output in &self.outputs {
            let name = output.name.trim();
            if name.is_empty() {
                return Some(OutputJobFault::NoName);
            }
            if !names.insert(name.to_string()) {
                return Some(OutputJobFault::NameTwice(name.to_string()));
            }
            if let Some(file) = &output.file
                && !plain_name(file)
            {
                return Some(OutputJobFault::BadFile {
                    name: name.to_string(),
                    file: file.clone(),
                });
            }
            if let Some(dpi) = output.dpi
                && !(MIN_PNG_DPI..=MAX_PNG_DPI).contains(&dpi)
            {
                return Some(OutputJobFault::BadDpi {
                    name: name.to_string(),
                    dpi,
                });
            }
        }
        None
    }

    /// The output folder below `job_folder`, the folder holding the job file.
    pub fn output_folder(&self, job_folder: &Path) -> Result<PathBuf, OutputJobFault> {
        relative_folder(&self.folder)
            .map(|folder| job_folder.join(folder))
            .ok_or_else(|| OutputJobFault::BadFolder(self.folder.clone()))
    }

    /// The outputs a run writes: every enabled one, or only the one named
    /// `only`. `stem` names the default files (the project's file name).
    pub fn plan(&self, stem: &str, only: Option<&str>) -> Result<JobPlan, PlanProblem> {
        if let Some(fault) = self.fault() {
            return Err(fault.into());
        }
        let only = only.map(str::trim).filter(|name| !name.is_empty());
        let mut plan = JobPlan::default();
        let mut files: Vec<(String, String)> = Vec::new();
        for output in &self.outputs {
            let name = output.name.trim();
            if only.is_some_and(|only| only != name) {
                continue;
            }
            if !output.enabled {
                if only.is_some() {
                    return Err(PlanProblem::Disabled(name.to_string()));
                }
                continue;
            }
            let Some(kind) = OutputKind::from_id(&output.kind) else {
                if only.is_some() {
                    return Err(PlanProblem::UnknownKind {
                        name: name.to_string(),
                        kind: output.kind.clone(),
                    });
                }
                plan.skipped.push(name.to_string());
                continue;
            };
            let file = output
                .file
                .clone()
                .unwrap_or_else(|| kind.default_file(stem));
            if !plain_name(&file) {
                return Err(OutputJobFault::BadFile {
                    name: name.to_string(),
                    file,
                }
                .into());
            }
            // Compared without case: one file on a case-insensitive volume.
            let key = file.to_lowercase();
            if let Some((first, _)) = files.iter().find(|(_, other)| *other == key) {
                return Err(PlanProblem::SameFile {
                    first: first.clone(),
                    second: name.to_string(),
                    file,
                });
            }
            files.push((name.to_string(), key));
            plan.outputs.push(PlannedOutput {
                name: name.to_string(),
                kind,
                file,
                dpi: output.dpi.unwrap_or(DEFAULT_PNG_DPI),
            });
        }
        match only {
            Some(only) if plan.outputs.is_empty() => {
                Err(PlanProblem::NoSuchOutput(only.to_string()))
            }
            None if plan.outputs.is_empty() => Err(PlanProblem::Nothing),
            _ => Ok(plan),
        }
    }
}

#[derive(Serialize)]
struct JobFileOut<'a> {
    format: &'static str,
    #[serde(flatten)]
    job: &'a OutputJob,
}

/// The job as its file holds it. Refused for a job that could not be run.
pub fn encode_output_job(job: &OutputJob) -> Result<String, OutputJobError> {
    if let Some(fault) = job.fault() {
        return Err(fault.into());
    }
    let file = JobFileOut {
        format: SNXOUT_FORMAT_V1,
        job,
    };
    toml::to_string(&file).map_err(|e| OutputJobError::Parse(e.to_string()))
}

/// Read a job file's text. Refused for a missing, newer or garbled token and
/// for TOML that is not a job; a job with a fault is read, and its run says
/// why it cannot run.
pub fn parse_output_job(text: &str) -> Result<OutputJob, OutputJobError> {
    let mut table: toml::Table =
        toml::from_str(text).map_err(|e| OutputJobError::Parse(e.to_string()))?;
    let token = match table.remove("format") {
        None => None,
        Some(toml::Value::String(token)) => Some(token),
        Some(_) => {
            return Err(OutputJobError::Parse(
                "`format` is not a format token".to_string(),
            ));
        }
    };
    SNXOUT_VERSION.read(token.as_deref())?;
    toml::Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| OutputJobError::Parse(e.to_string()))
}

/// Read the job file at `path`. Blocking IO: the app calls it from a task.
pub fn read_output_job(path: &Path) -> Result<OutputJob, OutputJobError> {
    let size = std::fs::metadata(path)
        .map_err(|e| OutputJobError::Io(format!("{}: {e}", path.display())))?
        .len();
    if size > MAX_OUTPUT_JOB_BYTES {
        return Err(OutputJobError::Io(format!(
            "{} is {size} bytes, more than the {MAX_OUTPUT_JOB_BYTES} an Export Job reads",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| OutputJobError::Io(format!("{}: {e}", path.display())))?;
    parse_output_job(&text)
}

/// Write the job to `path` atomically. Blocking IO: the app calls it from a
/// task.
pub fn write_output_job(path: &Path, job: &OutputJob) -> Result<(), OutputJobError> {
    let text = encode_output_job(job)?;
    crate::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| OutputJobError::Io(e.to_string()))
}

#[cfg(test)]
#[path = "output_job_tests.rs"]
mod tests;
