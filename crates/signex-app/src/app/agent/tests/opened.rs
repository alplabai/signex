//! #1924: `save_document`, `close_active_document` and `assign_net_class` run
//! on the agent channel with an explicit `document`, and the person's
//! interaction state is the same before and after.

use std::path::Path;

use serde_json::json;
use signex_types::net_class::NetClass;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::{app_at_work, app_in, ask_with_task, call, line, path, revision, snapshot};
use crate::app::Signex;
use crate::app::path_ops::settle_task;

fn doc(dir: &Path, file: &str) -> String {
    dir.join(file).display().to_string()
}

/// The person's snapshot without what a close or a save changes by design: the
/// tab strip and its active index, and the command palette's catalog (it lists
/// Save by whether a document is dirty). Active path, focus, selection, tool,
/// the move epoch and the rest stay.
fn person_without_tabs(app: &Signex) -> String {
    snapshot(app)
        .lines()
        .filter(|line| {
            ["tabs=", "active_tab=", "palette="]
                .iter()
                .all(|skipped| !line.starts_with(skipped))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn junction(app: &mut Signex, file: &Path) {
    let started = app.document_state.engines[file].revision();
    let applied = call(
        app,
        "apply_edits",
        json!({
            "document": file.display().to_string(),
            "expect_revision": started,
            "edits": [{"id": "place_junction", "args": {"at": [20, 20]}}],
        }),
    );
    assert_eq!(applied["status"], "answered", "{applied}");
}

/// The person at work on `b`: a move armed (with its pick on the app and the
/// canvas), an inline text edit open and a shape anchor placed. Each of
/// them is dropped by a tab sync.
fn mid_interaction() -> Signex {
    let mut app = app_at_work();
    arm(&mut app);
    app
}

fn arm(app: &mut Signex) {
    app.ui_state.move_pick = Some(crate::canvas::MovePick::Object);
    app.interaction_state.active_canvas_mut().move_pick = Some(crate::canvas::MovePick::Object);
    app.interaction_state.shape_anchor = Some(signex_types::schematic::Point::new(3.0, 4.0));
    app.interaction_state.editing_text = Some(crate::app::TextEditState {
        uuid: super::uid(super::WIRE_A),
        kind: SelectedKind::Label,
        text: "half typed".into(),
        original_text: "x".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    });
}

fn close_a(app: &mut Signex) -> serde_json::Value {
    call(
        app,
        "close_active_document",
        json!({"document": path("a.snxsch").display().to_string()}),
    )
}

#[test]
fn closing_a_background_tab_keeps_the_persons_move_pick_and_epoch() {
    let mut app = mid_interaction();
    let epoch = app.interaction_state.active_canvas().move_epoch;
    assert_eq!(close_a(&mut app)["status"], "ran");
    assert!(app.ui_state.move_pick.is_some(), "the app's move pick");
    assert!(app.interaction_state.active_canvas().move_pick.is_some());
    assert_eq!(app.interaction_state.active_canvas().move_epoch, epoch);
}

#[test]
fn closing_a_background_tab_keeps_the_persons_open_text_edit() {
    let mut app = mid_interaction();
    assert_eq!(close_a(&mut app)["status"], "ran");
    let edit = app.interaction_state.editing_text.as_ref();
    assert_eq!(edit.map(|e| e.text.as_str()), Some("half typed"));
}

#[test]
fn closing_a_background_tab_keeps_the_persons_shape_anchor() {
    let mut app = mid_interaction();
    assert_eq!(close_a(&mut app)["status"], "ran");
    assert!(app.interaction_state.shape_anchor.is_some());
}

#[test]
fn closing_another_dirty_sheet_parks_it_with_its_edits_and_the_person_is_as_they_were() {
    let mut app = app_at_work();
    junction(&mut app, &path("a.snxsch"));
    let a = path("a.snxsch");
    let edited = revision(&app, "a.snxsch");
    assert!(app.document_state.is_document_dirty(&a));
    let tabs = app.document_state.tabs.len();
    arm(&mut app);
    let person = person_without_tabs(&app);

    let closed = call(
        &mut app,
        "close_active_document",
        json!({"document": a.display().to_string()}),
    );

    assert_eq!(closed["status"], "ran", "{closed}");
    // No sheet is reloaded, so no load-time warning is logged again.
    assert_eq!(closed["messages"], json!([]), "{closed}");
    assert_eq!(app.document_state.tabs.len(), tabs - 1);
    assert!(app.document_state.tabs.iter().all(|tab| tab.path != a));
    // Parked, not discarded: the engine and its edits stay, still unsaved.
    assert_eq!(revision(&app, "a.snxsch"), edited, "the edits are kept");
    assert!(app.document_state.is_document_dirty(&a), "still unsaved");
    assert_eq!(person_without_tabs(&app), person);
    assert_eq!(
        app.document_state.tabs[app.document_state.active_tab].path,
        path("b.snxsch"),
        "the person's tab is still the active one"
    );
}

#[test]
fn save_writes_the_named_document_only_and_the_person_is_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_in(dir.path());
    junction(&mut app, &dir.path().join("a.snxsch"));
    junction(&mut app, &dir.path().join("b.snxsch"));
    let person = person_without_tabs(&app);

    let text = line(
        &app,
        "save_document",
        json!({"document": doc(dir.path(), "a.snxsch")}),
    );
    let (reply, task) = ask_with_task(&mut app, super::Input::Line(text));
    assert_eq!(reply["status"], "ran", "{reply}");
    settle_task(&mut app, task);

    let mut written: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    assert_eq!(written, ["a.snxsch"], "the named document and nothing else");
    assert!(
        !app.document_state
            .is_document_dirty(&dir.path().join("a.snxsch"))
    );
    assert!(
        app.document_state
            .is_document_dirty(&dir.path().join("b.snxsch")),
        "the person's other sheet is still unsaved"
    );
    assert_eq!(person_without_tabs(&app), person);
}

#[test]
fn assigning_a_net_class_changes_it_and_the_persons_undo_is_in_order() {
    let mut app = app_at_work();
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    app.settle_net_class_styles_blocking();
    // The person's own edit on the sheet they are on.
    let symbol = app
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .uuid;
    let x = |app: &Signex| {
        app.document_state
            .active_engine()
            .unwrap()
            .document()
            .symbols[0]
            .position
            .x
    };
    let start = x(&app);
    assert!(app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(symbol, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    ));
    let moved = x(&app);
    let person = snapshot(&app);

    let reply = call(
        &mut app,
        "assign_net_class",
        json!({
            "document": path("b.snxsch").display().to_string(),
            "net_name": "VCC",
            "class": "Power",
        }),
    );

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(
        app.document_state.projects[0].data.net_classes[0].nets,
        ["VCC"]
    );
    assert_eq!(snapshot(&app), person);
    // The pick is the newest step on the timeline; the person's own edit is
    // still behind it.
    app.handle_undo_requested();
    assert!(
        app.document_state.projects[0].data.net_classes[0]
            .nets
            .is_empty()
    );
    assert_eq!(x(&app), moved, "the person's edit stays");
    app.handle_undo_requested();
    assert_eq!(x(&app), start, "then the person's own step");
}

#[test]
fn assigning_a_net_class_on_a_background_sheet_leaves_the_person_as_they_were() {
    let mut app = app_at_work();
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    app.settle_net_class_styles_blocking();
    let person = snapshot(&app);

    let reply = call(
        &mut app,
        "assign_net_class",
        json!({
            "document": path("a.snxsch").display().to_string(),
            "net_name": "VCC",
            "class": "Power",
        }),
    );

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(
        app.document_state.projects[0].data.net_classes[0].nets,
        ["VCC"]
    );
    assert_eq!(snapshot(&app), person);
}
