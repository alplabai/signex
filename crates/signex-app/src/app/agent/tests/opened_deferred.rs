//! #2549: `assign_net_class` on a project whose child sheet is closed reads it
//! first and runs again when the read lands. The person's undo order and
//! interaction state hold on that route too, a person edit made while the read
//! is in flight included.

use std::path::Path;

use serde_json::{Value, json};
use signex_types::net_class::NetClass;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::{SHEET_B, app_in, call_pending, call_settled, settle_pending, snapshot};
use crate::app::Signex;
use crate::app::command::queries::tests::uid;

/// The agent project in `dir`, the child `power.snxsch` its sheets place on
/// disk and closed, the class `Power` defined and the styles resolved.
fn app_with_closed_child(dir: &Path) -> Signex {
    let child = signex_types::schematic::SchematicSheet {
        uuid: uid(SHEET_B + 50),
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(child)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("power.snxsch"), text).expect("write the child");
    let mut app = app_in(dir);
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    app.settle_net_class_styles_blocking();
    app
}

fn pick(dir: &Path, file: &str) -> Value {
    json!({
        "document": dir.join(file).display().to_string(),
        "net_name": "VCC",
        "class": "Power",
    })
}

fn nets(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.net_classes[0]
        .nets
        .clone()
}

fn x(app: &Signex) -> f64 {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .position
        .x
}

/// The person moves the first symbol of the sheet they are on.
fn persons_move(app: &mut Signex) {
    let symbol = app
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .uuid;
    assert!(app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(symbol, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        },
        true,
        true,
    ));
}

/// A person edit while the read is in flight refuses the pick, which changes
/// nothing; the retry lands as the newest step, the person's two edits still
/// behind it in order, and the person is as they were.
#[test]
fn a_pick_after_the_read_keeps_the_persons_undo_in_order_across_an_edit_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_closed_child(dir.path());
    let start = x(&app);
    persons_move(&mut app);
    let first = x(&app);

    let pending = call_pending(&mut app, "assign_net_class", pick(dir.path(), "b.snxsch"));
    persons_move(&mut app);
    let second = x(&app);
    let refused = settle_pending(&mut app, pending);

    assert_eq!(refused["status"], "refused", "{refused}");
    assert_eq!(
        refused["reason"],
        "the document changed while its project and files were read; retry. Nothing was applied"
    );
    assert!(nets(&app).is_empty());
    let person = snapshot(&app);

    let reply = call_settled(&mut app, "assign_net_class", pick(dir.path(), "b.snxsch"));

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(nets(&app), ["VCC"]);
    assert_eq!(snapshot(&app), person);
    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the pick is the newest step");
    assert_eq!(x(&app), second, "the person's edits stay");
    app.handle_undo_requested();
    assert_eq!(x(&app), first, "then the edit made while the read ran");
    app.handle_undo_requested();
    assert_eq!(x(&app), start, "then the person's first edit");
}

/// [`snapshot`] without the open palette's `catalog_stamp`, the hash of the
/// inputs its rows are built from.
fn snapshot_but_the_stamp(app: &Signex) -> String {
    let stamp = app.ui_state.command_palette.catalog_stamp;
    snapshot(app).replace(&format!("catalog_stamp: {stamp}"), "catalog_stamp: _")
}

/// A pick on a background sheet that waited for the read leaves the person as
/// they were. Only the palette's stamp moves: the pick leaves the project
/// unsaved, the stamp hashes that enablement fact, and the settled messages
/// re-rank the open palette to the same rows.
#[test]
fn a_pick_after_the_read_on_a_background_sheet_leaves_the_person_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_closed_child(dir.path());
    let person = snapshot_but_the_stamp(&app);
    let stamp = app.ui_state.command_palette.catalog_stamp;
    assert_eq!(stamp, crate::app::command_palette::catalog_stamp(&app));
    let facts = app.command_context(None);
    assert!(!facts.has_dirty_documents);

    let reply = call_settled(&mut app, "assign_net_class", pick(dir.path(), "a.snxsch"));

    assert_eq!(reply["status"], "ran", "{reply}");
    assert_eq!(nets(&app), ["VCC"]);
    assert_eq!(snapshot_but_the_stamp(&app), person);
    assert_eq!(
        app.ui_state.command_palette.catalog_stamp,
        crate::app::command_palette::catalog_stamp(&app),
        "the stamp is the current catalog's"
    );
    // The one fact that moved: the project now has unsaved changes.
    let dirty = crate::app::command::CommandContext {
        has_dirty_documents: true,
        ..facts
    };
    assert_eq!(app.command_context(None), dirty);
}
