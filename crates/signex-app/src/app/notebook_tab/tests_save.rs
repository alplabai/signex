//! Saving the notebook: one write at a time, an edit made while a write
//! runs stays unsaved, the first Save of a new notebook never replaces a
//! file written meanwhile, and a CRLF file keeps its line ends.

use iced::Task;
use signex_notebook::Template;

use super::NotebookMsg;
use super::tests::{NOTEBOOK, app_in, app_with_notebook, is_dirty, notebook_file, open, send};
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

fn settle(app: &mut Signex, task: Task<Message>) {
    TaskDriver::new().settle(app, task).expect("settles");
}

fn typed(c: char, id: crate::app::state::ProjectId) -> NotebookMsg {
    NotebookMsg::TextActed {
        project_id: id,
        action: super::edit::typed(c),
    }
}

#[test]
fn an_edit_while_a_save_runs_stays_unsaved_and_a_second_write_waits() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(&mut app, typed('x', id));
    // Ctrl+S starts its write; it has not landed yet.
    let saving = app.update(Message::from(NotebookMsg::SavePressed { project_id: id }));
    send(&mut app, typed('y', id));
    let path = notebook_file(dir.path());
    let refused = app.save_notebook_blocking(&path);
    assert!(refused.is_err(), "no second write while one runs");
    settle(&mut app, saving);
    let written = std::fs::read_to_string(&path).expect("written");
    assert!(written.contains("x#import"), "{written}");
    assert!(!written.contains("xy"), "the later edit was not written");
    assert!(
        is_dirty(&app, id),
        "the edit made while the write ran is still unsaved"
    );
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    assert!(!is_dirty(&app, id));
    let written = std::fs::read_to_string(&path).expect("written");
    assert!(written.contains("xy#import"), "{written}");
}

#[test]
fn the_first_save_of_a_new_notebook_does_not_replace_a_file_written_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    send(
        &mut app,
        NotebookMsg::TemplatePicked {
            project_id: id,
            template: Template::DesignReport,
        },
    );
    let path = notebook_file(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "theirs").unwrap();
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theirs");
    assert!(is_dirty(&app, id), "still unsaved");
    let doc = &app.document_state.notebooks[&id];
    let why = doc.save_error.as_deref().unwrap_or_default();
    assert!(why.contains("design.snxnbk"), "{why}");
}

/// A design of the part `designator` with the id `n`.
fn design_of(designator: &str, n: u128) -> signex_notebook::DesignIndex {
    let part = signex_notebook::DesignPart {
        designator: designator.into(),
        id: uuid::Uuid::from_u128(n),
        value: String::new(),
        description: String::new(),
        footprint: String::new(),
        sheet: "top.snxsch".into(),
        fitted_in: Vec::new(),
        variant_count: 0,
        pins: Vec::new(),
    };
    signex_notebook::DesignIndex::new(vec![part], Vec::new())
}

/// Mockup row 196 (the 2026-11-25 rename row): Save stores the id the
/// notebook read found for each reference, so the part renamed later,
/// outside Annotate, reads as renamed rather than deleted.
#[test]
fn save_stores_the_reference_ids_so_a_later_rename_reads_as_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let project = &app.document_state.projects[0];
    let target = (project.id, project.dir().to_path_buf());
    let state = &mut app.ui_state.annotations;
    state.target = Some(target);
    let scan = crate::panels::annotations::NotebookScan {
        folder_found: true,
        design: Some(std::sync::Arc::new(design_of("R7", 7))),
        ..Default::default()
    };
    state.scan = Some(std::sync::Arc::new(scan));
    send(&mut app, typed('x', id));
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    let written = std::fs::read_to_string(notebook_file(dir.path())).expect("written");
    let file = signex_notebook::NotebookFile::parse(&written).expect("parses");
    let r7 = signex_notebook::Reference::Component("R7".into());
    assert_eq!(file.header.id_of(&r7), Some(uuid::Uuid::from_u128(7)));
    let renamed = design_of("R17", 7);
    let report = signex_notebook::resolve_references(
        &signex_notebook::references(&written),
        &file.header,
        &renamed,
    );
    assert!(
        matches!(
            report.get(&r7).expect("R7").state,
            signex_notebook::ReferenceState::Renamed { .. }
        ),
        "{report:?}"
    );
}

#[test]
fn a_crlf_notebook_keeps_its_line_ends_after_an_edit() {
    let dir = tempfile::tempdir().unwrap();
    let crlf = NOTEBOOK.replace('\n', "\r\n");
    let (mut app, id) = app_with_notebook(dir.path(), &crlf);
    send(&mut app, typed('x', id));
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    let written = std::fs::read_to_string(notebook_file(dir.path())).expect("written");
    assert!(written.contains("x#import"), "{written:?}");
    assert!(
        !written.replace("\r\n", "").contains('\n'),
        "no bare LF: {written:?}"
    );
    let lines = written.matches("\r\n").count();
    assert_eq!(lines, crlf.matches("\r\n").count());
}
