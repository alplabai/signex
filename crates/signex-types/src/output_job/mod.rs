//! The Export Job file (`.snxout`, #1299; mockup rows 114, 167 and 193b):
//! the outputs one run writes, the variant they show and the folder they go
//! to. A run writes every enabled output, the one it names, or the ticked
//! outputs of a run profile (`profile.rs`); the runs from the window are
//! kept in its history (`history.rs`). The file is TOML under the token
//! `snxout/1`; keys this build does not know are kept. See
//! `docs/OUTPUT_JOBS.md`.

mod gerber;
mod history;
mod kind;
mod profile;

pub use gerber::{
    GERBER_ALL_COPPER, GERBER_DECIMAL_DIGITS, GERBER_INTEGER_DIGITS, GerberApertures, GerberFormat,
    GerberSettings, GerberUnits, GerberZeros, gerber_layers_text, is_gerber_layer_id,
    parse_gerber_layers,
};
pub use history::{
    MAX_HISTORY, MAX_RECORD_WARNINGS, RunRecord, RunResult, RunTrigger, with_run_record,
};
pub use kind::{OutputGroup, OutputKind};
pub use profile::{
    BeforeClose, DEFAULT_SAVE_WAIT_SECONDS, JobProfile, JobTriggers, MAX_SAVE_WAIT_SECONDS,
    OnErrors, OnWarnings, Packaging, RunPlan, RunRequest, expand_naming, release_folder_name,
};

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
    /// A board output names the stem its files start with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// A PNG output's resolution; [`DEFAULT_PNG_DPI`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u32>,
    /// An ODB++ output's job as its folder tree rather than one archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odb_folder: Option<bool>,
    /// A Gerber output's settings (`gerber.rs`); each one left out is its
    /// default, the writer's own, so a job written before them reads back
    /// and writes the same files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gerber_units: Option<GerberUnits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gerber_format: Option<GerberFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gerber_zeros: Option<GerberZeros>,
    /// The layer ids written; every layer when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gerber_layers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gerber_apertures: Option<GerberApertures>,
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
            odb_folder: None,
            gerber_units: None,
            gerber_format: None,
            gerber_zeros: None,
            gerber_layers: None,
            gerber_apertures: None,
            unknown: UnknownFields::default(),
        }
    }

    /// The output's kind, when this build writes it.
    pub fn known_kind(&self) -> Option<OutputKind> {
        OutputKind::from_id(&self.kind)
    }

    /// The Gerber settings the output is written with, a default for each
    /// one left out.
    pub fn gerber_settings(&self) -> GerberSettings {
        GerberSettings {
            units: self.gerber_units.unwrap_or_default(),
            format: self.gerber_format.unwrap_or_default(),
            zeros: self.gerber_zeros.unwrap_or_default(),
            layers: self.gerber_layers.clone(),
            apertures: self.gerber_apertures.unwrap_or_default(),
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
    /// The profile a run that names none uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub outputs: Vec<JobOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<JobProfile>,
    /// The runs from the window, newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<RunRecord>,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl Default for OutputJob {
    fn default() -> Self {
        Self {
            folder: default_folder(),
            variant: None,
            default_profile: None,
            outputs: Vec::new(),
            profiles: Vec::new(),
            history: Vec::new(),
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
    #[error("output {name:?}: {layers:?} is not a list of Gerber layer ids")]
    BadGerberLayers { name: String, layers: String },
    #[error("a profile needs a name")]
    NoProfileName,
    #[error("two profiles are named {0:?}")]
    ProfileTwice(String),
    #[error("the default profile {0:?} is not a profile of the job")]
    NoDefault(String),
    #[error("profile {profile:?}: a wait of {seconds} s is longer than an hour")]
    BadWait { profile: String, seconds: u32 },
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
    #[error("the job has no profile named {0:?}")]
    NoSuchProfile(String),
    #[error("profile {0:?} ticks no enabled output")]
    NothingTicked(String),
}

/// One output a run writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedOutput {
    pub name: String,
    pub kind: OutputKind,
    /// The file name in the job's folder; a PNG output writes
    /// [`png_page_file`] names from it, one per page, and a board output's
    /// files start with it.
    pub file: String,
    pub dpi: u32,
    /// An ODB++ output's job as its folder tree.
    #[serde(skip)]
    pub odb_folder: bool,
    /// A Gerber output's settings, the writer's own by default.
    #[serde(skip)]
    pub gerber: GerberSettings,
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

/// `text` as a folder below the job file's folder, if it is one: the check a
/// profile's folder and a run's release folder pass.
pub fn folder_below(text: &str) -> Option<PathBuf> {
    relative_folder(text)
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
            if let Some(layers) = &output.gerber_layers
                && (layers.is_empty() || !layers.iter().all(|id| is_gerber_layer_id(id)))
            {
                return Some(OutputJobFault::BadGerberLayers {
                    name: name.to_string(),
                    layers: layers.join(","),
                });
            }
        }
        self.profile_fault()
    }

    /// The output named `name`.
    pub fn output(&self, name: &str) -> Option<&JobOutput> {
        self.outputs.iter().find(|o| o.name.trim() == name.trim())
    }

    pub fn output_mut(&mut self, name: &str) -> Option<&mut JobOutput> {
        self.outputs
            .iter_mut()
            .find(|o| o.name.trim() == name.trim())
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
        self.plan_among(stem, only, None)
    }

    /// [`Self::plan`] over the outputs `among` names (every output when
    /// `None`); an output it does not name is left out quietly.
    pub fn plan_among(
        &self,
        stem: &str,
        only: Option<&str>,
        among: Option<&[String]>,
    ) -> Result<JobPlan, PlanProblem> {
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
            if only.is_none() && among.is_some_and(|among| !among.iter().any(|n| n.trim() == name))
            {
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
            // A board output's files start with its stem and its kind's
            // own suffixes, so only two of one kind can meet.
            let key = match kind.file_is_stem() {
                true => format!("{}:{}", kind.id(), file.to_lowercase()),
                false => file.to_lowercase(),
            };
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
                odb_folder: output.odb_folder.unwrap_or(false),
                gerber: output.gerber_settings(),
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

/// Refuse a job file at `path` of `size` bytes, larger than an Export Job
/// reads. The host reads the file (`app::output_job::file`); this crate does
/// no IO (#2431).
pub fn check_output_job_size(path: &Path, size: u64) -> Result<(), OutputJobError> {
    if size > MAX_OUTPUT_JOB_BYTES {
        return Err(OutputJobError::Io(format!(
            "{} is {size} bytes, more than the {MAX_OUTPUT_JOB_BYTES} an Export Job reads",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod tests;
