//! `generate_block_diagram document=<project> [path] [overwrite]` and
//! `regenerate_block_diagram document=<project> [path] [apply]` (#2968): the
//! operands checked and the run planned on the UI thread, the run itself on
//! a worker (`job`), the caller answered when it lands (`Deferred`).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::block_diagram::BlockDiagram;

use super::job::{SyncJob, SyncMode, not_run};
use super::{BlockDiagramSyncMsg, SyncLanded, TabMerge};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::project_sheets;
use crate::app::runtime::net_class_styles::plan_projects;
use crate::app::state::LoadedProject;
use crate::app::state::project_case::fold_of;
use crate::app::state::scope::path_key_with;
use crate::app::{Message, ProjectMsg, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// A run planned on the UI thread.
struct Planned {
    job: SyncJob,
    /// The project file it reads.
    project: PathBuf,
    /// The diagram its tab holds, for the tab's own Re-sync: the run merges
    /// into it and writes nothing.
    base: Option<BlockDiagram>,
}

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
    let refused = |reason: Translatable| (CommandOutcome::Disabled { reason }, Task::none());
    let command = match AppCommandId::new(id) {
        Ok(command) => command,
        Err(reason) => return refused(not_run(reason)),
    };
    let Planned { job, project, base } = match app.block_diagram_sync_job(args, id, mode) {
        Ok(planned) => planned,
        Err(reason) => return refused(reason),
    };
    // A run that writes the file is a project write: a folder rename waits
    // for it, and it waits for a rename already moving the folder.
    let writes_file = base.is_none() && mode.writes();
    let written_to = writes_file.then(|| job.target.clone());
    if let Some(target) = &written_to {
        if let Err(why) = app.refuse_while_project_moves(target, "write") {
            return refused(crate::app::state::untranslated(why.to_string()));
        }
        app.begin_project_write(target);
    }
    let merge = base.as_ref().map(|base| (job.target.clone(), base.clone()));
    // A person's run (the translator) is answered in Messages; a caller with
    // operands waits on its ticket.
    let waiting = (!app.agent.from_view).then(|| app.wait_for_command(&command, args));
    let ticket = waiting.as_ref().map(|w| w.ticket);
    let task = Task::perform(
        crate::app::path_op_jobs::run_blocking(move || match base {
            Some(base) => job
                .run_merge(&base)
                .map(|(answer, merged)| (answer, Some(merged))),
            None => job.run().map(|answer| (answer, None)),
        }),
        move |ran| {
            let ran = ran.map_err(not_run).and_then(|ran| ran);
            let merged = ran.as_ref().ok().and_then(|(_, merged)| merged.clone());
            let landed = SyncLanded {
                waiting,
                project,
                result: ran.map(|(answer, _)| answer),
                written_to,
                merge: merge.map(|(path, base)| TabMerge { path, base, merged }),
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
    fn block_diagram_sync_job(
        &self,
        args: &CommandArgs,
        id: &str,
        mode: SyncMode,
    ) -> Result<Planned, Translatable> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(
                Translatable::new(ids::PROJECT_BLOCK_DIAGRAM_NEEDS_DOCUMENT).arg("command", id)
            );
        };
        let entry = self.resolve_document_target(named).map_err(not_run)?;
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
                Translatable::new(ids::PROJECT_BLOCK_DIAGRAM_NOT_IN_PROJECT)
                    .arg("command", id)
                    .arg("name", entry.name.clone())
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
                project_sheets::project_root_sheet_path(project).ok_or_else(|| {
                    Translatable::new(ids::PROJECT_BLOCK_DIAGRAM_NO_ROOT)
                        .arg("name", entry.name.clone())
                })?,
                fold_of(Some(project)),
            ),
        };
        // A run that writes never writes under a diagram open in a tab: the
        // tab's next save would put back what the run wrote over. The tab's
        // own Re-sync merges into the open diagram instead, as an undo step.
        let target_key = path_key_with(&target, fold);
        let held = (self.document_state.block_diagrams.editors.iter())
            .find(|(open, _)| path_key_with(open, fold) == target_key);
        let (target, base) = match held {
            Some((open, editor)) if mode.writes() => {
                let resync = self.agent.from_view
                    && mode == (SyncMode::Regenerate { apply: true })
                    && !editor.is_read_only();
                if !resync {
                    return Err(Translatable::new(ids::PROJECT_BLOCK_DIAGRAM_OPEN_IN_TAB)
                        .arg("path", target.display().to_string()));
                }
                (open.clone(), Some(editor.diagram.clone()))
            }
            _ => (target, None),
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
        Ok(Planned {
            job,
            project: project.path.clone(),
            base,
        })
    }
}

/// The `.snxblk` the `path` operand names, read against the project folder;
/// the project's `<name>.snxblk` beside its `.snxprj` without one.
fn target_path(args: &CommandArgs, project: &LoadedProject) -> Result<PathBuf, Translatable> {
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
        return Err(Translatable::new(ids::PROJECT_BLOCK_DIAGRAM_NOT_SNXBLK)
            .arg("path", path.display().to_string()));
    }
    Ok(path)
}

fn bool_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}
