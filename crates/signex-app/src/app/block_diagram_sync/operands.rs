//! `generate_block_diagram document=<project> [path] [overwrite]` and
//! `regenerate_block_diagram document=<project> [path] [apply]` (#2968): the
//! operands checked and the run planned on the UI thread, the run itself on
//! a worker (`job`), the caller answered when it lands (`Deferred`).

use std::path::{Path, PathBuf};

use iced::Task;

use super::job::{SyncJob, SyncMode};
use super::{BlockDiagramSyncMsg, SyncLanded};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::project_sheets;
use crate::app::runtime::net_class_styles::plan_projects;
use crate::app::state::LoadedProject;
use crate::app::state::project_case::fold_of;
use crate::app::{Message, ProjectMsg, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The file extension of a block diagram, without the dot.
const SNXBLK: &str = signex_types::block_diagram::BLOCK_DIAGRAM_EXTENSION;

pub(crate) fn generate_block_diagram(app: &mut Signex, args: &CommandArgs) -> Ran {
    let overwrite = bool_operand(args, "overwrite").unwrap_or(false);
    start(
        app,
        args,
        "generate_block_diagram",
        SyncMode::Generate { overwrite },
    )
}

pub(crate) fn regenerate_block_diagram(app: &mut Signex, args: &CommandArgs) -> Ran {
    let apply = bool_operand(args, "apply").unwrap_or(false);
    start(
        app,
        args,
        "regenerate_block_diagram",
        SyncMode::Regenerate { apply },
    )
}

fn start(app: &mut Signex, args: &CommandArgs, id: &str, mode: SyncMode) -> Ran {
    let refused = |reason: String| (CommandOutcome::disabled(reason), Task::none());
    let command = match AppCommandId::new(id) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let (job, project) = match app.block_diagram_job(args, id, mode) {
        Ok(planned) => planned,
        Err(reason) => return refused(reason),
    };
    // A person's run (the translator) is answered in Messages; a caller with
    // operands waits on its ticket.
    let waiting = (!app.agent.from_view).then(|| app.wait_for_command(&command, args));
    let ticket = waiting.as_ref().map(|w| w.ticket);
    let task = Task::perform(
        crate::app::path_op_jobs::run_blocking(move || job.run()),
        move |result| {
            let landed = SyncLanded {
                waiting,
                project,
                result: result.and_then(|answer| answer),
            };
            Message::Project(ProjectMsg::BlockDiagramSync(BlockDiagramSyncMsg::Synced(
                Box::new(landed),
            )))
        },
    );
    match ticket {
        Some(ticket) => (CommandOutcome::Deferred { ticket }, task),
        // The translator passes on an answer, a refusal or a wait only.
        None => {
            let started = serde_json::json!({ "started": id });
            (CommandOutcome::Answered(started), task)
        }
    }
}

impl Signex {
    /// The run `args` asks for and the project file it reads; `Err` says why
    /// it cannot start.
    fn block_diagram_job(
        &self,
        args: &CommandArgs,
        id: &str,
        mode: SyncMode,
    ) -> Result<(SyncJob, PathBuf), String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(format!("`{id}` needs the operand `document`"));
        };
        let entry = self.resolve_document_target(named)?;
        let project_path = match entry.kind {
            DocumentKind::Project => entry.path.clone(),
            _ => entry.project.as_ref().map(|project| project.path.clone()),
        };
        let project = project_path
            .and_then(|path| {
                self.document_state
                    .projects
                    .iter()
                    .find(|project| project.path == path)
            })
            .ok_or_else(|| {
                format!(
                    "`{id}` works on a project, and {} is not part of an open one",
                    entry.name
                )
            })?;
        let target = target_path(args, project)?;
        let planned = plan_projects(&self.document_state)
            .into_iter()
            .find(|p| p.project == project.id);
        let (open, pages, root, fold) = match planned {
            Some(planned) => (
                project_sheets::open_sheets_snapshot_of(&self.document_state, &planned.open_paths),
                planned.key.pages,
                planned.key.root,
                planned.key.fold,
            ),
            None => (
                Default::default(),
                project_sheets::project_pages(Some(project)),
                project_sheets::project_root_sheet_path(project)
                    .ok_or_else(|| format!("the project {} has no root sheet", entry.name))?,
                fold_of(Some(project)),
            ),
        };
        let job = SyncJob {
            mode,
            target,
            project_dir: project.dir().to_path_buf(),
            open,
            pages,
            root,
            fold,
            naming: project.data.schematic_design.channel_naming.clone(),
        };
        Ok((job, project.path.clone()))
    }
}

/// The `.snxblk` the `path` operand names, read against the project folder;
/// the project's `<name>.snxblk` beside its `.snxprj` without one.
fn target_path(args: &CommandArgs, project: &LoadedProject) -> Result<PathBuf, String> {
    let path = match args.operand("path") {
        Some(OperandValue::Path(text)) => {
            let given = Path::new(text);
            if given.is_absolute() {
                given.to_path_buf()
            } else {
                project.dir().join(given)
            }
        }
        _ => {
            let stem = project
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project".to_string());
            project.dir().join(format!("{stem}.{SNXBLK}"))
        }
    };
    let is_snxblk = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(SNXBLK));
    if !is_snxblk {
        return Err(format!(
            "{} is not a block diagram file (.{SNXBLK})",
            path.display()
        ));
    }
    Ok(path)
}

fn bool_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}
