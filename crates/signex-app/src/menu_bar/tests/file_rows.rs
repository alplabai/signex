//! What the File menu's rows do.

use super::*;

/// A schematic app whose document is `dir/s.snxsch`, marked dirty.
fn dirty_app_in(dir: &std::path::Path) -> (Signex, std::path::PathBuf) {
    let path = dir.join("s.snxsch");
    let mut app = app_with_at(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]), path.clone());
    app.document_state.mark_document_dirty(&path);
    (app, path)
}

#[test]
fn new_project_and_open_ask_for_a_file_and_move_nothing() {
    for path in [
        "File/New Project",
        "File/Open...",
        "File/Import/Altium Project...",
    ] {
        let mut app = one_symbol_selected();
        let before = probe(&app);
        let task = click(&mut app, path);
        assert!(has_work(&task), "{path} opened no dialog");
        assert_eq!(probe(&app), before, "{path}");
    }
}

#[test]
fn open_recent_drops_a_vanished_project_and_says_so() {
    let mut app = one_symbol_selected();
    app.ui_state.recent_projects.clear();
    app.ui_state
        .recent_projects
        .record(std::path::Path::new("/w/gone-menu-row.snxprj"));
    let before = probe(&app);
    assert_eq!(before.recent, 1);
    let _ = click(&mut app, "File/Open Recent/gone-menu-row.snxprj");
    assert_eq!(
        probe(&app),
        Probe {
            recent: 0,
            error_notice: true,
            ..before
        }
    );
}

#[test]
fn clear_recent_empties_the_list() {
    let mut app = one_symbol_selected();
    app.ui_state.recent_projects.clear();
    app.ui_state
        .recent_projects
        .record(std::path::Path::new("/w/a.snxprj"));
    let before = probe(&app);
    let _ = click(&mut app, "File/Open Recent/Clear Recent");
    assert_eq!(
        probe(&app),
        Probe {
            recent: 0,
            ..before
        }
    );
    assert!(!row_is_live(&app, "File/Open Recent/Clear Recent"));
}

#[test]
fn close_document_closes_the_active_tab() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "File/Close Document");
    let after = probe(&app);
    assert_eq!(after.tabs, 0, "the tab stays open");
    assert_eq!(
        after,
        Probe {
            tabs: 0,
            symbols: 0,
            selected: 0,
            refs: Vec::new(),
            ..before
        }
    );
}

#[test]
fn close_project_closes_the_active_project() {
    let mut app = one_symbol_selected();
    open_project(&mut app, "P.snxprj");
    let before = probe(&app);
    assert_eq!(before.projects, 1);
    let task = click(&mut app, "Project/Close Project");
    crate::app::path_ops::settle_task(&mut app, task);
    assert_eq!(probe(&app).projects, 0, "the project stays open");
}

#[test]
fn project_options_opens_the_project_tab_and_is_greyed_without_a_project() {
    assert!(!row_is_live(&bare_app(), "Project/Project Options..."));
    let mut app = one_symbol_selected();
    open_project(&mut app, "P.snxprj");
    // The fixture sheet lives outside the project folder; with no active path
    // the command falls back to the active project, as it does for a project tab.
    app.document_state.active_path = None;
    let tabs = app.document_state.tabs.len();
    let _ = click(&mut app, "Project/Project Options...");
    assert_eq!(app.document_state.tabs.len(), tabs + 1, "no Project tab");
}

#[test]
fn save_writes_the_active_schematic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = dirty_app_in(dir.path());
    assert!(!path.exists());
    let task = click(&mut app, "File/Save");
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(path.exists(), "the schematic was not written");
    assert!(!probe(&app).dirty, "the document is still dirty");
}

#[test]
fn save_as_asks_where_to_save_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = dirty_app_in(dir.path());
    let before = probe(&app);
    let task = click(&mut app, "File/Save As...");
    assert!(has_work(&task), "no dialog");
    assert!(!path.exists());
    assert_eq!(probe(&app), before);
}

#[test]
fn save_all_writes_every_dirty_document() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = dirty_app_in(dir.path());
    let task = click(&mut app, "File/Save All");
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(path.exists(), "the schematic was not written");
    assert!(!probe(&app).dirty);
}

#[test]
fn library_rows_open_the_library_dialog_and_the_part_picker() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let task = click(&mut app, "File/Library/Open Library...");
    assert!(has_work(&task), "no dialog");
    assert_eq!(probe(&app), before);

    let _ = click(&mut app, "File/Library/Place Component...");
    assert_eq!(
        probe(&app),
        Probe {
            picker: true,
            ..before
        }
    );
}

#[test]
fn print_and_export_pdf_open_the_print_preview() {
    for path in ["File/Print...", "File/Export/PDF…"] {
        let mut app = one_symbol_selected();
        let before = probe(&app);
        let _ = click(&mut app, path);
        assert_eq!(
            probe(&app),
            Probe {
                print_preview: true,
                ..before
            },
            "{path}"
        );
    }
}

#[test]
fn export_netlist_asks_where_to_save() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let task = click(&mut app, "Reports/Netlist (Protel .NET)...");
    assert!(has_work(&task), "no dialog");
    assert_eq!(probe(&app), before);
}

#[test]
fn export_bom_opens_the_bom_preview() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Reports/Bill of Materials…");
    assert_eq!(
        probe(&app),
        Probe {
            bom_preview: true,
            ..before
        }
    );
}

#[test]
fn export_erc_report_before_a_run_says_to_run_erc_first() {
    let _ = crate::diagnostics::init_logging();
    let needle = "Cannot export the ERC report: ERC has not been run";
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let logged_before = crate::diagnostics::logged_count(needle);
    let task = click(&mut app, "Reports/ERC Report…");
    assert!(!has_work(&task));
    assert_eq!(
        probe(&app),
        Probe {
            error_notice: true,
            ..before
        }
    );
    assert!(
        crate::diagnostics::logged_count(needle) > logged_before,
        "the refusal was not logged"
    );
}

#[test]
fn exit_asks_to_confirm_while_something_is_unsaved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _path) = dirty_app_in(dir.path());
    let before = probe(&app);
    let _ = click(&mut app, "File/Exit");
    assert_eq!(
        probe(&app),
        Probe {
            quit_confirm: true,
            ..before
        }
    );
}

#[test]
fn exit_with_nothing_unsaved_closes_the_window() {
    let mut app = one_symbol_selected();
    let task = click(&mut app, "File/Exit");
    assert!(has_work(&task), "no window close was requested");
}

/// #2224: a system quit request (Command+Q, Dock Quit, logout) takes the same
/// unsaved-changes guard as File ▸ Exit.
#[test]
fn a_system_quit_request_asks_to_confirm_while_something_is_unsaved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _path) = dirty_app_in(dir.path());
    let before = probe(&app);
    let needle = "Quit: the system asked Signex to quit";
    let logged_before = crate::diagnostics::logged_count(needle);
    let _ = app.update(Message::Window(crate::app::WindowMsg::QuitRequested));
    assert_eq!(
        probe(&app),
        Probe {
            quit_confirm: true,
            ..before
        }
    );
    assert!(
        crate::diagnostics::logged_count(needle) > logged_before,
        "the request was not logged"
    );
}

#[test]
fn a_system_quit_request_with_nothing_unsaved_closes_the_window() {
    let mut app = one_symbol_selected();
    let task = app.update(Message::Window(crate::app::WindowMsg::QuitRequested));
    assert!(has_work(&task), "no window close was requested");
}

/// The prompt's Cancel answers the pending system request with a cancel, and
/// does not exit: macOS waits (and logout stalls) until it is answered.
#[test]
fn cancelling_the_quit_prompt_answers_the_system_request() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _path) = dirty_app_in(dir.path());
    let _ = app.update(Message::Window(crate::app::WindowMsg::QuitRequested));
    assert!(app.ui_state.app_quit_confirm.is_some());

    let answer = app.handle_app_quit_confirm(crate::app::ProjectCloseChoice::Cancel);

    assert!(app.ui_state.app_quit_confirm.is_none());
    crate::task_driver::assert_quit_declined(answer);
}

/// A quit held back by a running rename or delete leaves the app open, so the
/// pending system request is declined too.
#[test]
fn a_held_quit_request_is_declined() {
    let _ = crate::diagnostics::init_logging();
    let mut app = one_symbol_selected();
    app.document_state.pending_path_ops.begin(
        crate::app::path_ops::PathOpKind::Delete,
        vec![std::path::PathBuf::from("/w/x.snxsch")],
        crate::app::CaseFold::PRESERVE,
    );
    let answer = app.handle_app_quit_requested();
    crate::task_driver::assert_quit_declined(answer);
}

#[test]
fn rows_without_a_document_are_greyed() {
    let app = bare_app();
    for path in [
        "File/Close Document",
        "Project/Close Project",
        "File/Save",
        "File/Save As...",
        "File/Save All",
        "File/Library/Place Component...",
        "File/Export/PDF…",
        "Reports/Netlist (Protel .NET)...",
        "Reports/Bill of Materials…",
        "Reports/ERC Report…",
        "File/Print...",
    ] {
        assert!(!row_is_live(&app, path), "{path} is live with no document");
    }
    for path in [
        "File/New Project",
        "File/Open...",
        "File/Library/Open Library...",
        "File/Import/Altium Project...",
        "File/Exit",
    ] {
        assert!(row_is_live(&app, path), "{path}");
    }
}
