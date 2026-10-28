//! Keeps the Navigator panel's sheet hierarchy current.
//!
//! `refresh_panel_ctx` runs after nearly every update, and assembling the
//! project's sheets reads the closed ones from disk. So the hierarchy is only
//! rebuilt when what it depends on — the project's pages and root, the open
//! tabs, and the child-sheet references of the open sheets — has changed, and
//! the rebuild runs off the UI thread (#1470): the refresh only records the
//! request on `NavigatorJob`, `Signex::update` starts the assembly, and the
//! answer is swapped in when it lands. The previous hierarchy shows until
//! then, and an answer superseded by a newer request is dropped.
//! Everything cheaper (active-row highlight, open markers) is re-derived each
//! time from the cached hierarchy.
//!
//! Known limit: a *closed* child sheet that changes on disk (renamed, deleted,
//! corrupted) is not noticed until a tab opens or closes, because stat-ing
//! every sheet on each refresh would cost more than the staleness does.

use std::path::PathBuf;

use iced::Task;

use super::super::*;
use crate::app::project_sheets::{
    assemble_project_sheets_from, open_sheets_snapshot, project_root_sheet_path,
};
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::app::state::navigator_job::{NavigatorAssembly, NavigatorRequest};
use crate::app::state::scope::CaseFold;
use crate::panels::NavigatorModel;

impl Signex {
    /// The Navigator model for the current state: `previous`'s cached
    /// hierarchy, with a fresh assembly requested when what it was built from
    /// has changed. Never reads the disk.
    pub(super) fn next_navigator_model(&mut self, mut previous: NavigatorModel) -> NavigatorModel {
        let Some((root, pages, fold)) = self.navigator_project_scope() else {
            self.document_state.navigator_job.want(None);
            return NavigatorModel::default();
        };

        let open: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let active = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());

        let fingerprint = self.navigator_fingerprint(&root, &pages, fold);
        let stale = previous.fingerprint != fingerprint || previous.hierarchy.is_none();
        self.document_state
            .navigator_job
            .want(stale.then_some(NavigatorRequest {
                fingerprint,
                root,
                pages,
                fold,
            }));
        previous.rebuild_tree(active.as_deref(), &open);
        previous
    }

    /// Start the sheet assembly `refresh_panel_ctx` asked for, unless the
    /// wanted one is on screen or already running.
    pub(crate) fn start_navigator_assembly(&mut self) -> Option<Task<Message>> {
        let applied = self.document_state.panel_ctx.navigator.fingerprint.clone();
        let (generation, request) = self.document_state.navigator_job.begin(&applied)?;
        let open = open_sheets_snapshot(&self.document_state);
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || assemble_navigator(&open, request))
                    .await
                    .map_err(|error| error.to_string())
            },
            move |assembly| {
                Message::Project(ProjectMsg::NavigatorAssembled(
                    generation,
                    assembly.map(Box::new),
                ))
            },
        ))
    }

    /// Run the wanted assembly inline and apply it, as the Task would. For
    /// tests, which read the hierarchy straight after a message; production
    /// never calls it, so `update` stays free of disk reads.
    #[doc(hidden)]
    pub fn settle_navigator(&mut self) {
        self.document_state.navigator_job.abandon_running();
        while let Some((generation, request)) = {
            let applied = self.document_state.panel_ctx.navigator.fingerprint.clone();
            self.document_state.navigator_job.begin(&applied)
        } {
            let open = open_sheets_snapshot(&self.document_state);
            let assembly = assemble_navigator(&open, request);
            // Applied inline it is the current assembly, so it starts nothing;
            // the next wanted one is this loop's next turn.
            let follow_up = self.handle_navigator_assembled(generation, Ok(Box::new(assembly)));
            debug_assert_eq!(follow_up.units(), 0, "a settled assembly starts no task");
        }
    }

    /// Whether the Navigator's hierarchy is the current one and nothing is
    /// rebuilding it. While it is not, it may describe an earlier state of the
    /// project, so anything that WRITES on the strength of it ("this sheet is
    /// placed once") must treat the walk as incomplete.
    pub(crate) fn navigator_is_settled(&self) -> bool {
        self.document_state
            .navigator_job
            .is_settled(&self.document_state.panel_ctx.navigator.fingerprint)
    }

    /// An assembly landed: swap it in if it is still what the Navigator
    /// wants, then start the latest wanted one if it moved on meanwhile.
    pub(crate) fn handle_navigator_assembled(
        &mut self,
        generation: u64,
        assembly: Result<Box<NavigatorAssembly>, String>,
    ) -> Task<Message> {
        let fingerprint = assembly.as_ref().ok().map(|a| a.fingerprint.clone());
        let current = self.document_state.navigator_job.finish(
            generation,
            fingerprint.as_deref(),
            assembly.is_ok(),
        );
        let assembly = match assembly {
            Ok(assembly) if current => *assembly,
            Ok(_) => return self.start_navigator_assembly().unwrap_or_else(Task::none),
            Err(reason) => {
                crate::diagnostics::log_warning(format!(
                    "Navigator: the sheet hierarchy could not be built ({reason});\
                     it keeps its previous state"
                ));
                return Task::none();
            }
        };
        // Said once per rebuild, not per refresh: the reason a sheet that
        // exists could not be read would otherwise only be a bare marker.
        for (path, why) in &assembly.unreadable {
            crate::diagnostics::log_warning(format!(
                "Navigator: sheet {} is unreadable: {why}",
                path.display()
            ));
        }
        let open: Vec<PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let active = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());
        let navigator = &mut self.document_state.panel_ctx.navigator;
        navigator.replace_hierarchy(assembly.hierarchy, active.as_deref(), &open);
        navigator.fingerprint = assembly.fingerprint;
        // The canvas designators are a function of the hierarchy's placements.
        self.refresh_canvas_designators();
        Task::none()
    }

    /// The project the Navigator describes, as `(root sheet, declared pages,
    /// the project's key rules)`.
    ///
    /// The project owning the focused document, else the workspace's active
    /// project. Ownership alone is not enough: it is discovered through the
    /// *open* sheets' child references, so a leaf opened while its ancestors
    /// are closed resolves to no project — and the Navigator would go empty
    /// exactly when the user has navigated deep.
    pub(crate) fn navigator_project_scope(&self) -> Option<(PathBuf, Vec<PathBuf>, CaseFold)> {
        let project = self
            .document_state
            .active_document_project()
            .or_else(|| self.document_state.active_loaded_project())?;
        let root = project_root_sheet_path(project)?;
        let dir = project.dir();
        let pages = project
            .data
            .sheets
            .iter()
            .map(|s| dir.join(&s.filename))
            .collect();
        Some((root, pages, project.case_fold))
    }

    fn navigator_fingerprint(
        &self,
        root: &std::path::Path,
        pages: &[PathBuf],
        fold: CaseFold,
    ) -> String {
        let mut lines = vec![format!("root {}", root.display()), format!("fold {fold:?}")];
        lines.extend(pages.iter().map(|p| format!("page {}", p.display())));
        lines.extend(
            self.document_state
                .tabs
                .iter()
                .map(|t| format!("tab {}", t.path.display())),
        );
        let mut engines: Vec<_> = self.document_state.engines.iter().collect();
        engines.sort_by(|a, b| a.0.cmp(b.0));
        for (path, engine) in engines {
            lines.push(format!("sheet {}", path.display()));
            lines.extend(
                engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|c| format!("  ref {}|{}|{}", c.name, c.filename, c.uuid)),
            );
        }
        lines.join("\n")
    }
}

/// The blocking half of the assembly: read the closed sheets and build the
/// occurrence tree. Runs on a worker thread.
fn assemble_navigator(
    open: &crate::app::project_sheets::OpenSheets,
    request: NavigatorRequest,
) -> NavigatorAssembly {
    let set = assemble_project_sheets_from(open, &request.pages, &request.root, request.fold);
    let hierarchy = build_sheet_hierarchy(
        &set.sheets,
        &set.unreadable,
        &request.root,
        &set.pages_outside_the_hierarchy,
        request.fold,
    );
    NavigatorAssembly {
        fingerprint: request.fingerprint,
        hierarchy,
        unreadable: set.unreadable,
    }
}

#[cfg(test)]
mod tests {
    use signex_types::project::{ProjectData, SheetEntry};

    use super::*;
    use crate::app::sheet_hierarchy::SheetHierarchy;
    use crate::app::state::LoadedProject;
    use crate::app::state::navigator_job::NavigatorAssembly;

    fn app_with_project() -> Signex {
        let (mut app, _task) = Signex::new();
        let id = app.document_state.mint_project_id();
        let dir = std::path::Path::new("/nowhere/1470");
        app.document_state.projects.push(LoadedProject {
            id,
            path: dir.join("P.snxprj"),
            data: ProjectData {
                description: None,
                name: "P".to_string(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: Some("S.snxsch".to_string()),
                pcb_file: None,
                sheets: vec![SheetEntry {
                    name: "S".to_string(),
                    filename: "S.snxsch".to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
        app.document_state.active_project = Some(id);
        app
    }

    fn assembly(fingerprint: &str) -> Box<NavigatorAssembly> {
        Box::new(NavigatorAssembly {
            fingerprint: fingerprint.to_string(),
            hierarchy: SheetHierarchy::default(),
            unreadable: Vec::new(),
        })
    }

    fn wanted(app: &Signex) -> String {
        app.document_state
            .navigator_job
            .wanted_fingerprint()
            .expect("a project is in view")
            .to_string()
    }

    fn add_page(app: &mut Signex, filename: &str) {
        app.document_state.projects[0].data.sheets.push(SheetEntry {
            name: filename.to_string(),
            filename: filename.to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
    }

    /// The refresh asks for an assembly and shows the previous hierarchy;
    /// the answer is applied when it lands and not re-requested.
    #[test]
    fn the_refresh_requests_and_the_task_result_is_applied() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        assert!(
            app.document_state.panel_ctx.navigator.hierarchy.is_none(),
            "the refresh itself assembles nothing"
        );
        assert!(!app.navigator_is_settled());
        assert!(
            app.start_navigator_assembly().is_some(),
            "assembly requested"
        );
        assert!(
            app.start_navigator_assembly().is_none(),
            "not requested twice"
        );

        let generation = app.document_state.navigator_job.latest_generation();
        let fp = wanted(&app);
        let _ = app.handle_navigator_assembled(generation, Ok(assembly(&fp)));
        let navigator = &app.document_state.panel_ctx.navigator;
        assert!(navigator.hierarchy.is_some());
        assert_eq!(navigator.fingerprint, fp);
        assert!(app.navigator_is_settled());
    }

    /// An answer for a request the project has moved past is dropped, and the
    /// latest request starts as soon as it lands.
    #[test]
    fn a_superseded_result_is_dropped_and_the_latest_starts() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_some());
        let (stale, stale_fp) = (
            app.document_state.navigator_job.latest_generation(),
            wanted(&app),
        );
        add_page(&mut app, "T.snxsch");
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_none(), "one at a time");

        let next = app.handle_navigator_assembled(stale, Ok(assembly(&stale_fp)));
        assert_eq!(
            next.units(),
            1,
            "the stale answer starts the latest request"
        );
        assert!(
            app.document_state.panel_ctx.navigator.hierarchy.is_none(),
            "the stale answer must not land"
        );
        assert_eq!(
            app.document_state.navigator_job.latest_generation(),
            stale + 1,
            "the latest request started when the stale one landed"
        );
        let latest_fp = wanted(&app);
        let _ = app.handle_navigator_assembled(stale + 1, Ok(assembly(&latest_fp)));
        assert_eq!(
            app.document_state.panel_ctx.navigator.fingerprint,
            latest_fp
        );
    }

    /// The project returns to the state already on screen while an assembly
    /// for the intermediate state runs (an undo): that answer must not land.
    #[test]
    fn an_answer_for_a_state_the_user_undid_is_dropped() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        app.settle_navigator();
        let on_screen = app.document_state.panel_ctx.navigator.fingerprint.clone();
        assert!(!on_screen.is_empty());

        add_page(&mut app, "T.snxsch");
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_some());
        let (generation, intermediate) = (
            app.document_state.navigator_job.latest_generation(),
            wanted(&app),
        );
        app.document_state.projects[0].data.sheets.pop();
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_none());
        assert!(app.navigator_is_settled());

        let _ = app.handle_navigator_assembled(generation, Ok(assembly(&intermediate)));
        assert_eq!(
            app.document_state.panel_ctx.navigator.fingerprint,
            on_screen
        );
    }

    /// The project closes while its assembly runs: nothing may be installed.
    #[test]
    fn an_answer_for_a_closed_project_is_dropped() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_some());
        let (generation, fp) = (
            app.document_state.navigator_job.latest_generation(),
            wanted(&app),
        );
        app.document_state.projects.clear();
        app.document_state.active_project = None;
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_none());

        let _ = app.handle_navigator_assembled(generation, Ok(assembly(&fp)));
        assert!(app.document_state.panel_ctx.navigator.hierarchy.is_none());
        assert!(app.navigator_is_settled());
    }

    /// Between the request and the settle, the hierarchy on screen may
    /// predate the edit: a designator edit must take the entry branch, not
    /// write `reference` on the strength of "placed once".
    #[test]
    fn a_pending_hierarchy_is_not_trusted_to_be_a_single_placement() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        app.settle_navigator();
        assert!(app.navigator_is_settled());
        let path = std::path::Path::new("/nowhere/1470/S.snxsch");

        add_page(&mut app, "T.snxsch");
        app.refresh_panel_ctx();
        assert!(!app.navigator_is_settled());
        assert!(!app.known_single_placement(path));
    }
}
