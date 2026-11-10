//! Prefetch, then run (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6771692137): a
//! headless or agent edit does no file IO inside `update` (epic #665). What
//! its checks read from disk (the project's closed sheets for the name check,
//! the designators and the hierarchy-loop check; a library row for
//! `place_component`; the child file of `sync_sheet_pins`; whether a child
//! reference leaves the project through a link) is read by a `Task` off the
//! UI thread, and the edit runs when the reads land, with its outcome reported
//! then. An `apply_edits` list plans the union of its edits once; a closed
//! file a `place_sheet_symbol` of the list refers to is read with the project.
//! The revisions of the target and of its project's open sheets are recorded
//! at the start; if one changed, or a project sheet was opened or closed, by
//! completion, the edit is refused, nothing applied. An edit that needs no
//! disk (a loose sheet, a project whose sheets are all open) runs at once.
//! Rules: `docs/AGENT_EDITS.md`, "Edits that read the disk".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::document_operands::named_sheet;
use super::placement_operands::text;
use super::{CommandArgs, CommandOutcome};
use crate::app::agent::{AgentMsg, Origin};
use crate::app::project_sheets::{
    OpenSheets, ProjectSheetSet, assemble_project_sheets_quiet, assemble_project_sheets_quiet_from,
    open_sheets_snapshot, project_pages, resolve_child_reference_quiet, resolve_netlist_root_for,
};
use crate::app::state::scope::{CaseFold, path_key_with};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::library::messages::ResolvedPlacement;

type Ran = (CommandOutcome, Task<Message>);

/// A library row: the `.snxlib` file, the table and the row id.
pub(crate) type PartKey = (PathBuf, String, Uuid);

/// What an edit's checks read from disk, read off the UI thread.
#[derive(Default)]
pub struct EditReads {
    /// The target's project, assembled from a snapshot of the open sheets and
    /// the disk; the open sheets are replaced by their live state when used.
    pub(crate) project: Option<ProjectSheetSet>,
    pub(crate) parts: HashMap<PartKey, Result<ResolvedPlacement, String>>,
    /// Child sheet files by path: `None` when there is no file.
    pub(crate) children: HashMap<PathBuf, Result<Option<SchematicSheet>, String>>,
    /// Whether a path leaves its project folder through a link.
    pub(crate) escapes: HashMap<PathBuf, bool>,
    /// Why the reads did not run to their end (the reader panicked).
    pub(crate) failed: Option<String>,
}

/// The reads as they landed: a reader that panicked leaves none and says so.
pub(super) fn landed(result: Result<EditReads, tokio::task::JoinError>) -> EditReads {
    result.unwrap_or_else(|error| EditReads {
        failed: Some(error.to_string()),
        ..EditReads::default()
    })
}

impl std::fmt::Debug for EditReads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EditReads {{ project: {}, parts: {}, children: {}, links: {} }}",
            self.project.is_some(),
            self.parts.len(),
            self.children.len(),
            self.escapes.len()
        )
    }
}

/// The reads an edit needs, captured in `update`.
#[derive(Default)]
struct ReadPlan {
    project: Option<ProjectRead>,
    /// Closed files the list's `place_sheet_symbol` edits refer to: read
    /// with the project, as children-to-be, not as pages of their own.
    seeds: Vec<PathBuf>,
    parts: Vec<PartKey>,
    open_libraries: Vec<(Uuid, PathBuf)>,
    children: Vec<PathBuf>,
    /// `(project folder, path)` to check for a link out of the folder.
    links: Vec<(PathBuf, PathBuf)>,
}

impl ReadPlan {
    fn is_empty(&self) -> bool {
        self.project.is_none()
            && self.parts.is_empty()
            && self.children.is_empty()
            && self.links.is_empty()
    }

    /// Read everything. Blocking: run it off the UI thread.
    fn read(self) -> EditReads {
        let seeds = self.seeds;
        let project = self.project.map(|read| {
            let walked: Vec<PathBuf> = read.pages.iter().chain(&seeds).cloned().collect();
            let mut set =
                assemble_project_sheets_quiet_from(&read.open, &walked, &read.root, read.fold);
            // A child-to-be is no top-level page: the edit places it.
            set.pages_outside_the_hierarchy
                .retain(|page| !seeds.contains(page));
            set
        });
        let parts = self
            .parts
            .into_iter()
            .map(|(root, table, row)| {
                let read = crate::library::place::resolve_placement_with_footprint(
                    &root,
                    &table,
                    signex_library::RowId(row),
                    None,
                    &self.open_libraries,
                );
                ((root, table, row), read)
            })
            .collect();
        let children = self
            .children
            .into_iter()
            .map(|path| {
                let read = crate::app::project_sheets::load_sheet_file(&path);
                (path, read)
            })
            .collect();
        let escapes = self
            .links
            .into_iter()
            .map(|(dir, path)| {
                let escapes = crate::app::handlers::canvas::escapes_through_symlink(&dir, &path);
                (path, escapes)
            })
            .collect();
        EditReads {
            project,
            parts,
            children,
            escapes,
            failed: None,
        }
    }
}

/// The project to read: a snapshot of its open sheets, its pages and root.
struct ProjectRead {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
}

/// The completion of an edit's reads: the reads and the request they are for.
#[derive(Debug, Clone)]
pub struct ReadsLoaded {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    /// The sheet the edit acts on.
    pub(crate) target: PathBuf,
    /// The revision of the target and of each open sheet of its project.
    pub(crate) revisions: BTreeMap<PathBuf, u64>,
    pub(crate) origin: Origin,
    pub(crate) serving: bool,
    pub(crate) reads: Arc<EditReads>,
}

/// Whether every sheet the project walk from `root` and `pages` reaches is
/// open, so assembling the project reads no file.
fn project_in_memory(app: &Signex, pages: &[PathBuf], root: &Path, fold: CaseFold) -> bool {
    let engines = &app.document_state.engines;
    let mut queue: Vec<PathBuf> = pages.to_vec();
    queue.push(root.to_path_buf());
    let mut visited: HashSet<String> = HashSet::new();
    while let Some(path) = queue.pop() {
        if !visited.insert(path_key_with(&path, fold)) {
            continue;
        }
        let Some(engine) = engines.get(&path) else {
            return false;
        };
        let dir = path.parent().unwrap_or_else(|| Path::new(""));
        queue.extend(
            engine
                .document()
                .child_sheets
                .iter()
                .filter_map(|c| resolve_child_reference_quiet(dir, &c.filename).ok()),
        );
    }
    true
}

impl Signex {
    /// The project of `sheet` for an edit's check: the prefetched read with
    /// the open sheets in their live state, else, when every sheet is open,
    /// assembled from memory. Never read here.
    pub(super) fn edit_project_set(&self, sheet: &Path) -> Result<ProjectSheetSet, String> {
        let docs = &self.document_state;
        let project = docs
            .document_project(sheet)
            .ok_or("the sheet is in no project")?;
        if let Some(read) = docs
            .agent_edits
            .reads
            .as_ref()
            .and_then(|r| r.project.as_ref())
        {
            let mut set = read.clone();
            for (path, kept) in &mut set.sheets {
                if let Some(engine) = docs.engines.get(path) {
                    *kept = engine.document().clone();
                }
            }
            return Ok(set);
        }
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let (pages, root) = (
            project_pages(Some(project)),
            resolve_netlist_root_for(docs, sheet),
        );
        if project_in_memory(self, &pages, &root, fold) {
            return Ok(assemble_project_sheets_quiet(docs, &pages, &root, fold));
        }
        // `run_operands` reads the project and runs the command again.
        docs.agent_edits.project_wanted.set(true);
        Err(NOT_READ.to_string())
    }

    /// The prefetched read of `key`.
    pub(super) fn prefetched_part(&self, key: &PartKey) -> Result<ResolvedPlacement, String> {
        let reads = self.document_state.agent_edits.reads.as_ref();
        match reads.and_then(|r| r.parts.get(key)) {
            Some(read) => read.clone(),
            None => Err(NOT_READ.to_string()),
        }
    }

    /// The sheet at `path` for an edit: open, else the prefetched file.
    pub(super) fn prefetched_sheet(&self, path: &Path) -> Result<Option<SchematicSheet>, String> {
        if let Some(engine) = self.document_state.engines.get(path) {
            return Ok(Some(engine.document().clone()));
        }
        let reads = self.document_state.agent_edits.reads.as_ref();
        match reads.and_then(|r| r.children.get(path)) {
            Some(read) => read.clone(),
            None => Err(NOT_READ.to_string()),
        }
    }

    /// Whether `path` leaves its project folder through a link, as read.
    pub(super) fn prefetched_escape(&self, path: &Path) -> Result<bool, String> {
        let reads = self.document_state.agent_edits.reads.as_ref();
        reads
            .and_then(|r| r.escapes.get(path).copied())
            .ok_or_else(|| NOT_READ.to_string())
    }

    /// Start the reads `command` needs whatever its checks find (a library
    /// row, a child file, a link check) and run it when they land; `None` when
    /// it needs none (or is already running on its reads) and runs now. A
    /// command that waits anyway reads the project too when it has closed
    /// sheets, so an edit list is read once.
    pub(super) fn defer_for_reads(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<Ran> {
        let (target, plan) = self.read_plan(command, args, false)?;
        Some(self.start_reads(command, args, &target, plan))
    }

    /// The command's checks asked for the project's closed sheets, which only
    /// a check knows it needs (a net that reaches another sheet): start the
    /// reads, the project's included, and run it again when they land.
    pub(super) fn defer_for_project(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<Ran> {
        let (target, plan) = self.read_plan(command, args, true)?;
        Some(self.start_reads(command, args, &target, plan))
    }

    fn read_plan(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        wants_project: bool,
    ) -> Option<(PathBuf, ReadPlan)> {
        if self.document_state.agent_edits.reads.is_some() {
            return None;
        }
        let target = match args.operand("document") {
            Some(_) => named_sheet(self, args, "the edit").ok()?,
            None => self.document_state.edit_path()?.to_path_buf(),
        };
        let edits = if command.as_str() == "apply_edits" {
            super::agent_edits::listed_edits(self, args)?
        } else {
            vec![(command.clone(), args.clone())]
        };
        let mut plan = ReadPlan::default();
        let previous = self
            .document_state
            .agent_edits
            .target
            .replace(target.clone());
        for (id, edit) in &edits {
            self.plan_reads(id.as_str(), edit, &edits, &target, &mut plan);
        }
        self.document_state.agent_edits.target = previous;
        if wants_project || !plan.is_empty() {
            self.plan_project_read(&target, &mut plan);
        }
        (!plan.is_empty()).then_some((target, plan))
    }

    /// Add the project of `target` to `plan` when it, or a file the list's
    /// sheet symbols refer to, has a closed sheet.
    fn plan_project_read(&self, target: &Path, plan: &mut ReadPlan) {
        let docs = &self.document_state;
        let Some(project) = docs.document_project(target) else {
            plan.seeds.clear();
            return;
        };
        let fold = crate::app::state::project_case::fold_of(Some(project));
        let (pages, root) = (
            project_pages(Some(project)),
            resolve_netlist_root_for(docs, target),
        );
        let page_keys: HashSet<String> = pages.iter().map(|p| path_key_with(p, fold)).collect();
        plan.seeds
            .retain(|seed| !page_keys.contains(&path_key_with(seed, fold)));
        let walked: Vec<PathBuf> = pages.iter().chain(&plan.seeds).cloned().collect();
        if !project_in_memory(self, &walked, &root, fold) {
            let open = open_sheets_snapshot(docs);
            plan.project = Some(ProjectRead {
                open,
                pages,
                root,
                fold,
            });
        }
    }

    /// The revision of `target` and of each open sheet of its project: what
    /// an edit's reads depend on.
    pub(super) fn watched_revisions(&self, target: &Path) -> BTreeMap<PathBuf, u64> {
        let docs = &self.document_state;
        let project = docs.document_project(target).map(|p| p.id);
        docs.engines
            .iter()
            .filter(|(path, _)| {
                path.as_path() == target
                    || (project.is_some() && docs.document_project(path).map(|p| p.id) == project)
            })
            .map(|(path, engine)| (path.clone(), engine.revision()))
            .collect()
    }

    fn start_reads(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        target: &Path,
        plan: ReadPlan,
    ) -> Ran {
        let revisions = self.watched_revisions(target);
        let edits = &mut self.document_state.agent_edits;
        edits.tickets += 1;
        let ticket = edits.tickets;
        let request = ReadsLoaded {
            ticket,
            command: command.clone(),
            args: args.clone(),
            target: target.to_path_buf(),
            revisions,
            origin: self.agent.origin.clone(),
            serving: self.agent.serving,
            reads: Arc::new(EditReads::default()),
        };
        let task = Task::perform(
            async move { landed(tokio::task::spawn_blocking(move || plan.read()).await) },
            move |reads| {
                let loaded = ReadsLoaded {
                    reads: Arc::new(reads),
                    ..request
                };
                Message::Agent(AgentMsg::EditReadsLoaded(Box::new(loaded)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// Add what edit `id` needs to `plan`; `list` is the whole edit list (an
    /// earlier edit may place the sheet symbol a later one syncs).
    fn plan_reads(
        &self,
        id: &str,
        args: &CommandArgs,
        list: &[(AppCommandId, CommandArgs)],
        target: &Path,
        plan: &mut ReadPlan,
    ) {
        let docs = &self.document_state;
        match id {
            "place_component" => {
                let named = text(args, "library").ok().zip(text(args, "part").ok());
                let Some((library, part)) = named else { return };
                let Ok(found) = self.open_library_named(&library) else {
                    return;
                };
                let Ok((table, row)) = super::part_operands::library_row(found, &part) else {
                    return;
                };
                plan.parts.push((found.root.clone(), table, row));
                plan.open_libraries = self
                    .library
                    .open_libraries
                    .iter()
                    .map(|lib| (lib.library_id, lib.root.clone()))
                    .collect();
            }
            "place_sheet_symbol" => {
                if let Ok((dir, path, _)) =
                    text(args, "file").and_then(|f| self.child_file_target(&f))
                {
                    plan.seeds.push(path.clone());
                    plan.links.push((dir, path));
                }
            }
            "sync_sheet_pins" => {
                let Ok(name) = text(args, "sheet") else {
                    return;
                };
                let Some(file) = self.planned_child_file(&name, list) else {
                    return;
                };
                let dir = target.parent().unwrap_or_else(|| Path::new(""));
                if let Ok(path) = resolve_child_reference_quiet(dir, &file)
                    && !docs.engines.contains_key(&path)
                {
                    plan.children.push(path);
                }
            }
            _ => {}
        }
    }

    /// The child file of the sheet symbol `name`: on the target sheet, else
    /// placed by a `place_sheet_symbol` of the same list.
    fn planned_child_file(
        &self,
        name: &str,
        list: &[(AppCommandId, CommandArgs)],
    ) -> Option<String> {
        let on_sheet = self.document_state.edit_engine().and_then(|engine| {
            let found = engine
                .document()
                .child_sheets
                .iter()
                .find(|c| c.name == name);
            found.map(|c| c.filename.clone())
        });
        on_sheet.or_else(|| {
            list.iter()
                .filter(|(id, _)| id.as_str() == "place_sheet_symbol")
                .find(|(_, args)| text(args, "name").is_ok_and(|n| n.trim() == name))
                .and_then(|(_, args)| text(args, "file").ok())
        })
    }

    /// Run the edit `loaded` waited for, on its reads, as it would have run;
    /// refused when a sheet it depends on changed meanwhile.
    pub(crate) fn run_with_reads(&mut self, loaded: ReadsLoaded) -> Ran {
        if let Some(why) = &loaded.reads.failed {
            crate::diagnostics::log_warning(format!(
                "Reading the files `{}` needs failed: {why}",
                loaded.command.as_str()
            ));
            return (
                CommandOutcome::disabled(format!(
                    "reading the files this edit needs failed ({why}); nothing was applied"
                )),
                Task::none(),
            );
        }
        // A revision that moved, or a project sheet opened or closed.
        if self.watched_revisions(&loaded.target) != loaded.revisions {
            return (
                CommandOutcome::disabled(
                    "the document changed while its project and files were read; retry. Nothing was applied",
                ),
                Task::none(),
            );
        }
        self.document_state.agent_edits.reads = Some(loaded.reads);
        let ran = self.with_origin(loaded.origin, |app| {
            let serving = std::mem::replace(&mut app.agent.serving, loaded.serving);
            let ran = app.run_operands(&loaded.command, &loaded.args);
            app.agent.serving = serving;
            ran
        });
        self.document_state.agent_edits.reads = None;
        ran
    }

    /// A deferred edit run to its end: `task` is driven here (a headless run
    /// or a test) and the outcome taken by `ticket`.
    pub(crate) fn settle_deferred(&mut self, ticket: u64, task: Task<Message>) -> Ran {
        if let Err(error) = crate::task_driver::TaskDriver::new().settle(self, task) {
            return (
                CommandOutcome::disabled(format!("the edit did not finish: {error}")),
                Task::none(),
            );
        }
        let outcome = self
            .take_finished_edit(ticket)
            .unwrap_or_else(|| CommandOutcome::disabled("the edit did not finish"));
        (outcome, Task::none())
    }
}

impl Signex {
    /// [`Signex::run_operands`] driven to its outcome, its reads included: the
    /// tests' way to run an edit as a headless run reports it.
    #[cfg(test)]
    pub(crate) fn run_operands_settled(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Ran {
        match self.run_operands(command, args) {
            (CommandOutcome::Deferred { ticket }, task) => self.settle_deferred(ticket, task),
            ran => ran,
        }
    }
}

/// The refusal of a check whose read was not planned: never read in `update`.
const NOT_READ: &str = "a file this edit's check needs was not read first; retry";
