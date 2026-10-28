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

    /// Run the wanted assembly inline and apply it. Tests only: production
    /// hands the same work to a Task so `update` never reads the disk; tests
    /// that read the hierarchy straight after a message need it settled.
    #[cfg(test)]
    pub(crate) fn settle_navigator(&mut self) {
        let applied = self.document_state.panel_ctx.navigator.fingerprint.clone();
        if let Some((generation, request)) = self.document_state.navigator_job.begin(&applied) {
            let open = open_sheets_snapshot(&self.document_state);
            let assembly = assemble_navigator(&open, request);
            self.handle_navigator_assembled(generation, Ok(Box::new(assembly)));
        }
    }

    /// An assembly landed: swap it in unless a newer request superseded it.
    pub(crate) fn handle_navigator_assembled(
        &mut self,
        generation: u64,
        assembly: Result<Box<NavigatorAssembly>, String>,
    ) {
        if !self
            .document_state
            .navigator_job
            .finish(generation, assembly.is_ok())
        {
            return;
        }
        let assembly = match assembly {
            Ok(assembly) => *assembly,
            Err(reason) => {
                crate::diagnostics::log_warning(format!(
                    "Navigator: the sheet hierarchy could not be built ({reason});                      it keeps its previous state"
                ));
                return;
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
        assert!(
            app.start_navigator_assembly().is_some(),
            "assembly requested"
        );
        assert!(
            app.start_navigator_assembly().is_none(),
            "not requested twice"
        );

        let generation = app.document_state.navigator_job.latest_generation();
        app.handle_navigator_assembled(generation, Ok(assembly("fp-1")));
        let navigator = &app.document_state.panel_ctx.navigator;
        assert!(navigator.hierarchy.is_some());
        assert_eq!(navigator.fingerprint, "fp-1");
    }

    /// An answer superseded by a newer request is dropped.
    #[test]
    fn a_superseded_result_is_dropped() {
        let mut app = app_with_project();
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_some());
        let stale = app.document_state.navigator_job.latest_generation();
        // The project changes under the running assembly: a newer request.
        app.document_state.projects[0].data.sheets.push(SheetEntry {
            name: "T".to_string(),
            filename: "T.snxsch".to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
        app.refresh_panel_ctx();
        assert!(app.start_navigator_assembly().is_some(), "newer request");

        app.handle_navigator_assembled(stale, Ok(assembly("stale")));
        assert!(
            app.document_state.panel_ctx.navigator.hierarchy.is_none(),
            "the stale answer must not land"
        );
        let latest = app.document_state.navigator_job.latest_generation();
        app.handle_navigator_assembled(latest, Ok(assembly("fresh")));
        assert_eq!(app.document_state.panel_ctx.navigator.fingerprint, "fresh");
    }
}
