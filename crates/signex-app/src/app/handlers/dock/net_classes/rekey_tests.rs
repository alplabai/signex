//! #661: undo history that is read through a sheet's path (net colours and
//! net-class picks) follows the sheet through a rename and a Save As.

use std::path::{Path, PathBuf};

use signex_types::net_class::NetClass;
use signex_types::theme::Color;

use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver::{colour_of, paint};
use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::path_ops::settle_task;
use crate::panels::NetClassMsg;

const RED: Color = Color {
    r: 255,
    g: 0,
    b: 0,
    a: 255,
};

/// `root.snxsch` on disk and open, with one wire and a project class `Power`.
fn fixture(dir: &Path) -> (Signex, PathBuf) {
    let root = dir.join("root.snxsch");
    let text = signex_types::format::SnxSchematic::new(sheet_with_net("R1", "N", &[]))
        .write_string()
        .expect("encode");
    std::fs::write(&root, text).expect("write sheet");
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    app.open_or_focus_sheet_path(root.clone());
    (app, root)
}

/// Colour the sheet's wire the way a net flood does.
fn flood_red(app: &mut Signex, path: &Path) -> uuid::Uuid {
    let wire = app.document_state.engines[path].document().wires[0].uuid;
    paint(app, &[wire], RED);
    wire
}

fn pick_power(app: &mut Signex) {
    let project = app.document_state.projects[0].id;
    app.handle_net_class_message(&NetClassMsg::AssignNet {
        project,
        net: "GND".to_string(),
        class: Some("Power".to_string()),
    });
    assert_eq!(nets(app), ["GND"]);
}

fn nets(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.net_classes[0]
        .nets
        .clone()
}

fn save_as(app: &mut Signex, from: &Path, to: PathBuf) {
    let task = app
        .save_active_document_as(from, to)
        .expect("Save As starts");
    settle_task(app, task);
}

#[test]
fn net_colour_undo_survives_a_rename() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = fixture(dir.path());
    let wire = flood_red(&mut app, &root);

    app.rekey_document(&root, &dir.path().join("renamed.snxsch"));
    app.handle_undo_requested();

    assert!(
        !colour_of(&app, wire).is_some(),
        "Undo restores the colour from before the flood"
    );
}

#[test]
fn net_colour_redo_survives_a_rename() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = fixture(dir.path());
    let wire = flood_red(&mut app, &root);
    app.handle_undo_requested();
    assert!(!colour_of(&app, wire).is_some());

    app.rekey_document(&root, &dir.path().join("renamed.snxsch"));
    app.handle_redo_requested();

    assert_eq!(colour_of(&app, wire).as_ref(), Some(&RED));
}

#[test]
fn net_class_pick_undo_survives_a_rename() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = fixture(dir.path());
    pick_power(&mut app);

    app.rekey_document(&root, &dir.path().join("renamed.snxsch"));
    app.handle_undo_requested();

    assert!(nets(&app).is_empty(), "Undo reverts the pick");
}

#[test]
fn net_colour_undo_survives_save_as() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = fixture(dir.path());
    let wire = flood_red(&mut app, &root);

    save_as(&mut app, &root, dir.path().join("copy.snxsch"));
    app.handle_undo_requested();

    assert!(!colour_of(&app, wire).is_some());
}

#[test]
fn net_class_pick_undo_survives_save_as() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root) = fixture(dir.path());
    pick_power(&mut app);

    save_as(&mut app, &root, dir.path().join("copy.snxsch"));
    app.handle_undo_requested();

    assert!(nets(&app).is_empty(), "Undo reverts the pick");
}
