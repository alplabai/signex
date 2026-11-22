//! Row m165: the Annotations panel over a project's notebook files.

use std::fs;
use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::dock::DockMessage;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::annotations::{
    AnnotationKind, AnnotationsView, ListStatus, Reference, Status,
};
use signex_app::panels::{AnnotationsMsg, PanelMsg};
use signex_app::task_driver::TaskDriver;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use tempfile::TempDir;
use uuid::Uuid;

use super::support::{add_project, fresh_app, project_data};

const U1: u128 = 0xC165_01;
const U3: u128 = 0xC165_03;

/// Settle without the dock layout save, which would write the test run's
/// config folder.
fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([task]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 1_000, "the task chain settles");
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
}

fn panel(app: &mut Signex, msg: AnnotationsMsg) {
    settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::Annotations(msg))),
    );
}

fn view(app: &Signex) -> &AnnotationsView {
    &app.document_state.panel_ctx.annotations
}

fn write_sheet(dir: &Path, name: &str, id: u128, reference: &str) -> PathBuf {
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".into(),
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(id),
            lib_id: "Device:R".into(),
            reference: reference.into(),
            position: Point::new(10.0, 10.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = dir.join(name);
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    fs::write(&path, text).expect("write sheet");
    path
}

const NOTEBOOK: &str = "\
#annotation(kind: \"issue\", status: \"open\", date: \"2026-11-10\", title: \"U3 runs warm\")[Hot at #component(\"U3\").]\n\
\n\
#annotation(kind: \"measurement\", status: \"done\", date: \"2026-11-12\", title: \"Ripple\")[18 mV on #net(\"VOUT\") at #component(\"U1\").]\n\
\n\
#annotation(kind: \"note\", date: \"2026-11-01\", title: \"Rework\")[Removed #component(\"C7\").]\n\
\n\
#annotation(kind: \"todo\")[Bad kind]\n";

/// A project of `a.snxsch` (U1) and `b.snxsch` (U3), both open with `a` in
/// front.
fn project(dir: &Path, notebook: bool) -> (Signex, PathBuf, PathBuf) {
    let a = write_sheet(dir, "a.snxsch", U1, "U1");
    let b = write_sheet(dir, "b.snxsch", U3, "U3");
    if notebook {
        fs::create_dir_all(dir.join("notebook")).expect("notebook folder");
        fs::write(dir.join("notebook/bench.snxnbk"), NOTEBOOK).expect("write notebook");
    }
    let mut app = fresh_app();
    let data = project_data(dir, &["a.snxsch", "b.snxsch"], None);
    add_project(&mut app, dir.join("P.snxprj"), data);
    for path in [&b, &a] {
        let text = fs::read_to_string(path).expect("read");
        let sheet = SnxSchematic::parse(&text).expect("parse").sheet;
        let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
            path: path.clone(),
            title: "t".into(),
            result: Ok(Box::new(sheet)),
            disk: None,
        }));
    }
    (app, a, b)
}

fn opened() -> (TempDir, Signex, PathBuf, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, a, b) = project(tmp.path(), true);
    settle(&mut app, Message::Menu(MenuMessage::OpenAnnotationsPanel));
    (tmp, app, a, b)
}

fn titles(app: &Signex) -> Vec<String> {
    view(app)
        .groups
        .iter()
        .flat_map(|group| group.rows.iter().map(|row| row.title.clone()))
        .collect()
}

#[test]
fn the_panel_lists_the_notebook_blocks_of_a_file_newest_first() {
    let (_tmp, app, _a, _b) = opened();
    let v = view(&app);
    assert_eq!(v.status, ListStatus::Ready);
    assert_eq!(v.groups.len(), 1);
    assert_eq!(v.groups[0].label, "notebook/bench.snxnbk");
    assert_eq!(titles(&app), ["Ripple", "U3 runs warm", "Rework"]);
    assert_eq!((v.shown, v.total, v.files), (3, 3, 1));
}

#[test]
fn a_block_it_cannot_read_is_skipped_and_named_in_messages() {
    let (_tmp, app, _a, _b) = opened();
    assert_eq!(view(&app).total, 3, "the bad block is not listed");
    let lines: Vec<String> = app
        .document_state
        .panel_ctx
        .diagnostics
        .iter()
        .map(|entry| entry.message.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("notebook/bench.snxnbk") && line.contains("todo")),
        "{lines:?}"
    );
}

#[test]
fn a_kind_chip_hides_its_rows_and_the_counts_stay() {
    let (_tmp, mut app, _a, _b) = opened();
    panel(&mut app, AnnotationsMsg::KindToggled(AnnotationKind::Issue));
    assert_eq!(titles(&app), ["Ripple", "Rework"]);
    let chip = view(&app)
        .kinds
        .iter()
        .find(|chip| chip.kind == AnnotationKind::Issue)
        .expect("the Issue chip");
    assert!(!chip.on);
    assert_eq!(chip.count, 1, "the chip keeps its count");
    panel(&mut app, AnnotationsMsg::KindToggled(AnnotationKind::Issue));
    assert_eq!(view(&app).shown, 3);
}

#[test]
fn the_part_status_and_text_filters_narrow_the_list_and_clear() {
    let (_tmp, mut app, _a, _b) = opened();
    panel(&mut app, AnnotationsMsg::PartPicked("U3".into()));
    assert_eq!(titles(&app), ["U3 runs warm"]);
    panel(&mut app, AnnotationsMsg::PartCleared);
    assert_eq!(view(&app).shown, 3);

    panel(&mut app, AnnotationsMsg::StatusPicked(Status::Done));
    assert_eq!(titles(&app), ["Ripple"]);
    panel(&mut app, AnnotationsMsg::StatusCleared);

    panel(&mut app, AnnotationsMsg::QueryChanged("REWORK".into()));
    assert_eq!(titles(&app), ["Rework"]);
    panel(
        &mut app,
        AnnotationsMsg::QueryChanged("no such text".into()),
    );
    let v = view(&app);
    assert!(v.groups.is_empty());
    assert_eq!(v.total, 3, "the total stays");
}

#[test]
fn expanding_a_row_shows_its_typst_source_and_a_second_click_folds_it() {
    let (_tmp, mut app, _a, _b) = opened();
    let key = view(&app).groups[0].rows[0].key.clone();
    panel(&mut app, AnnotationsMsg::RowToggled(key.clone()));
    let row = &view(&app).groups[0].rows[0];
    assert!(row.expanded);
    assert!(row.source.starts_with("#annotation("), "{}", row.source);
    panel(&mut app, AnnotationsMsg::RowToggled(key));
    assert!(!view(&app).groups[0].rows[0].expanded);
}

#[test]
fn a_reference_to_a_deleted_part_is_marked_gone() {
    let (_tmp, app, _a, _b) = opened();
    let rework = &view(&app).groups[0].rows[2];
    let chips: Vec<(String, bool)> = rework
        .chips
        .iter()
        .map(|chip| (chip.label.clone(), chip.gone))
        .collect();
    assert_eq!(chips, [("C7".to_string(), true)]);
}

#[test]
fn a_part_chip_opens_its_sheet_and_selects_the_part() {
    let (_tmp, mut app, _a, b) = opened();
    panel(
        &mut app,
        AnnotationsMsg::ReferenceClicked(Reference::Component("U3".into())),
    );
    assert_eq!(app.document_state.active_path.as_deref(), Some(b.as_path()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![SelectedItem::new(Uuid::from_u128(U3), SelectedKind::Symbol)]
    );
}

#[test]
fn a_chip_of_a_deleted_part_goes_nowhere() {
    let (_tmp, mut app, a, _b) = opened();
    panel(
        &mut app,
        AnnotationsMsg::ReferenceClicked(Reference::Component("C7".into())),
    );
    assert_eq!(app.document_state.active_path.as_deref(), Some(a.as_path()));
    assert!(app.interaction_state.active_canvas().selected.is_empty());
}

#[test]
fn reading_and_filtering_marks_no_sheet_dirty() {
    let (_tmp, mut app, a, b) = opened();
    let dirty = |app: &Signex| {
        (
            app.document_state.is_document_dirty(&a),
            app.document_state.is_document_dirty(&b),
        )
    };
    let before = dirty(&app);
    panel(&mut app, AnnotationsMsg::KindToggled(AnnotationKind::Note));
    panel(&mut app, AnnotationsMsg::RefreshRequested);
    assert_eq!(dirty(&app), before);
}

#[test]
fn a_project_with_no_notebook_shows_the_empty_state_until_refresh_finds_one() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _a, _b) = project(tmp.path(), false);
    settle(&mut app, Message::Menu(MenuMessage::OpenAnnotationsPanel));
    assert_eq!(view(&app).status, ListStatus::NoNotebook);
    fs::create_dir_all(tmp.path().join("notebook")).expect("folder");
    fs::write(tmp.path().join("notebook/bench.snxnbk"), NOTEBOOK).expect("write");
    panel(&mut app, AnnotationsMsg::RefreshRequested);
    assert_eq!(view(&app).status, ListStatus::Ready);
    assert_eq!(view(&app).total, 3);
}
