//! #510 - a font family set from the Properties panel goes through the
//! engine (undoable), is stored on the item as typed, and a family this
//! machine lacks is reported once per document in the Messages panel while
//! the stored name is left alone.

use signex_app::app::contracts::EditMsg;
use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::schematic::{HAlign, Label, LabelType, Point, SchematicSheet, TextNote, VAlign};
use std::path::{Path, PathBuf};

const MISSING: &str = "Surely Not Installed 510";

fn label(id: uuid::Uuid) -> Label {
    Label {
        uuid: id,
        text: "VIN".to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    }
}

fn note(id: uuid::Uuid) -> TextNote {
    TextNote {
        uuid: id,
        text: "note".to_string(),
        position: Point::new(40.0, 40.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn app_with(sheet: SchematicSheet, path: &Path) -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    // The scan is normally a startup Task; the warning needs its result.
    signex_app::fonts::catalog::installed_blocking();
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "fonts".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    app
}

fn set_family(app: &mut Signex, target: signex_engine::TextTarget, family: Option<&str>) {
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::EditTextFontFamily(target, family.map(str::to_string)),
    )));
}

/// The notices for `document`. The diagnostics buffer is process-wide, so
/// notices other tests in this binary raised for their own documents are
/// filtered out by file name.
fn missing_notices(app: &Signex, document: &str) -> Vec<String> {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .filter(|entry| {
            entry.message.contains(&format!("Font '{MISSING}'")) && entry.message.contains(document)
        })
        .map(|entry| entry.message.clone())
        .collect()
}

fn label_family(app: &Signex, path: &Path, id: uuid::Uuid) -> Option<String> {
    app.document_state.engines[path]
        .document()
        .labels
        .iter()
        .find(|label| label.uuid == id)
        .and_then(|label| label.font_family.clone())
}

#[test]
fn a_family_edit_is_stored_as_typed_and_undone_with_one_undo() {
    let id = uuid::Uuid::new_v4();
    let path = PathBuf::from("font-edit.snxsch");
    let sheet = SchematicSheet {
        labels: vec![label(id)],
        ..Default::default()
    };
    let mut app = app_with(sheet, &path);

    set_family(
        &mut app,
        signex_engine::TextTarget::Label(id),
        Some(MISSING),
    );
    assert_eq!(
        label_family(&app, &path, id).as_deref(),
        Some(MISSING),
        "a family this machine lacks is still what the document stores"
    );
    assert!(app.document_state.dirty_paths.contains(&path) || app.document_state.tabs[0].dirty);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(label_family(&app, &path, id), None);
}

#[test]
fn a_missing_family_is_reported_once_per_document() {
    let (first, second) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let path = PathBuf::from("font-notice.snxsch");
    let sheet = SchematicSheet {
        labels: vec![label(first), label(second)],
        text_notes: vec![note(uuid::Uuid::new_v4())],
        ..Default::default()
    };
    let mut app = app_with(sheet, &path);

    set_family(
        &mut app,
        signex_engine::TextTarget::Label(first),
        Some(MISSING),
    );
    let after_first = missing_notices(&app, "font-notice.snxsch");
    assert_eq!(after_first.len(), 1, "{after_first:?}");
    assert!(
        after_first[0].starts_with(&format!(
            "Font '{MISSING}' is not installed; using Iosevka for 1 text item in font-notice.snxsch"
        )),
        "{}",
        after_first[0]
    );

    // A second item naming the same family does not repeat the notice.
    set_family(
        &mut app,
        signex_engine::TextTarget::Label(second),
        Some(MISSING),
    );
    assert_eq!(missing_notices(&app, "font-notice.snxsch").len(), 1);
}

#[test]
fn the_sheet_default_family_is_set_through_the_engine_and_undone() {
    let path = PathBuf::from("font-default.snxsch");
    let mut app = app_with(SchematicSheet::default(), &path);

    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SetDocumentFontFamily(Some(MISSING.to_string())),
    )));
    assert_eq!(
        app.document_state.engines[&path]
            .document()
            .font_family
            .as_deref(),
        Some(MISSING)
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        app.document_state.engines[&path].document().font_family,
        None
    );
}
