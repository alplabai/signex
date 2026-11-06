//! #2029 — Save, Save All and autosave write the measured width of every
//! text whose face is installed into `[extras.sheet.text_extents]`.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_app::autosave::{AutosaveMsg, sidecar};
use signex_app::menu_bar::MenuMessage;
use signex_types::schematic::{HAlign, Label, LabelType, Point, SchematicSheet, VAlign};
use tempfile::TempDir;
use uuid::Uuid;

const TABLE: &str = "[extras.sheet.text_extents.";

fn net_label() -> Label {
    Label {
        uuid: Uuid::from_u128(0x2029),
        text: "NET_A".into(),
        position: Point { x: 1.0, y: 2.0 },
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn label_key() -> String {
    format!("{}:label", Uuid::from_u128(0x2029))
}

/// An app with a dirty schematic holding one default-face label, open at `path`.
fn app_with_label_sheet(path: &Path) -> Signex {
    // The shaper the running app installs at startup; without it no width is final.
    signex_app::fonts::measure::install_blocking();
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        labels: vec![net_label()],
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    let mut engine = signex_engine::Engine::new(sheet).unwrap();
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "label".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    app.document_state.mark_document_dirty(path);
    app
}

fn assert_has_label_width(text: &str) {
    assert!(text.contains(TABLE), "no text_extents table:\n{text}");
    assert!(
        text.contains(&label_key()),
        "no record for the label:\n{text}"
    );
}

#[test]
fn save_writes_the_width_of_an_installed_face() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("one.snxsch");
    let mut app = app_with_label_sheet(&path);

    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));

    assert_has_label_width(&std::fs::read_to_string(&path).expect("Save writes the file"));
}

#[test]
fn save_all_writes_the_width_of_an_installed_face() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    let project = tmp.path().join("All.snxprj");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::NewProject(Some(project))));
    let sheet_path = tmp.path().join("one.snxsch");
    let mut with_sheet = app_with_label_sheet(&sheet_path);
    let engine = with_sheet
        .document_state
        .engines
        .remove(&sheet_path)
        .unwrap();
    app.document_state
        .engines
        .insert(sheet_path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "one".into(),
        path: sheet_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.mark_document_dirty(&sheet_path);

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert_has_label_width(&std::fs::read_to_string(&sheet_path).expect("Save All writes it"));
}

#[test]
fn autosave_writes_the_width_of_an_installed_face() {
    let tmp = TempDir::new().unwrap();
    let path: PathBuf = tmp.path().join("auto.snxsch");
    let recovery = tmp.path().join("recovery");
    let mut app = app_with_label_sheet(&path);
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    app.ui_state.autosave_enabled = true;

    crate::settle_support::settle(&mut app, Message::Autosave(AutosaveMsg::Ticked));

    let copy = sidecar::sidecar_path(&recovery, &path);
    assert_has_label_width(&std::fs::read_to_string(copy).expect("a recovery copy is written"));
}
