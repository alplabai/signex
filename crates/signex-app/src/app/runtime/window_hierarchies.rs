//! The sheet hierarchy of a project an undocked window shows (#2021).
//!
//! The Navigator's hierarchy follows the focused document's project. A
//! window on a sheet of another project needs that project's own hierarchy
//! to know the sheet's placements, so one is assembled off-thread per such
//! project (see `state/window_hierarchies.rs`).

use std::path::Path;

use iced::Task;

use super::super::*;
use super::navigator_ctx::assemble_navigator;
use crate::app::project_sheets::{open_sheets_snapshot, project_root_sheet_path};
use crate::app::sheet_hierarchy::SheetHierarchy;
use crate::app::state::WindowKind;
use crate::app::state::navigator_job::{NavigatorAssembly, NavigatorRequest};

impl Signex {
    /// Record which projects' hierarchies the undocked windows need: the
    /// owners of their sheets, except the one the Navigator already holds.
    pub(super) fn want_window_hierarchies(&mut self) {
        let navigator_root = self.navigator_project_scope().map(|(root, ..)| root);
        let mut requests: Vec<NavigatorRequest> = Vec::new();
        let paths: Vec<_> = self
            .ui_state
            .windows
            .values()
            .filter_map(|kind| match kind {
                WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        for path in paths {
            let Some((root, pages, fold)) = self
                .document_state
                .document_project(&path)
                .and_then(Self::project_scope_of)
            else {
                continue;
            };
            if Some(&root) == navigator_root.as_ref() || requests.iter().any(|r| r.root == root) {
                continue;
            }
            let fingerprint = self.navigator_fingerprint(&root, &pages, fold);
            requests.push(NavigatorRequest {
                fingerprint,
                root,
                pages,
                fold,
            });
        }
        self.document_state.window_hierarchies.want(requests);
    }

    /// Start the assemblies `want_window_hierarchies` asked for.
    pub(crate) fn start_window_hierarchy_assemblies(&mut self) -> Vec<Task<Message>> {
        let started = self.document_state.window_hierarchies.begin_all();
        if started.is_empty() {
            return Vec::new();
        }
        let open = open_sheets_snapshot(&self.document_state);
        started
            .into_iter()
            .map(|(generation, request)| {
                let (open, root) = (open.clone(), request.root.clone());
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || assemble_navigator(&open, request))
                            .await
                            .map_err(|error| error.to_string())
                    },
                    move |assembly| {
                        Message::Project(ProjectMsg::WindowHierarchyAssembled(
                            root.clone(),
                            generation,
                            assembly.map(Box::new),
                        ))
                    },
                )
            })
            .collect()
    }

    /// An assembly landed: keep it if it is still wanted, then redraw the
    /// windows' designators from it.
    pub(crate) fn handle_window_hierarchy_assembled(
        &mut self,
        root: &Path,
        generation: u64,
        assembly: Result<Box<NavigatorAssembly>, String>,
    ) -> Task<Message> {
        if let Err(reason) = &assembly {
            crate::diagnostics::log_warning(format!(
                "Window: the sheet hierarchy of {} could not be built ({reason})",
                root.display()
            ));
        }
        let applied = self
            .document_state
            .window_hierarchies
            .finish(root, generation, assembly);
        if applied {
            self.refresh_window_designators();
        }
        Task::batch(self.start_window_hierarchy_assemblies())
    }

    /// Run the wanted window assemblies inline, as the Tasks would. For
    /// tests; production never calls it, so `update` stays free of disk reads.
    #[doc(hidden)]
    pub(super) fn settle_window_hierarchies(&mut self) {
        self.want_window_hierarchies();
        self.document_state.window_hierarchies.abandon_running();
        loop {
            let started = self.document_state.window_hierarchies.begin_all();
            if started.is_empty() {
                break;
            }
            let open = open_sheets_snapshot(&self.document_state);
            for (generation, request) in started {
                let root = request.root.clone();
                let assembly = assemble_navigator(&open, request);
                self.document_state.window_hierarchies.finish(
                    &root,
                    generation,
                    Ok(Box::new(assembly)),
                );
            }
        }
        self.refresh_window_designators();
    }

    /// The hierarchy that describes the project owning the sheet at `path`,
    /// and whether it is current (nothing is rebuilding it):
    /// - the Navigator's, for the focused sheet while it holds that sheet's
    ///   own project;
    /// - else the one assembled for the undocked windows;
    /// - for a sheet no project owns, the Navigator's when it is focused.
    pub(crate) fn hierarchy_describing(&self, path: &Path) -> Option<(&SheetHierarchy, bool)> {
        let ds = &self.document_state;
        let navigator = ds
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .filter(|_| ds.active_path.as_deref() == Some(path));
        let Some(owner_root) = ds.document_project(path).and_then(project_root_sheet_path) else {
            return navigator.map(|h| (h, self.navigator_is_settled()));
        };
        let navigator_is_owners = |h: &&SheetHierarchy| {
            h.root
                .as_ref()
                .is_none_or(|node| node.is_at(&owner_root, h.fold))
        };
        match navigator.filter(navigator_is_owners) {
            Some(h) => Some((h, self.navigator_is_settled())),
            None => ds
                .window_hierarchies
                .hierarchy_of(&owner_root)
                .map(|h| (h, ds.window_hierarchies.is_settled(&owner_root))),
        }
    }
}
