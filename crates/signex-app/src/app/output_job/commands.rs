//! The catalog commands of the Export Job tab (#1299, mockup row 114): every
//! action of the tab is a command a CLI or an agent runs with operands and
//! is answered as JSON. A job open in a tab is edited as its draft (marked
//! unsaved, as the tab's own edits are); any other job file is read, edited
//! and written back off the UI thread. Rules: docs/OUTPUT_JOBS.md, "Commands".

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_types::output_job::{
    BeforeClose, JobOutput, JobProfile, MAX_SAVE_WAIT_SECONDS, OnErrors, OnWarnings, OutputJob,
    OutputKind, read_output_job, write_output_job,
};

use super::JobDocMsg;
use super::operands::{check_job_file, job_refusal, path_operand, text_operand};
use super::tab::edit::{fresh_output_name, kind_base_name};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// An edit of a job's content; `Ok` carries what the answer adds.
type JobEdit = Box<dyn FnOnce(&mut OutputJob) -> Result<Value, String> + Send>;

fn bool_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

fn owned_text(args: &CommandArgs, name: &str) -> Option<String> {
    text_operand(args, name).map(|text| text.trim().to_string())
}

/// The job operand, checked: a `.snxout` file in an open project's folder.
fn job_operand(app: &Signex, args: &CommandArgs) -> Result<PathBuf, (PathBuf, String)> {
    let Some(job) = path_operand(args) else {
        return Err((PathBuf::new(), "needs the operand `path`".to_string()));
    };
    if let Err(why) = check_job_file(&job) {
        return Err((job, why));
    }
    if app.document_state.document_project(&job).is_none() {
        let why = format!("{} is not in the folder of an open project", job.display());
        return Err((job, why));
    }
    Ok(job)
}

fn refused(job: &Path, why: impl Into<String>) -> Ran {
    (job_refusal(job, why), Task::none())
}

impl Signex {
    /// Apply `edit` to `job`: to its draft when one is held, else to its
    /// file, read and written off the UI thread.
    fn edit_job_by_command(&mut self, command: &str, args: &CommandArgs, edit: JobEdit) -> Ran {
        let job = match job_operand(self, args) {
            Ok(job) => job,
            Err((job, why)) => return refused(&job, why),
        };
        if self.document_state.output_jobs.contains_key(&job) {
            let mut added = Value::Null;
            let mut failed = None;
            self.edit_job_draft(&job, |draft| match edit(draft) {
                Ok(value) => {
                    added = value;
                    Ok(())
                }
                Err(why) => {
                    failed = Some(why.clone());
                    Err(why)
                }
            });
            if let Some(why) = failed {
                return refused(&job, why);
            }
            let fault = self
                .document_state
                .output_jobs
                .get(&job)
                .and_then(OutputJob::fault)
                .map(|fault| fault.to_string());
            let answer = json!({
                "job": job.display().to_string(),
                "saved": false,
                "fault": fault,
                "result": added,
            });
            return (CommandOutcome::Answered(answer), Task::none());
        }
        let Ok(id) = AppCommandId::new(command) else {
            return refused(&job, "not a catalog command");
        };
        let waiting = self.wait_for_command(&id, args);
        let ticket = waiting.ticket;
        let path = job.clone();
        let task = Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                let mut content = read_output_job(&path).map_err(|e| e.to_string())?;
                let added = edit(&mut content)?;
                write_output_job(&path, &content).map_err(|e| e.to_string())?;
                Ok(json!({"job": path.display().to_string(), "saved": true, "result": added}))
            }),
            move |result| {
                Message::from(JobDocMsg::CommandDone {
                    job,
                    waiting: Box::new(waiting),
                    result: result.and_then(|done| done),
                })
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// A command's read or write of a job file landed: answer it.
    pub(crate) fn job_command_done(
        &mut self,
        job: PathBuf,
        waiting: crate::app::agent::WaitingCommand,
        result: Result<Value, String>,
    ) -> Task<Message> {
        let outcome = match result {
            Ok(answer) => CommandOutcome::Answered(answer),
            Err(why) => job_refusal(&job, why),
        };
        self.finish_waiting_command(waiting, &job, move |_| outcome)
    }

    /// `read_output_job`'s answer for a held job: its content and its run.
    fn job_answer(&self, job: &Path, content: &OutputJob) -> Value {
        let mut answer = serde_json::to_value(content).unwrap_or(Value::Null);
        answer["job"] = Value::String(job.display().to_string());
        answer["unsaved"] = Value::Bool(self.document_state.is_document_dirty(job));
        if let Some(run) = self.output_jobs().runs.latest(job) {
            let nodes: Vec<Value> = run
                .nodes
                .iter()
                .map(|node| {
                    json!({
                        "name": node.name,
                        "kind": node.kind.id(),
                        "state": format!("{:?}", node.state).to_ascii_lowercase(),
                        "files": node.files,
                    })
                })
                .collect();
            answer["run"] = json!({
                "stage": format!("{:?}", run.stage).to_ascii_lowercase(),
                "profile": run.profile,
                "variant": run.variant,
                "started": run.started,
                "nodes": nodes,
                "stopped": run.stopped.as_ref().map(|s| json!({"at": s.at, "reason": s.reason})),
            });
        }
        answer
    }
}

/// `new_output_job [path=<job.snxout>]`: with a path, an empty job file is
/// written there (refused over an existing file); without, the palette's
/// row opens a new job in its tab.
pub(crate) fn new_output_job(app: &mut Signex, args: &CommandArgs) -> Ran {
    if path_operand(args).is_none() {
        return (
            CommandOutcome::Ran,
            app.list_project_jobs(super::ListPurpose::New),
        );
    }
    let job = match job_operand(app, args) {
        Ok(job) => job,
        Err((job, why)) => return refused(&job, why),
    };
    let Ok(id) = AppCommandId::new("new_output_job") else {
        return refused(&job, "not a catalog command");
    };
    let waiting = app.wait_for_command(&id, args);
    let ticket = waiting.ticket;
    let path = job.clone();
    let task = Task::perform(
        crate::app::path_op_jobs::run_blocking(move || {
            if path.exists() {
                return Err(format!("{} exists already", path.display()));
            }
            write_output_job(&path, &OutputJob::default()).map_err(|e| e.to_string())?;
            Ok(json!({"job": path.display().to_string(), "created": true}))
        }),
        move |result| {
            Message::from(JobDocMsg::CommandDone {
                job,
                waiting: Box::new(waiting),
                result: result.and_then(|done| done),
            })
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

/// `open_output_job [path=<job.snxout>]`: the job in its tab, or without a
/// path the project's job (View ▸ Panels ▸ Export Jobs).
pub(crate) fn open_output_job(app: &mut Signex, args: &CommandArgs) -> Ran {
    if path_operand(args).is_none() {
        return (CommandOutcome::Ran, app.open_project_output_job());
    }
    match job_operand(app, args) {
        Ok(job) => {
            let answer = json!({"job": job.display().to_string(), "opened": true});
            let task = app.open_output_job_tab(job);
            (CommandOutcome::Answered(answer), task)
        }
        Err((job, why)) => refused(&job, why),
    }
}

/// `read_output_job path=<job.snxout>`: the job's outputs, profiles and
/// history, and its run this session, as JSON.
pub(crate) fn read_output_job_command(app: &mut Signex, args: &CommandArgs) -> Ran {
    let job = match job_operand(app, args) {
        Ok(job) => job,
        Err((job, why)) => return refused(&job, why),
    };
    if let Some(draft) = app.document_state.output_jobs.get(&job) {
        let answer = app.job_answer(&job, draft);
        return (CommandOutcome::Answered(answer), Task::none());
    }
    let Ok(id) = AppCommandId::new("read_output_job") else {
        return refused(&job, "not a catalog command");
    };
    let run = app
        .output_jobs()
        .runs
        .latest(&job)
        .map(|_| app.job_answer(&job, &OutputJob::default())["run"].clone());
    let waiting = app.wait_for_command(&id, args);
    let ticket = waiting.ticket;
    let path = job.clone();
    let task = Task::perform(
        crate::app::path_op_jobs::run_blocking(move || {
            let content = read_output_job(&path).map_err(|e| e.to_string())?;
            let mut answer = serde_json::to_value(&content).map_err(|e| e.to_string())?;
            answer["job"] = Value::String(path.display().to_string());
            answer["unsaved"] = Value::Bool(false);
            if let Some(run) = run {
                answer["run"] = run;
            }
            Ok(answer)
        }),
        move |result| {
            Message::from(JobDocMsg::CommandDone {
                job,
                waiting: Box::new(waiting),
                result: result.and_then(|done| done),
            })
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

/// `add_job_output path= kind= [name=] [profile=]`: a new output of `kind`
/// (`gerber`, `schematic_pdf`, ...), ticked in `profile` when one is named.
pub(crate) fn add_job_output(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(kind_id) = owned_text(args, "kind") else {
        return refused(Path::new(""), "needs the operand `kind`");
    };
    let Some(kind) = OutputKind::from_id(&kind_id) else {
        let ids: Vec<&str> = OutputKind::ALL.iter().map(|k| k.id()).collect();
        let why = format!("`kind` is one of {}, not `{kind_id}`", ids.join(", "));
        return refused(Path::new(""), why);
    };
    let name = owned_text(args, "name");
    let profile = owned_text(args, "profile");
    app.edit_job_by_command(
        "add_job_output",
        args,
        Box::new(move |job| {
            let name = match name {
                Some(name) if job.output(&name).is_some() => {
                    return Err(format!("the job has an output named {name:?} already"));
                }
                Some(name) => name,
                None => fresh_output_name(job, kind_base_name(kind)),
            };
            if let Some(profile) = &profile {
                job.profile_mut(profile)
                    .ok_or_else(|| format!("the job has no profile named {profile:?}"))?
                    .outputs
                    .push(name.clone());
            }
            job.outputs.push(JobOutput::new(&name, kind));
            Ok(json!({"output": name, "kind": kind.id()}))
        }),
    )
}

/// `set_job_output path= name= [enabled=] [file=] [dpi=] [odb_folder=]
/// [remove=]`: change or remove one output.
pub(crate) fn set_job_output(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(name) = owned_text(args, "name") else {
        return refused(Path::new(""), "needs the operand `name`");
    };
    let enabled = bool_operand(args, "enabled");
    let file = owned_text(args, "file");
    let dpi = owned_text(args, "dpi");
    let odb_folder = bool_operand(args, "odb_folder");
    let remove = bool_operand(args, "remove").unwrap_or(false);
    let dpi = match dpi
        .as_deref()
        .map(|text| super::operands::parse_dpi(Some(text)))
    {
        Some(Err(why)) => return refused(Path::new(""), why),
        Some(Ok(dpi)) => Some(dpi),
        None => None,
    };
    app.edit_job_by_command(
        "set_job_output",
        args,
        Box::new(move |job| {
            if job.output(&name).is_none() {
                return Err(format!("the job has no output named {name:?}"));
            }
            if remove {
                job.outputs.retain(|o| o.name.trim() != name.trim());
                for profile in &mut job.profiles {
                    profile.outputs.retain(|n| n.trim() != name.trim());
                }
                return Ok(json!({"output": name, "removed": true}));
            }
            let output = job.output_mut(&name).ok_or("no such output")?;
            if let Some(enabled) = enabled {
                output.enabled = enabled;
            }
            if let Some(file) = file {
                output.file = Some(file).filter(|f| !f.is_empty());
            }
            if dpi.is_some() {
                output.dpi = dpi;
            }
            if odb_folder.is_some() {
                output.odb_folder = odb_folder;
            }
            let value = serde_json::to_value(&*output).unwrap_or(Value::Null);
            Ok(value)
        }),
    )
}

/// The settings `set_job_profile` reads, taken from the operands.
struct ProfileSettings {
    from: Option<String>,
    to: Option<String>,
    remove: bool,
    default: Option<bool>,
    variant: Option<String>,
    outputs: Option<Vec<String>>,
    validate: Option<bool>,
    on_errors: Option<OnErrors>,
    on_warnings: Option<OnWarnings>,
    folder: Option<String>,
    naming: Option<String>,
    switches: [(Option<bool>, fn(&mut JobProfile) -> &mut bool); 4],
}

fn policy<T>(text: Option<String>, name: &str, all: &[(&str, T)]) -> Result<Option<T>, String>
where
    T: Copy,
{
    let Some(text) = text else {
        return Ok(None);
    };
    all.iter()
        .find(|(id, _)| id.eq_ignore_ascii_case(&text))
        .map(|(_, value)| Some(*value))
        .ok_or_else(|| {
            let ids: Vec<&str> = all.iter().map(|(id, _)| *id).collect();
            format!("`{name}` is one of {}, not `{text}`", ids.join(", "))
        })
}

fn profile_settings(args: &CommandArgs) -> Result<ProfileSettings, String> {
    let on_errors = policy(
        owned_text(args, "on_errors"),
        "on_errors",
        &[("stop", OnErrors::Stop), ("ask", OnErrors::Ask)],
    )?;
    let on_warnings = policy(
        owned_text(args, "on_warnings"),
        "on_warnings",
        &[
            ("continue", OnWarnings::Continue),
            ("ask", OnWarnings::Ask),
            ("stop", OnWarnings::Stop),
        ],
    )?;
    Ok(ProfileSettings {
        from: owned_text(args, "from"),
        to: owned_text(args, "to"),
        remove: bool_operand(args, "remove").unwrap_or(false),
        default: bool_operand(args, "default"),
        variant: owned_text(args, "variant"),
        outputs: owned_text(args, "outputs").map(|list| {
            list.split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect()
        }),
        validate: bool_operand(args, "validate"),
        on_errors,
        on_warnings,
        folder: owned_text(args, "folder"),
        naming: owned_text(args, "naming"),
        switches: [
            (bool_operand(args, "zip"), zip_flag),
            (bool_operand(args, "group_folders"), group_folders_flag),
            (bool_operand(args, "release_folder"), release_folder_flag),
            (bool_operand(args, "clear_folder"), clear_folder_flag),
        ],
    })
}

fn zip_flag(profile: &mut JobProfile) -> &mut bool {
    &mut profile.zip
}

fn group_folders_flag(profile: &mut JobProfile) -> &mut bool {
    &mut profile.group_folders
}

fn release_folder_flag(profile: &mut JobProfile) -> &mut bool {
    &mut profile.release_folder
}

fn clear_folder_flag(profile: &mut JobProfile) -> &mut bool {
    &mut profile.clear_folder
}

/// `set_job_profile path= profile= [from=] [to=] [remove=] [default=]
/// [variant=] [outputs=] [validate=] [on_errors=] [on_warnings=]
/// [folder=] [naming=] [zip=] [group_folders=] [release_folder=]
/// [clear_folder=]`: make a profile (a copy of `from`, else empty), rename
/// it (`to`), delete it, or set what it runs and how.
pub(crate) fn set_job_profile(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(name) = owned_text(args, "profile") else {
        return refused(Path::new(""), "needs the operand `profile`");
    };
    let settings = match profile_settings(args) {
        Ok(settings) => settings,
        Err(why) => return refused(Path::new(""), why),
    };
    app.edit_job_by_command(
        "set_job_profile",
        args,
        Box::new(move |job| apply_profile(job, &name, settings)),
    )
}

fn apply_profile(job: &mut OutputJob, name: &str, s: ProfileSettings) -> Result<Value, String> {
    if s.remove {
        let before = job.profiles.len();
        job.profiles
            .retain(|p| !p.name.trim().eq_ignore_ascii_case(name));
        if job.profiles.len() == before {
            return Err(format!("the job has no profile named {name:?}"));
        }
        if job
            .default_profile
            .as_deref()
            .is_some_and(|d| d.trim().eq_ignore_ascii_case(name))
        {
            job.default_profile = job.profiles.first().map(|p| p.name.clone());
        }
        return Ok(json!({"profile": name, "removed": true}));
    }
    if job.profile(name).is_none() {
        let made = match &s.from {
            Some(from) => JobProfile {
                name: name.to_string(),
                ..job
                    .profile(from)
                    .ok_or_else(|| format!("the job has no profile named {from:?}"))?
                    .clone()
            },
            None => JobProfile::new(name, Vec::new()),
        };
        job.profiles.push(made);
    }
    if let Some(outputs) = &s.outputs
        && let Some(missing) = outputs.iter().find(|n| job.output(n).is_none())
    {
        return Err(format!("the job has no output named {missing:?}"));
    }
    let renamed = s.to.clone().unwrap_or_else(|| name.to_string());
    let was_default = job
        .default_profile
        .as_deref()
        .is_some_and(|d| d.trim().eq_ignore_ascii_case(name));
    let profile = job.profile_mut(name).ok_or("no such profile")?;
    profile.name = renamed.clone();
    if let Some(variant) = s.variant {
        profile.variant = Some(variant).filter(|v| !v.is_empty());
    }
    if let Some(outputs) = s.outputs {
        profile.outputs = outputs;
    }
    if let Some(validate) = s.validate {
        profile.validate = validate;
    }
    if let Some(policy) = s.on_errors {
        profile.on_errors = policy;
    }
    if let Some(policy) = s.on_warnings {
        profile.on_warnings = policy;
    }
    if let Some(folder) = s.folder {
        profile.folder = Some(folder).filter(|f| !f.is_empty());
    }
    if let Some(naming) = s.naming {
        profile.naming = Some(naming).filter(|n| !n.is_empty());
    }
    for (value, flag) in s.switches {
        if let Some(on) = value {
            *flag(profile) = on;
        }
    }
    let value = serde_json::to_value(&*profile).unwrap_or(Value::Null);
    if s.default == Some(true) || (was_default && s.default != Some(false)) {
        job.default_profile = Some(renamed);
    } else if s.default == Some(false) && was_default {
        job.default_profile = None;
    }
    Ok(value)
}

/// `set_job_triggers path= profile= [on_save=] [sources=] [wait=]
/// [keep_up_to_date=] [before_close=] [tag=]`: what starts a profile
/// without a click.
pub(crate) fn set_job_triggers(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(name) = owned_text(args, "profile") else {
        return refused(Path::new(""), "needs the operand `profile`");
    };
    let before_close = match policy(
        owned_text(args, "before_close"),
        "before_close",
        &[
            ("off", BeforeClose::Off),
            ("ask", BeforeClose::Ask),
            ("always", BeforeClose::Always),
        ],
    ) {
        Ok(policy) => policy,
        Err(why) => return refused(Path::new(""), why),
    };
    let wait = match owned_text(args, "wait") {
        None => None,
        Some(text) => match text
            .parse::<u32>()
            .ok()
            .filter(|s| *s <= MAX_SAVE_WAIT_SECONDS)
        {
            Some(seconds) => Some(seconds),
            None => {
                let why = format!(
                    "`wait` is a whole number of seconds up to {MAX_SAVE_WAIT_SECONDS}, not `{text}`"
                );
                return refused(Path::new(""), why);
            }
        },
    };
    let on_save = bool_operand(args, "on_save");
    let keep = bool_operand(args, "keep_up_to_date");
    let sources = owned_text(args, "sources");
    let tag = owned_text(args, "tag");
    app.edit_job_by_command(
        "set_job_triggers",
        args,
        Box::new(move |job| {
            let profile = job
                .profile_mut(&name)
                .ok_or_else(|| format!("the job has no profile named {name:?}"))?;
            let triggers = &mut profile.triggers;
            if let Some(on) = on_save {
                triggers.on_save = on;
            }
            if let Some(on) = keep {
                triggers.keep_up_to_date = on;
            }
            if let Some(seconds) = wait {
                triggers.wait_seconds = seconds;
            }
            if let Some(when) = before_close {
                triggers.before_close = when;
            }
            if let Some(sources) = sources {
                triggers.sources = sources
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            if let Some(tag) = tag {
                triggers.on_tag = Some(tag).filter(|t| !t.is_empty());
            }
            Ok(serde_json::to_value(&*triggers).unwrap_or(Value::Null))
        }),
    )
}

/// `stop_output_job path=`: the run of the job going now stops; nothing of
/// it is written.
pub(crate) fn stop_output_job(app: &mut Signex, args: &CommandArgs) -> Ran {
    let job = match job_operand(app, args) {
        Ok(job) => job,
        Err((job, why)) => return refused(&job, why),
    };
    if app.output_jobs().runs.running(&job).is_none() {
        return refused(&job, "no run of this job is going");
    }
    let task = app.stop_job_run_of(&job);
    let answer = json!({"job": job.display().to_string(), "stopped": true});
    (CommandOutcome::Answered(answer), task)
}

/// `view_output_job_run path= [run=<n>]`: the files the run wrote that the
/// Gerber Viewer reads open on its tab (mockup row 193b); `run` is the
/// history row, 0 the newest, else this session's last run.
pub(crate) fn view_output_job_run(app: &mut Signex, args: &CommandArgs) -> Ran {
    let job = match job_operand(app, args) {
        Ok(job) => job,
        Err((job, why)) => return refused(&job, why),
    };
    let row = match owned_text(args, "run") {
        None => None,
        Some(text) => match text.parse::<usize>() {
            Ok(row) => Some(row),
            Err(_) => return refused(&job, format!("`run` is a history row number, not `{text}`")),
        },
    };
    let Some(place) = app.job_run_place(&job, row) else {
        return refused(
            &job,
            "the job has no run to view; open it in its tab to read its history",
        );
    };
    if !place.complete || place.viewable.is_empty() {
        return refused(&job, "that run wrote no Gerber or drill file");
    }
    let files: Vec<String> = place
        .viewable
        .iter()
        .map(|file| place.folder.join(file).display().to_string())
        .collect();
    let task = app.view_job_run_in_gerber(&job, row);
    let answer = json!({"job": job.display().to_string(), "files": files});
    (CommandOutcome::Answered(answer), task)
}
