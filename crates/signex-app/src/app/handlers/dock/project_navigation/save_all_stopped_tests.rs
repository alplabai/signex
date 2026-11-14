//! #2269 — a Save All write that stops without a result (its task panics)
//! is failed on its own: its siblings still report, and every file and row
//! of the batch can be saved again.

use std::path::{Path, PathBuf};

use super::save_all::STOP_WRITE_OF;
use super::save_all_tests::{
    app_with_two_dirty_documents, feed, feed_then_settle, quit_settle, write_that_panics,
};
use crate::app::contracts::{SaveAllNext, SaveAllPlan, SaveAllWrite};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, Signex, TabInfo, TabKind};
use crate::library::state::EditorAddress;
use crate::persist::run_task::outputs;

/// The quit's Save All of `paths`, probe task included, with the write of
/// `stopped` panicking in place of its task; returns what the settle does.
fn quit_save_all_stopping(
    app: &mut Signex,
    paths: &[PathBuf],
    stopped: &Path,
) -> iced::Task<Message> {
    STOP_WRITE_OF.with(|stop| *stop.borrow_mut() = Some(stopped.to_path_buf()));
    let probe = app.begin_save_all("Quit", paths, &[], SaveAllNext::Quit);
    let mut messages = Vec::new();
    for message in outputs(probe) {
        messages.extend(outputs(app.update(message)));
    }
    assert!(
        STOP_WRITE_OF.with(|stop| stop.borrow().is_none()),
        "the stopped write was started"
    );
    feed_then_settle(app, messages)
}

/// The two dirty documents, the symbol also open in a tab so its editor
/// outlives a save; a lettered entry lets the probe answer from `dir`.
fn app_with_open_symbol(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    std::fs::write(dir.join("Probe.txt"), b"").expect("probe entry");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir);
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: symbol.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol.clone()),
    });
    (app, symbol, sheet)
}

/// Rename the editor's symbol, the way a canvas edit changes it.
fn rename_symbol(app: &mut Signex, path: &Path, name: &str) {
    let editor = app
        .document_state
        .symbol_editors
        .get_mut(path)
        .expect("editor");
    editor.primitive_mut().name = name.to_string();
    editor.mark_dirty();
}

fn names_on_disk(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).expect("read container");
    signex_library::SymbolFile::from_bytes(&bytes)
        .expect("parse container")
        .symbols
        .iter()
        .map(|symbol| symbol.name.clone())
        .collect()
}

fn notice(app: &Signex) -> String {
    format!("{:?}", app.document_state.error_notice.as_ref())
}

/// A panicking write next to a real primitive save: the quit is declined,
/// the primitive's own write still lands, and after the declined quit a
/// second save of either file writes it.
#[test]
fn a_write_that_panics_next_to_a_primitive_save_leaves_both_files_saveable() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_open_symbol(dir.path());

    let after = quit_save_all_stopping(&mut app, &[symbol.clone(), sheet.clone()], &sheet);

    crate::task_driver::assert_quit_declined(after);
    let notice = notice(&app);
    assert!(
        notice.contains("Sheet.snxsch") && notice.contains("stopped unexpectedly"),
        "{notice}"
    );
    assert!(!notice.contains("Lib.snxsym"), "the symbol saved: {notice}");
    assert_eq!(names_on_disk(&symbol), vec!["Sym1".to_string()]);
    assert!(!sheet.exists(), "the stopped write wrote nothing");
    let saves = &app.document_state.document_saves;
    assert!(!saves.is_saving(&symbol), "the symbol's write reported");
    assert!(!saves.is_saving(&sheet), "the stopped write is released");
    assert!(app.document_state.is_document_dirty(&sheet));

    rename_symbol(&mut app, &symbol, "Sym2");
    let task = app.save_primitive_tab_at(&symbol).expect("save starts");
    feed(&mut app, outputs(task));
    assert_eq!(names_on_disk(&symbol), vec!["Sym2".to_string()]);

    let task = app
        .begin_schematic_save(sheet.clone(), None)
        .expect("save starts");
    feed(&mut app, outputs(task));
    assert!(sheet.exists(), "the second save wrote the sheet");
    assert!(!app.document_state.has_dirty_documents());
}

/// The primitive's own write panicking: the draft stays unsaved and named,
/// and is not left marked saving, so the next save writes it instead of
/// queueing a follow-up that never runs.
#[test]
fn a_primitive_whose_save_all_write_panicked_saves_again() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_open_symbol(dir.path());

    let after = quit_save_all_stopping(&mut app, &[symbol.clone(), sheet.clone()], &symbol);

    crate::task_driver::assert_quit_declined(after);
    let notice = notice(&app);
    assert!(
        notice.contains("Lib.snxsym") && notice.contains("stopped unexpectedly"),
        "{notice}"
    );
    assert!(sheet.exists(), "the sheet's write still landed");
    assert!(!symbol.exists(), "the stopped write wrote nothing");
    assert!(app.document_state.is_document_dirty(&symbol));
    assert!(
        !app.document_state.document_saves.is_saving(&symbol),
        "the stopped write is released"
    );

    rename_symbol(&mut app, &symbol, "Sym2");
    let task = app.save_primitive_tab_at(&symbol).expect("save starts");
    feed(&mut app, outputs(task));
    assert_eq!(names_on_disk(&symbol), vec!["Sym2".to_string()]);
    assert!(!app.document_state.is_document_dirty(&symbol));
}

/// Only the row whose write stopped is dropped from the batch: a row of the
/// same batch still writing on a task of its own is left alone, and the
/// settle waits for it.
#[test]
fn a_stopped_row_write_leaves_the_batch_rows_still_writing_alone() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let row = |table: &str| {
        EditorAddress::new(
            dir.path().join("Parts.snxlib"),
            table.into(),
            signex_library::RowId::new(),
        )
    };
    let (stopped, running) = (row("resistors"), row("capacitors"));
    let batch = 41;
    let in_flight = &mut app.library.batch_rows_in_flight;
    in_flight.insert(7, (batch, stopped.clone()));
    in_flight.insert(8, (batch, running.clone()));

    let task = super::save_all::writes_then_settle(
        vec![(
            SaveAllWrite::Row(stopped.clone()),
            iced::Task::future(write_that_panics()),
        )],
        quit_settle(Vec::new(), vec![stopped, running.clone()], batch),
    );
    let after = feed_then_settle(&mut app, outputs(task));

    assert_eq!(after.units(), 0, "nothing quits or declines yet");
    assert_eq!(
        app.library.batch_rows_in_flight.get(&8),
        Some(&(batch, running)),
        "the running row is untouched"
    );
    assert!(!app.library.batch_rows_in_flight.contains_key(&7));
    assert!(
        app.document_state
            .parked_save_all_settles
            .contains_key(&batch),
        "the settle waits for the running row"
    );
    assert!(
        app.document_state.stopped_save_all_batches.contains(&batch),
        "a parked batch is kept until its settle finishes"
    );
    let parked = &app.document_state.parked_save_all_settles[&batch];
    assert!(
        parked
            .failed
            .iter()
            .any(|(label, reason)| label.contains("resistors")
                && reason.contains("stopped unexpectedly")),
        "{:?}",
        parked.failed
    );
}

/// A Ctrl+S of the `.snxprj` asked while the Save All's write of it runs is
/// queued behind that write; when the write stops, the queued save still
/// runs and writes what the project holds.
#[test]
fn a_project_save_queued_behind_a_stopped_save_all_write_still_runs() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(dir.path().to_str().expect("utf8"), &[]);
    let (id, prj) = {
        let project = &app.document_state.projects[0];
        (project.id, project.path.clone())
    };
    app.document_state.mark_document_dirty(&prj);

    STOP_WRITE_OF.with(|stop| *stop.borrow_mut() = Some(prj.clone()));
    let save_all = app.begin_save_all_planned(SaveAllPlan {
        context: "Save All".into(),
        paths: vec![prj.clone()],
        rows: Vec::new(),
        next: SaveAllNext::Nothing,
        probes: Default::default(),
        undecided: Vec::new(),
    });
    assert!(
        STOP_WRITE_OF.with(|stop| stop.borrow().is_none()),
        "the stopped write was started"
    );
    app.document_state.projects[0].data.description = Some("typed meanwhile".into());
    let queued = app.begin_project_save(id).expect("queued");
    assert_eq!(queued.units(), 0, "queued behind the running write");

    let after = feed_then_settle(&mut app, outputs(save_all));

    let messages = outputs(after);
    let writes = messages
        .iter()
        .filter(|m| matches!(m, Message::Project(ProjectMsg::FileSaved(_))))
        .count();
    assert_eq!(writes, 1, "the queued save runs once");
    feed(&mut app, messages);
    assert!(!app.document_state.is_document_dirty(&prj));
    assert!(!app.document_state.document_saves.is_saving(&prj));
    let on_disk = signex_types::project::parse_project(&prj).expect("project on disk");
    assert_eq!(on_disk.description.as_deref(), Some("typed meanwhile"));
}
