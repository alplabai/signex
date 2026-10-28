//! Off-thread refresh of the project tree's filesystem facts (#111).

use iced::Task;

use crate::app::state::project_fs::{ProjectFsFacts, probe};
use crate::app::{Message, ProjectMsg, Signex};

impl Signex {
    /// Start a probe when the tree's inputs changed (`force` false) or the
    /// poll ticked (`force` true), unless one is already running.
    pub(crate) fn start_project_fs_probe(&mut self, force: bool) -> Option<Task<Message>> {
        let (generation, request) = self.document_state.project_fs.begin_probe(force)?;
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || probe(&request))
                    .await
                    .map_err(|error| error.to_string())
            },
            move |facts| {
                Message::Project(ProjectMsg::FsFactsProbed(generation, facts.map(Box::new)))
            },
        ))
    }

    /// A probe landed: swap the facts in and rebuild the tree if they differ.
    pub(crate) fn handle_project_fs_probed(
        &mut self,
        generation: u64,
        facts: Result<Box<ProjectFsFacts>, String>,
    ) -> Task<Message> {
        let finish = self
            .document_state
            .project_fs
            .finish_probe(generation, facts.map(|f| *f));
        if let Some(reason) = finish.warning {
            crate::diagnostics::log_warning(format!(
                "Project tree: the file-status check failed ({reason}); \
                 (missing) marks may be out of date until it succeeds"
            ));
        }
        if finish.changed {
            self.refresh_panel_ctx();
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use signex_types::project::{ProjectData, SheetEntry};

    use super::*;
    use crate::app::state::LoadedProject;

    fn app_with_sheet(dir: &Path) -> Signex {
        let (mut app, _task) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: dir.join("P.snxprj"),
            data: ProjectData {
                description: None,
                name: "P".to_string(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: None,
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
        app
    }

    fn sheet_missing(app: &Signex) -> bool {
        app.document_state.panel_ctx.projects[0].sheets[0].missing
    }

    /// Run the probe the way the task would, then deliver its message.
    fn run_probe(app: &mut Signex, force: bool) {
        let (generation, request) = app
            .document_state
            .project_fs
            .begin_probe(force)
            .expect("a probe should start");
        let facts = Ok(Box::new(probe(&request)));
        let _ = app.handle_project_fs_probed(generation, facts);
    }

    #[test]
    fn missing_mark_follows_the_disk_only_when_a_probe_lands() {
        let tmp = tempfile::Builder::new()
            .prefix("snx-111")
            .tempdir()
            .unwrap();
        let sheet = tmp.path().join("S.snxsch");
        std::fs::write(&sheet, "x").unwrap();
        let mut app = app_with_sheet(tmp.path());

        app.refresh_panel_ctx();
        assert!(!sheet_missing(&app), "unprobed paths read as present");
        run_probe(&mut app, false);
        assert!(!sheet_missing(&app));

        std::fs::remove_file(&sheet).unwrap();
        app.refresh_panel_ctx();
        assert!(!sheet_missing(&app), "refresh_panel_ctx must not stat");
        // Nothing changed in the tree's inputs: only the poll tick re-probes.
        assert!(app.document_state.project_fs.begin_probe(false).is_none());
        run_probe(&mut app, true);
        assert!(sheet_missing(&app), "the probe result is shown");

        std::fs::write(&sheet, "x").unwrap();
        app.refresh_panel_ctx();
        assert!(
            sheet_missing(&app),
            "last known state stays until the probe"
        );
        run_probe(&mut app, true);
        assert!(!sheet_missing(&app));
    }

    #[test]
    fn a_save_completion_makes_the_next_update_probe() {
        let tmp = tempfile::Builder::new()
            .prefix("snx-111")
            .tempdir()
            .unwrap();
        let mut app = app_with_sheet(tmp.path());
        app.refresh_panel_ctx();
        run_probe(&mut app, false);
        assert!(!app.document_state.project_fs.probe_running());
        let done = crate::persist::DocumentSaveDone {
            path: tmp.path().join("S.snxsch"),
            revision: 0,
            net_color_revision: 0,
            save_as_from: None,
            bytes: std::sync::Arc::new(b"".to_vec()),
            sequence: 0,
            result: Ok(crate::persist::SaveOutcome::Written),
            recovery_hold: None,
        };
        let _ = app.update(Message::File(crate::app::FileMsg::DocumentSaved(done)));
        assert!(
            app.document_state.project_fs.probe_running(),
            "the update started a probe"
        );
    }

    #[test]
    fn a_changed_project_set_makes_the_next_update_probe() {
        let tmp = tempfile::Builder::new()
            .prefix("snx-111")
            .tempdir()
            .unwrap();
        let mut app = app_with_sheet(tmp.path());
        app.refresh_panel_ctx();
        assert!(app.document_state.project_fs.begin_probe(false).is_some());
        assert!(
            app.document_state.project_fs.begin_probe(true).is_none(),
            "no second probe while one is in flight"
        );
    }

    #[test]
    fn chatty_and_probe_messages_do_not_invalidate() {
        use crate::library::LibraryMessage;
        let tmp = tempfile::Builder::new()
            .prefix("snx-111")
            .tempdir()
            .unwrap();
        let mut app = app_with_sheet(tmp.path());
        app.refresh_panel_ctx();
        run_probe(&mut app, false);
        let _ = app.update(Message::Library(LibraryMessage::BrowserSearchChanged {
            library_path: tmp.path().to_path_buf(),
            value: "r".to_string(),
        }));
        let _ = app.update(Message::Library(LibraryMessage::ExternalEditsPollTicked));
        // Generation 0 is not in flight, so the cache drops it; this line
        // proves only that `FsFactsProbed` is off the allow-list, not the
        // stale-drop path.
        let _ = app.update(Message::Project(ProjectMsg::FsFactsProbed(
            0,
            Ok(Box::default()),
        )));
        assert!(
            app.document_state.project_fs.begin_probe(false).is_none(),
            "none of these touch the disk"
        );
    }

    #[test]
    fn a_dying_probe_worker_reaches_the_messages_panel() {
        let tmp = tempfile::Builder::new()
            .prefix("snx-1091")
            .tempdir()
            .unwrap();
        let mut app = app_with_sheet(tmp.path());
        app.refresh_panel_ctx();
        let reason = "probe-1091-unique-reason";
        let before = crate::diagnostics::logged_count(reason);
        let (generation, _) = app
            .document_state
            .project_fs
            .begin_probe(true)
            .expect("a probe should start");

        let _ = app.update(Message::Project(ProjectMsg::FsFactsProbed(
            generation,
            Err(reason.to_string()),
        )));

        assert_eq!(
            crate::diagnostics::logged_count(reason),
            before + 1,
            "one Messages entry"
        );
    }

    #[test]
    fn a_stale_probe_failure_is_not_reported() {
        let tmp = tempfile::Builder::new()
            .prefix("snx-1091")
            .tempdir()
            .unwrap();
        let mut app = app_with_sheet(tmp.path());
        let reason = "probe-1091-stale-unique-reason";
        let before = crate::diagnostics::logged_count(reason);

        let _ = app.update(Message::Project(ProjectMsg::FsFactsProbed(
            0,
            Err(reason.to_string()),
        )));

        assert_eq!(
            crate::diagnostics::logged_count(reason),
            before,
            "a probe nobody is waiting on stays quiet"
        );
    }
}
