//! Run profiles of an Export Job (mockup row 114, "Profiles"): a named set of
//! ticked outputs with the one variant it builds, where the files go, the
//! run's policy and the triggers that start it without a click. Profiles are
//! stored in the `.snxout`, so everyone who opens the project gets them.
//! Rules: `docs/OUTPUT_JOBS.md`, "Profiles".

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{JobPlan, OutputJob, PlanProblem};
use crate::format::UnknownFields;

/// The wait after the last save before a run on save starts, in seconds.
pub const DEFAULT_SAVE_WAIT_SECONDS: u32 = 30;

/// The longest wait a run on save takes, in seconds.
pub const MAX_SAVE_WAIT_SECONDS: u32 = 3600;

/// What a run does when Validate finds an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnErrors {
    /// Nothing is generated or written.
    #[default]
    Stop,
    /// The person is asked; a run with no person stops.
    Ask,
}

/// What a run does when Validate finds a warning and no error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnWarnings {
    #[default]
    Continue,
    /// The person is asked; a run with no person stops.
    Ask,
    Stop,
}

/// Whether a profile runs when its project closes with out-of-date outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeforeClose {
    #[default]
    Off,
    /// A notice names the out-of-date outputs; the close goes on.
    Ask,
    /// The run starts before the project closes.
    Always,
}

fn default_wait() -> u32 {
    DEFAULT_SAVE_WAIT_SECONDS
}

fn yes() -> bool {
    true
}

/// What starts a profile without a click (mockup row 114, "Automation").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobTriggers {
    /// Run when one of `sources` is saved.
    #[serde(default)]
    pub on_save: bool,
    /// File names or `*.ext` patterns a save is matched against; empty is
    /// the project's board and every schematic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// Seconds after the last save, so a burst of saves runs once.
    #[serde(default = "default_wait")]
    pub wait_seconds: u32,
    /// On a save, run only the ticked outputs made from the saved file.
    #[serde(default)]
    pub keep_up_to_date: bool,
    #[serde(default)]
    pub before_close: BeforeClose,
    /// The tag pattern of a run on release or git tag. Kept and shown; no
    /// release or tag event exists yet to start it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_tag: Option<String>,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl Default for JobTriggers {
    fn default() -> Self {
        Self {
            on_save: false,
            sources: Vec::new(),
            wait_seconds: DEFAULT_SAVE_WAIT_SECONDS,
            keep_up_to_date: false,
            before_close: BeforeClose::Off,
            on_tag: None,
            unknown: UnknownFields::default(),
        }
    }
}

impl JobTriggers {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether anything starts the profile without a click: its Auto badge.
    pub fn is_auto(&self) -> bool {
        self.on_save
            || self.keep_up_to_date
            || self.before_close != BeforeClose::Off
            || self.on_tag.is_some()
    }

    /// Whether a save of the file named `file_name` starts the profile.
    pub fn watches(&self, file_name: &str) -> bool {
        if !(self.on_save || self.keep_up_to_date) {
            return false;
        }
        let name = file_name.to_ascii_lowercase();
        if self.sources.is_empty() {
            return name.ends_with(".snxsch") || name.ends_with(".snxpcb");
        }
        self.sources.iter().any(|source| {
            let source = source.trim().to_ascii_lowercase();
            match source.strip_prefix('*') {
                Some(tail) => name.ends_with(tail),
                None => name == source,
            }
        })
    }
}

/// One run profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobProfile {
    /// Unique in the job, compared without case.
    pub name: String,
    /// The ticked outputs, by name.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// The one variant the profile builds; the project's active variant
    /// when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Where the files go, beside the job file; the job's folder when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Run ERC, DRC and the BOM checks before anything is generated.
    #[serde(default = "yes")]
    pub validate: bool,
    #[serde(default)]
    pub on_errors: OnErrors,
    #[serde(default)]
    pub on_warnings: OnWarnings,
    /// Also write a zip of the run's files beside its folder.
    #[serde(default)]
    pub zip: bool,
    /// One subfolder per group: Fabrication, Assembly, Documentation, Reports.
    #[serde(default)]
    pub group_folders: bool,
    /// Write into a release folder named with the revision and the date.
    #[serde(default)]
    pub release_folder: bool,
    /// Delete the files in the folder this run did not write.
    #[serde(default)]
    pub clear_folder: bool,
    /// The stem default file names start with: `{project}`, `{rev}` and
    /// `{variant}` are filled in; `{project}` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naming: Option<String>,
    #[serde(default, skip_serializing_if = "JobTriggers::is_default")]
    pub triggers: JobTriggers,
    #[serde(default, flatten, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
}

impl JobProfile {
    /// A profile named `name` ticking `outputs`, with the defaults of the
    /// mockup: validate, stop on errors, continue on warnings.
    pub fn new(name: &str, outputs: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            outputs,
            variant: None,
            folder: None,
            validate: true,
            on_errors: OnErrors::Stop,
            on_warnings: OnWarnings::Continue,
            zip: false,
            group_folders: false,
            release_folder: false,
            clear_folder: false,
            naming: None,
            triggers: JobTriggers::default(),
            unknown: UnknownFields::default(),
        }
    }

    pub fn ticks(&self, output: &str) -> bool {
        self.outputs.iter().any(|name| name.trim() == output.trim())
    }
}

/// How the files of a run are laid out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Packaging {
    pub zip: bool,
    pub group_folders: bool,
    pub release_folder: bool,
    pub clear_folder: bool,
}

/// What a run asks for.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunRequest<'a> {
    /// The profile by name; the job's default profile when `None`.
    pub profile: Option<&'a str>,
    /// The one output to write.
    pub only: Option<&'a str>,
    /// Run Selected: generate the ticked outputs only, without Validate
    /// and without a zip.
    pub selected: bool,
    /// Keep outputs up to date: only these outputs, when ticked.
    pub among: Option<&'a [String]>,
}

/// What a run does.
#[derive(Debug, Clone, PartialEq)]
pub struct RunPlan {
    /// The profile run; `None` when the job has none and every enabled
    /// output runs.
    pub profile: Option<String>,
    pub plan: JobPlan,
    pub variant: Option<String>,
    /// The folder, relative to the job file's.
    pub folder: String,
    pub validate: bool,
    pub on_errors: OnErrors,
    pub on_warnings: OnWarnings,
    pub packaging: Packaging,
    pub naming: Option<String>,
}

impl OutputJob {
    /// The profile named `name`, compared without case.
    pub fn profile(&self, name: &str) -> Option<&JobProfile> {
        let name = name.trim();
        self.profiles
            .iter()
            .find(|profile| profile.name.trim().eq_ignore_ascii_case(name))
    }

    pub fn profile_mut(&mut self, name: &str) -> Option<&mut JobProfile> {
        let name = name.trim();
        self.profiles
            .iter_mut()
            .find(|profile| profile.name.trim().eq_ignore_ascii_case(name))
    }

    /// The profile a run without one uses: the default when it exists.
    pub fn default_profile(&self) -> Option<&JobProfile> {
        self.default_profile
            .as_deref()
            .and_then(|name| self.profile(name))
    }

    /// What `request` runs, with default files named from `stem`.
    pub fn run_plan(&self, stem: &str, request: &RunRequest<'_>) -> Result<RunPlan, PlanProblem> {
        let profile = match request.profile.map(str::trim).filter(|p| !p.is_empty()) {
            Some(name) => Some(
                self.profile(name)
                    .ok_or_else(|| PlanProblem::NoSuchProfile(name.to_string()))?,
            ),
            None => self.default_profile(),
        };
        let Some(profile) = profile else {
            let plan = self.plan_among(stem, request.only, request.among)?;
            return Ok(RunPlan {
                profile: None,
                plan,
                variant: self.variant.clone(),
                folder: self.folder.clone(),
                validate: false,
                on_errors: OnErrors::Stop,
                on_warnings: OnWarnings::Continue,
                packaging: Packaging::default(),
                naming: None,
            });
        };
        // A run of one output by name ignores the ticks.
        let ticked: Vec<String> = match request.only {
            Some(_) => self.outputs.iter().map(|o| o.name.clone()).collect(),
            None => profile
                .outputs
                .iter()
                .filter(|name| request.among.is_none_or(|among| among.contains(name)))
                .cloned()
                .collect(),
        };
        let plan = match self.plan_among(stem, request.only, Some(ticked.as_slice())) {
            Err(PlanProblem::Nothing) => {
                return Err(PlanProblem::NothingTicked(profile.name.clone()));
            }
            other => other?,
        };
        Ok(RunPlan {
            profile: Some(profile.name.clone()),
            plan,
            variant: profile.variant.clone().or_else(|| self.variant.clone()),
            folder: profile
                .folder
                .clone()
                .unwrap_or_else(|| self.folder.clone()),
            validate: profile.validate && !request.selected,
            on_errors: profile.on_errors,
            on_warnings: profile.on_warnings,
            packaging: Packaging {
                zip: profile.zip && !request.selected,
                group_folders: profile.group_folders,
                release_folder: profile.release_folder,
                clear_folder: profile.clear_folder,
            },
            naming: profile.naming.clone(),
        })
    }

    /// Why the profiles cannot be saved: a blank or repeated name, a tick
    /// of no output, a default that names no profile, a folder outside.
    pub(super) fn profile_fault(&self) -> Option<super::OutputJobFault> {
        use super::OutputJobFault as F;
        let mut names = BTreeSet::new();
        for profile in &self.profiles {
            let name = profile.name.trim();
            if name.is_empty() {
                return Some(F::NoProfileName);
            }
            if !names.insert(name.to_lowercase()) {
                return Some(F::ProfileTwice(name.to_string()));
            }
            if let Some(folder) = &profile.folder
                && super::relative_folder(folder).is_none()
            {
                return Some(F::BadFolder(folder.clone()));
            }
            if profile.triggers.wait_seconds > MAX_SAVE_WAIT_SECONDS {
                return Some(F::BadWait {
                    profile: name.to_string(),
                    seconds: profile.triggers.wait_seconds,
                });
            }
        }
        match &self.default_profile {
            Some(name) if self.profile(name).is_none() => Some(F::NoDefault(name.clone())),
            _ => None,
        }
    }
}

/// `pattern` with `{project}`, `{rev}` and `{variant}` filled in; a token
/// with no value is left out with the `-` before it.
pub fn expand_naming(pattern: &str, project: &str, rev: &str, variant: &str) -> String {
    let mut out = pattern.to_string();
    for (token, value) in [
        ("{project}", project),
        ("{rev}", rev),
        ("{variant}", variant),
    ] {
        if value.is_empty() {
            out = out.replace(&format!("-{token}"), "").replace(token, "");
        } else {
            out = out.replace(token, value);
        }
    }
    let out = out.trim().trim_matches('-').to_string();
    if out.is_empty() {
        project.to_string()
    } else {
        out
    }
}

/// The release folder of a run on `date` (`YYYY-MM-DD`): `<stem>-rev<R>-<date>`,
/// or `<stem>-<date>` with no revision.
pub fn release_folder_name(stem: &str, rev: &str, date: &str) -> String {
    if rev.is_empty() {
        format!("{stem}-{date}")
    } else {
        format!("{stem}-rev{rev}-{date}")
    }
}
