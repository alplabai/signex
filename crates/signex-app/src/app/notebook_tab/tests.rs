//! #2795, mockup row 195: the notebook tab driven through `Signex::update`
//! and the command dispatch. One tab per project, the empty state writes
//! nothing, a template or an import starts an unsaved notebook, Save writes
//! it atomically, a closed tab keeps its unsaved text for Save All, and the
//! `notebook_*` commands answer with JSON.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_notebook::Template;

use super::{Load, NotebookMsg};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex, TabKind};
use crate::keymap::AppCommandId;

/// A notebook with a three-line header, a section and an error-free body.
pub(super) const NOTEBOOK: &str = "+++\nformat = \"snxnbk/1\"\n+++\n\
    #import \"@signex/notebook:0.1.0\": *\n\
    = Power stage\n\
    The divider #component(\"R7\") sets #net(\"+3V3\").\n";

/// A project in `dir`, no sheet open.
pub(super) fn app_in(dir: &Path) -> (Signex, ProjectId) {
    let _ = crate::diagnostics::init_logging();
    let app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    (app, id)
}

/// Send `msg` and run what it starts to the end.
pub(super) fn send(app: &mut Signex, msg: NotebookMsg) {
    let task = app.update(Message::from(msg));
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

pub(super) fn open(app: &mut Signex, id: ProjectId) {
    send(app, NotebookMsg::OpenRequested { project: Some(id) });
}

pub(super) fn notebook_file(dir: &Path) -> PathBuf {
    dir.join("notebook").join("design.snxnbk")
}

/// A project in `dir` whose notebook file holds `text`, its tab open.
pub(super) fn app_with_notebook(dir: &Path, text: &str) -> (Signex, ProjectId) {
    let path = notebook_file(dir);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(&path, text).expect("the notebook");
    let (mut app, id) = app_in(dir);
    open(&mut app, id);
    (app, id)
}

pub(super) fn body(app: &Signex, id: ProjectId) -> String {
    app.document_state.notebooks[&id]
        .text
        .as_ref()
        .expect("text")
        .body()
}

pub(super) fn is_dirty(app: &Signex, id: ProjectId) -> bool {
    let path = app.document_state.notebooks[&id].path.clone();
    app.document_state.is_document_dirty(&path)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = app.run_operands_settled(&command, &args);
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
    outcome
}

pub(super) fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

#[test]
fn a_project_without_a_notebook_opens_the_empty_state_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(tab.kind, TabKind::Notebook(held) if held == id));
    assert_eq!(tab.path, notebook_file(dir.path()));
    assert!(tab.title.ends_with("Design Notebook"), "{}", tab.title);
    assert_eq!(app.document_state.notebooks[&id].load, Load::Missing);
    assert!(!notebook_file(dir.path()).exists());
    assert!(!dir.path().join("notebook").exists(), "no folder either");
    assert!(!is_dirty(&app, id));
}

#[test]
fn opening_twice_focuses_the_one_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    open(&mut app, id);
    let tabs = app
        .document_state
        .tabs
        .iter()
        .filter(|tab| tab.kind.as_notebook() == Some(id))
        .count();
    assert_eq!(tabs, 1);
}

#[test]
fn an_existing_notebook_opens_its_body_below_its_header_and_compiles() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let tab = &app.document_state.notebooks[&id];
    assert_eq!(tab.load, Load::Open);
    assert_eq!(tab.header_lines, 3);
    let typed = body(&app, id);
    assert!(typed.starts_with("#import"), "no header in the editor");
    assert_eq!(tab.view, super::ViewMode::Split, "Split, as drawn");
    let last = tab.compile.last.as_ref().expect("a compile ran");
    assert!(last.status.compiled, "{:?}", last.status.problems);
    assert!(tab.compile.shown.is_some(), "a page is shown");
    let outline = &tab.text.as_ref().unwrap().outline;
    assert_eq!(outline.sections[0].title, "Power stage");
    assert!(!is_dirty(&app, id));
}

#[test]
fn a_template_starts_an_unsaved_notebook_that_save_writes() {
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
    assert_eq!(app.document_state.notebooks[&id].load, Load::Open);
    assert!(is_dirty(&app, id), "unsaved until Save");
    let path = notebook_file(dir.path());
    assert!(!path.exists(), "nothing written before Save");
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    let written = std::fs::read_to_string(notebook_file(dir.path())).expect("written");
    assert!(written.starts_with("+++\n"), "{written}");
    assert!(written.contains("snxnbk/1"), "{written}");
    assert!(written.ends_with(&body(&app, id)));
    assert!(!is_dirty(&app, id));
    assert!(!app.document_state.notebooks[&id].is_unsaved());
}

#[test]
fn a_typed_change_marks_the_file_unsaved_and_undoing_it_marks_it_saved() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let typed = |action| NotebookMsg::TextActed {
        project_id: id,
        action,
    };
    send(&mut app, typed(super::edit::typed('x')));
    assert!(is_dirty(&app, id));
    send(
        &mut app,
        typed(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Backspace,
        )),
    );
    assert!(!is_dirty(&app, id), "back to the file");
}

#[test]
fn an_unchanged_header_is_written_back_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let odd = NOTEBOOK.replace("format = \"snxnbk/1\"", "format   =   \"snxnbk/1\"");
    let (mut app, id) = app_with_notebook(dir.path(), &odd);
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: super::edit::typed('x'),
        },
    );
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    let written = std::fs::read_to_string(notebook_file(dir.path())).expect("written");
    let at = odd.find("#import").expect("the body");
    assert_eq!(written, format!("{}x{}", &odd[..at], &odd[at..]));
}

#[test]
fn a_closed_tab_keeps_its_unsaved_text_and_save_all_writes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: super::edit::typed('x'),
        },
    );
    let idx = app.notebook_tab_index(id).expect("the tab");
    let _close = app.close_tab_now(idx);
    assert!(app.notebook_tab_index(id).is_none());
    assert!(is_dirty(&app, id), "still unsaved, so the prompts see it");
    let path = notebook_file(dir.path());
    app.try_save_dirty_path(&path).expect("Save All writes it");
    assert!(std::fs::read_to_string(&path).unwrap().contains("x#import"));
    assert!(!is_dirty(&app, id));
    open(&mut app, id);
    let reopened = body(&app, id);
    assert!(reopened.starts_with("x#import"), "reopened with its text");
}

#[test]
fn discard_drops_the_text_its_mark_and_its_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: super::edit::typed('x'),
        },
    );
    let path = notebook_file(dir.path());
    let _discard = app.discard_document_draft(&path);
    assert!(!app.document_state.notebooks.contains_key(&id));
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(app.notebook_tab_index(id).is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), NOTEBOOK);
}

#[test]
fn a_notebook_a_newer_signex_wrote_takes_no_edit_and_is_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let newer = NOTEBOOK.replace("snxnbk/1", "snxnbk/1.1");
    let (mut app, id) = app_with_notebook(dir.path(), &newer);
    assert!(app.document_state.notebooks[&id].is_read_only());
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: super::edit::typed('x'),
        },
    );
    assert!(!body(&app, id).starts_with('x'));
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    let on_disk = std::fs::read_to_string(notebook_file(dir.path())).unwrap();
    assert_eq!(on_disk, newer);
}

#[test]
fn a_file_that_is_not_utf8_is_not_opened_for_editing() {
    let dir = tempfile::tempdir().unwrap();
    let path = notebook_file(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, [0xff, 0xfe, b'=', b' ', b'A']).unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    let tab = &app.document_state.notebooks[&id];
    assert!(matches!(tab.load, Load::Failed(_)), "{:?}", tab.load);
    assert!(tab.text.is_none());
}

#[test]
fn closing_the_project_forgets_its_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    app.forget_notebook(id);
    assert!(!app.document_state.notebooks.contains_key(&id));
}

#[test]
fn notebook_compile_answers_from_the_file_with_the_tab_closed() {
    let dir = tempfile::tempdir().unwrap();
    let path = notebook_file(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, NOTEBOOK).unwrap();
    let (mut app, _id) = app_in(dir.path());
    let value = answer(&mut app, "notebook_compile", &[]);
    assert_eq!(value["exists"], true);
    assert_eq!(value["compiled"], true, "{value}");
    assert_eq!(value["unsaved"], false);
    let pages = value["pages"].as_array().map_or(0, Vec::len);
    assert!(pages > 0, "{value}");
    assert_eq!(value["outline"]["sections"][0]["title"], "Power stage");
    assert_eq!(value["references"].as_array().map(Vec::len), Some(2));
}

#[test]
fn notebook_compile_reads_the_text_as_typed_and_reports_its_problems() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Paste(std::sync::Arc::new("#(1 + ".into())),
            ),
        },
    );
    let value = answer(&mut app, "notebook_compile", &[]);
    assert_eq!(value["unsaved"], true);
    assert_eq!(value["compiled"], false, "{value}");
    assert_eq!(value["problems"][0]["line"], 4, "the file's own line");
}

#[test]
fn notebook_compile_says_when_there_is_no_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _id) = app_in(dir.path());
    let value = answer(&mut app, "notebook_compile", &[]);
    assert_eq!(value["exists"], false);
}

#[test]
fn notebook_insert_writes_what_the_button_writes_and_answers_with_the_id() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let value = answer(&mut app, "notebook_insert", &["kind=calculation", "line=6"]);
    assert_eq!(value["id"], "CALC-1.1", "{value}");
    assert_eq!(value["line"], 6);
    assert!(body(&app, id).contains("#calculation(id: \"CALC-1.1\""));
    assert!(is_dirty(&app, id));
    let pin = answer(&mut app, "notebook_insert", &["kind=pin", "name=U3.12"]);
    assert_eq!(pin["inserted"], "#pin(\"U3\", \"12\")");
}

#[test]
fn notebook_insert_and_view_refuse_while_the_tab_is_not_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _id) = app_in(dir.path());
    for (command, operands) in [
        ("notebook_insert", vec!["kind=note"]),
        ("notebook_view", vec!["view=preview"]),
    ] {
        let outcome = run(&mut app, command, &operands);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("not open")),
            "{command}: {outcome:?}"
        );
    }
}

#[test]
fn notebook_insert_refuses_an_unknown_kind_and_a_pin_without_its_number() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _id) = app_with_notebook(dir.path(), NOTEBOOK);
    let refused = [
        vec!["kind=table"],
        vec!["kind=pin", "name=U3"],
        vec!["kind=net"],
    ];
    for operands in refused {
        let outcome = run(&mut app, "notebook_insert", &operands);
        let is_refused = matches!(outcome, CommandOutcome::Disabled { .. });
        assert!(is_refused, "{operands:?}: {outcome:?}");
    }
}

#[test]
fn notebook_view_picks_the_view_and_the_panel_and_answers_with_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let operands = ["view=preview", "panel=references"];
    let value = answer(&mut app, "notebook_view", &operands);
    assert_eq!(value["view"], "preview");
    assert_eq!(value["panel"], "references");
    let tab = &app.document_state.notebooks[&id];
    assert_eq!(tab.view, super::ViewMode::Preview);
    assert_eq!(tab.panel, super::SidePanel::References);
    let refused = run(&mut app, "notebook_view", &["view=wide"]);
    assert!(matches!(refused, CommandOutcome::Disabled { .. }));
}
