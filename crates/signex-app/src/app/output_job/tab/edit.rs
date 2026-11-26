//! The tab's edits of its job (mockup row 114): outputs, ticks, the shown
//! profile's variant, container, policy and triggers, and the profiles
//! themselves. Each edit changes the draft and marks the job dirty; nothing
//! is written until Save. An edit the job could not be saved with (a blank
//! or repeated name) is refused and named in Messages.

use std::path::Path;

use iced::Task;
use signex_types::output_job::{
    DEFAULT_SAVE_WAIT_SECONDS, JobOutput, JobProfile, MAX_PNG_DPI, MAX_SAVE_WAIT_SECONDS,
    MIN_PNG_DPI, OnErrors, OutputJob, OutputKind,
};

use super::{JobTabMsg, NamingMode, PackagingSwitch, TriggerEdit};
use crate::app::{Message, Signex};

/// A new output's name: the kind's catalog name, numbered when taken.
pub(crate) fn fresh_output_name(job: &OutputJob, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while job.output(&name).is_some() {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}

/// A new profile's name, numbered when taken.
fn fresh_profile_name(job: &OutputJob, base: &str) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while job.profile(&name).is_some() {
        name = format!("{base} {n}");
        n += 1;
    }
    name
}

/// The English name a new output of `kind` takes in the file; the tab
/// shows the kind's catalog name beside it.
pub(crate) fn kind_base_name(kind: OutputKind) -> &'static str {
    match kind {
        OutputKind::SchematicPdf => "Schematic PDF",
        OutputKind::SchematicPng => "Schematic PNG",
        OutputKind::Bom => "BOM",
        OutputKind::Netlist => "Netlist",
        OutputKind::Gerber => "Gerber",
        OutputKind::NcDrill => "NC Drill",
        OutputKind::PickAndPlace => "Pick and Place",
        OutputKind::TestPoints => "Test points",
        OutputKind::Ipc2581 => "IPC-2581",
        OutputKind::Odb => "ODB++",
        OutputKind::Step => "3D STEP",
        OutputKind::AssemblyPdf => "Assembly drawings",
    }
}

/// The profile `name` of `job`, made when the job has none yet: a job
/// without profiles gets "Default", ticking every output.
fn profile_for_edit<'a>(job: &'a mut OutputJob, name: Option<&str>) -> &'a mut JobProfile {
    if job.profiles.is_empty() {
        let all = job.outputs.iter().map(|o| o.name.clone()).collect();
        let mut profile = JobProfile::new("Default", all);
        profile.variant = job.variant.clone();
        job.default_profile = Some(profile.name.clone());
        job.profiles.push(profile);
    }
    let index = name
        .and_then(|name| {
            job.profiles
                .iter()
                .position(|p| p.name.trim().eq_ignore_ascii_case(name.trim()))
        })
        .unwrap_or(0);
    &mut job.profiles[index]
}

impl Signex {
    /// Apply an edit message to its job's draft.
    pub(super) fn edit_job_from_tab(&mut self, msg: JobTabMsg) -> Task<Message> {
        use JobTabMsg as M;
        let job = match &msg {
            M::OutputEnabledToggled { job, .. }
            | M::OutputTicked { job, .. }
            | M::OutputAdded { job, .. }
            | M::OutputRemoved { job, .. }
            | M::OutputFileEdited { job, .. }
            | M::OutputDpiEdited { job, .. }
            | M::OdbFolderToggled { job, .. }
            | M::ProfileNamingConfirmed { job }
            | M::ProfileDuplicated { job }
            | M::ProfileDeleted { job }
            | M::ProfileMadeDefault { job }
            | M::VariantPicked { job, .. }
            | M::FolderEdited { job, .. }
            | M::NamingEdited { job, .. }
            | M::PackagingToggled { job, .. }
            | M::OnErrorsPicked { job, .. }
            | M::OnWarningsPicked { job, .. }
            | M::TriggerEdited { job, .. }
            | M::AskNextTimePressed { job } => job.clone(),
            _ => return Task::none(),
        };
        let shown = self.shown_profile_name(&job);
        let naming = self
            .output_jobs()
            .tabs
            .get(&job)
            .and_then(|ui| ui.naming.clone());
        let confirmed = matches!(msg, M::ProfileNamingConfirmed { .. });
        let typed = naming.as_ref().map(|(_, typed)| typed.trim().to_string());
        let mut picked: Option<String> = None;
        let changed = self.edit_job_draft(&job, |draft| {
            apply(draft, msg, shown.as_deref(), naming, &mut picked)
        });
        if let Some(ui) = self.output_jobs_mut().tabs.get_mut(&job) {
            ui.add_menu = false;
            if let Some(name) = picked {
                ui.selected = Some(name);
            }
            if confirmed && changed {
                ui.profile = typed;
                ui.naming = None;
            }
        }
        Task::none()
    }

    /// Change `job`'s draft with `edit` and mark it dirty when it changed.
    /// An edit `edit` refuses is undone and named. A draft that cannot be
    /// saved yet (a folder half typed) is kept: the tab names the fault and
    /// Save and Run refuse it. Returns whether the draft changed.
    pub(crate) fn edit_job_draft(
        &mut self,
        job: &Path,
        edit: impl FnOnce(&mut OutputJob) -> Result<(), String>,
    ) -> bool {
        let Some(draft) = self.document_state.output_jobs.get_mut(job) else {
            return false;
        };
        let before = draft.clone();
        if let Err(why) = edit(draft) {
            *draft = before;
            crate::diagnostics::log_warning(format!("Export Job {}: {why}", job.display()));
            return false;
        }
        let changed = *draft != before;
        if changed {
            self.document_state.mark_document_dirty(job);
        }
        changed
    }
}

/// Apply one edit to `job`. `shown` is the profile the tab shows; `naming`
/// the profile name being typed; `picked` takes a new output's name.
fn apply(
    job: &mut OutputJob,
    msg: JobTabMsg,
    shown: Option<&str>,
    naming: Option<(NamingMode, String)>,
    picked: &mut Option<String>,
) -> Result<(), String> {
    use JobTabMsg as M;
    match msg {
        M::OutputEnabledToggled { name, .. } => {
            let output = job.output_mut(&name).ok_or("no such output")?;
            output.enabled = !output.enabled;
        }
        M::OutputTicked { name, .. } => {
            let profile = profile_for_edit(job, shown);
            match profile.ticks(&name) {
                true => profile.outputs.retain(|n| n.trim() != name.trim()),
                false => profile.outputs.push(name),
            }
        }
        M::OutputAdded { kind, .. } => {
            let name = fresh_output_name(job, kind_base_name(kind));
            job.outputs.push(JobOutput::new(&name, kind));
            if let Some(profile) = shown.and_then(|shown| job.profile_mut(shown)) {
                profile.outputs.push(name.clone());
            }
            *picked = Some(name);
        }
        M::OutputRemoved { name, .. } => {
            job.outputs.retain(|o| o.name.trim() != name.trim());
            for profile in &mut job.profiles {
                profile.outputs.retain(|n| n.trim() != name.trim());
            }
        }
        M::OutputFileEdited { name, text, .. } => {
            let output = job.output_mut(&name).ok_or("no such output")?;
            output.file = Some(text.trim().to_string()).filter(|t| !t.is_empty());
        }
        M::OutputDpiEdited { name, text, .. } => {
            // Out of range is kept while typed; the tab names the fault.
            let dpi = match text.trim() {
                "" => None,
                typed => Some(typed.parse::<u32>().map_err(|_| {
                    format!("dpi is a whole number from {MIN_PNG_DPI} to {MAX_PNG_DPI}")
                })?),
            };
            job.output_mut(&name).ok_or("no such output")?.dpi = dpi;
        }
        M::OdbFolderToggled { name, .. } => {
            let output = job.output_mut(&name).ok_or("no such output")?;
            output.odb_folder = Some(!output.odb_folder.unwrap_or(false));
        }
        M::ProfileNamingConfirmed { .. } => confirm_naming(job, shown, naming)?,
        M::ProfileDuplicated { .. } => {
            let from = profile_for_edit(job, shown).clone();
            let name = fresh_profile_name(job, &from.name);
            job.profiles.push(JobProfile { name, ..from });
        }
        M::ProfileDeleted { .. } => {
            let name = shown.ok_or("no profile to delete")?.to_string();
            job.profiles
                .retain(|p| !p.name.trim().eq_ignore_ascii_case(name.trim()));
            if job
                .default_profile
                .as_deref()
                .is_some_and(|d| d.trim().eq_ignore_ascii_case(name.trim()))
            {
                job.default_profile = job.profiles.first().map(|p| p.name.clone());
            }
        }
        M::ProfileMadeDefault { .. } => {
            let name = profile_for_edit(job, shown).name.clone();
            job.default_profile = Some(name);
        }
        M::VariantPicked { variant, .. } => {
            profile_for_edit(job, shown).variant = Some(variant);
        }
        M::FolderEdited { text, .. } => {
            profile_for_edit(job, shown).folder =
                Some(text.trim().to_string()).filter(|t| !t.is_empty());
        }
        M::NamingEdited { text, .. } => {
            profile_for_edit(job, shown).naming =
                Some(text.trim().to_string()).filter(|t| !t.is_empty());
        }
        M::PackagingToggled { switch, .. } => {
            let profile = profile_for_edit(job, shown);
            let flag = match switch {
                PackagingSwitch::Zip => &mut profile.zip,
                PackagingSwitch::GroupFolders => &mut profile.group_folders,
                PackagingSwitch::ReleaseFolder => &mut profile.release_folder,
                PackagingSwitch::ClearFolder => &mut profile.clear_folder,
                PackagingSwitch::Validate => &mut profile.validate,
            };
            *flag = !*flag;
        }
        M::OnErrorsPicked { policy, .. } => profile_for_edit(job, shown).on_errors = policy,
        M::OnWarningsPicked { policy, .. } => profile_for_edit(job, shown).on_warnings = policy,
        M::AskNextTimePressed { .. } => profile_for_edit(job, shown).on_errors = OnErrors::Ask,
        M::TriggerEdited { edit, .. } => edit_trigger(profile_for_edit(job, shown), edit)?,
        _ => {}
    }
    Ok(())
}

/// Save as profile… makes a copy of the shown profile under the typed
/// name; Rename… renames it, and the default follows.
fn confirm_naming(
    job: &mut OutputJob,
    shown: Option<&str>,
    naming: Option<(NamingMode, String)>,
) -> Result<(), String> {
    let (mode, typed) = naming.ok_or("no profile name is being typed")?;
    let typed = typed.trim().to_string();
    if typed.is_empty() {
        return Err("a profile needs a name".to_string());
    }
    match mode {
        NamingMode::SaveAs => {
            let from = profile_for_edit(job, shown).clone();
            job.profiles.push(JobProfile {
                name: typed,
                ..from
            });
        }
        NamingMode::Rename => {
            let profile = profile_for_edit(job, shown);
            let old = std::mem::replace(&mut profile.name, typed.clone());
            if job
                .default_profile
                .as_deref()
                .is_some_and(|d| d.trim().eq_ignore_ascii_case(old.trim()))
            {
                job.default_profile = Some(typed);
            }
        }
    }
    Ok(())
}

fn edit_trigger(profile: &mut JobProfile, edit: TriggerEdit) -> Result<(), String> {
    let triggers = &mut profile.triggers;
    match edit {
        TriggerEdit::OnSaveToggled => triggers.on_save = !triggers.on_save,
        TriggerEdit::SourcesEdited(text) => {
            triggers.sources = text
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
        }
        TriggerEdit::WaitEdited(text) => {
            triggers.wait_seconds = match text.trim() {
                "" => DEFAULT_SAVE_WAIT_SECONDS,
                typed => typed
                    .parse::<u32>()
                    .ok()
                    .filter(|s| *s <= MAX_SAVE_WAIT_SECONDS)
                    .ok_or_else(|| {
                        format!(
                            "the wait is a whole number of seconds up to {MAX_SAVE_WAIT_SECONDS}"
                        )
                    })?,
            };
        }
        TriggerEdit::KeepUpToDateToggled => triggers.keep_up_to_date = !triggers.keep_up_to_date,
        TriggerEdit::BeforeClosePicked(when) => triggers.before_close = when,
        TriggerEdit::TagEdited(text) => {
            triggers.on_tag = Some(text.trim().to_string()).filter(|t| !t.is_empty());
        }
    }
    Ok(())
}
