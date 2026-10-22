//! #359 — "Sync Sheet Pins" reconciles the selected sheet symbol's pins with
//! the child schematic's hierarchical / global labels, through the context
//! menu action and the normal mutation path (undo, dirty tracking, netlist
//! invalidation).
//!
//! Real temp dir: `parent.snxsch` carries the sheet symbol, `child.snxsch`
//! (written to disk, never opened as a tab) carries the labels.

use signex_app::app::{ContextAction, ContextMenuMsg, EditMsg, ErcMsg, Message, Signex};
use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SelectedItem,
    SelectedKind, SheetPin, VAlign,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

fn label(text: &str, label_type: LabelType, shape: &str) -> Label {
    Label {
        uuid: uuid::Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        label_type,
        shape: shape.to_string(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn pin(name: &str, direction: &str, auto: bool, moved: bool, at: (f64, f64)) -> SheetPin {
    SheetPin {
        uuid: uuid::Uuid::new_v4(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        auto_generated: auto,
        user_moved: moved,
    }
}

fn symbol_for(filename: &str, pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::new_v4(),
        name: "Child".to_string(),
        filename: filename.to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise sheet");
    std::fs::write(path, text).expect("write sheet");
}

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    child_path: PathBuf,
    symbol_uuid: uuid::Uuid,
}

/// The parent sheet is the active tab and its sheet symbol is selected. The
/// symbol starts with OLD_AUTO (stale, auto), MANUAL (hand-added) and MOVED
/// (auto, user-moved, direction "output").
fn fixture(child: Option<SchematicSheet>) -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    let parent_path = tmp.path().join("parent.snxsch");
    let child_path = tmp.path().join("child.snxsch");
    if let Some(child) = child {
        write_sheet(&child_path, child);
    }

    let symbol = symbol_for(
        "child.snxsch",
        vec![
            pin("OLD_AUTO", "input", true, false, (100.0, 102.0)),
            pin("MANUAL", "input", false, false, (100.0, 106.0)),
            pin("MOVED", "output", true, true, (123.0, 117.0)),
        ],
    );
    let symbol_uuid = symbol.uuid;
    let mut parent = empty_sheet();
    parent.child_sheets.push(symbol);
    write_sheet(&parent_path, parent.clone());

    let (mut app, _task) = Signex::new();
    let engine =
        signex_engine::Engine::new_with_path(parent, Some(parent_path.clone())).expect("engine");
    app.document_state
        .engines
        .insert(parent_path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "parent".to_string(),
        path: parent_path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(parent_path);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(symbol_uuid, SelectedKind::ChildSheet)];
    Fixture {
        app,
        _tmp: tmp,
        child_path,
        symbol_uuid,
    }
}

/// Exposes SDA (no shape), MOVED (input), GLOB (global, output); the Net
/// label is not a port.
fn child_with_labels() -> SchematicSheet {
    let mut child = empty_sheet();
    child.labels = vec![
        label("SDA", LabelType::Hierarchical, ""),
        label("MOVED", LabelType::Hierarchical, "input"),
        label("GLOB", LabelType::Global, "output"),
        label("LOCAL_ONLY", LabelType::Net, "output"),
    ];
    child
}

fn sync(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::SyncSheetPins,
    )));
}

fn pins_of(f: &Fixture) -> Vec<SheetPin> {
    f.app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .child_sheets
        .iter()
        .find(|c| c.uuid == f.symbol_uuid)
        .expect("symbol")
        .pins
        .clone()
}

fn named<'a>(pins: &'a [SheetPin], name: &str) -> Option<&'a SheetPin> {
    pins.iter().find(|p| p.name == name)
}

fn sorted_names(pins: &[SheetPin]) -> Vec<String> {
    let mut names: Vec<_> = pins.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

fn panel_has(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|entry| entry.message.contains(needle))
}

fn setup_logger() {
    let _ = signex_app::diagnostics::init_logging();
}

#[test]
fn sync_adds_new_drops_stale_and_keeps_user_pins() {
    let mut f = fixture(Some(child_with_labels()));

    sync(&mut f.app);

    let pins = pins_of(&f);
    assert!(
        named(&pins, "SDA").is_some_and(|p| p.auto_generated),
        "child gained SDA"
    );
    assert!(
        named(&pins, "GLOB").is_some(),
        "Global labels are ports too"
    );
    assert!(
        named(&pins, "LOCAL_ONLY").is_none(),
        "Net labels never cross"
    );
    assert!(named(&pins, "OLD_AUTO").is_none(), "stale auto pin dropped");
    assert!(
        named(&pins, "MANUAL").is_some_and(|p| !p.auto_generated),
        "hand-added pin survives"
    );
    let moved = named(&pins, "MOVED").expect("MOVED stays");
    assert_eq!((moved.position.x, moved.position.y), (123.0, 117.0));
    assert!(moved.user_moved);
}

#[test]
fn sync_never_overwrites_an_existing_pins_direction() {
    let mut f = fixture(Some(child_with_labels()));

    sync(&mut f.app);

    let pins = pins_of(&f);
    // MOVED's label says "input" but the user's pin is "output": the pin wins.
    assert_eq!(named(&pins, "MOVED").expect("MOVED").direction, "output");
    // New pins take their label's shape; no shape means bidirectional.
    assert_eq!(named(&pins, "SDA").expect("SDA").direction, "bidirectional");
    assert_eq!(named(&pins, "GLOB").expect("GLOB").direction, "output");
}

#[test]
fn sync_is_undoable() {
    let mut f = fixture(Some(child_with_labels()));
    let before = sorted_names(&pins_of(&f));

    sync(&mut f.app);
    assert_ne!(sorted_names(&pins_of(&f)), before, "the sync changed pins");

    let _ = f.app.update(Message::Edit(EditMsg::Undo));

    assert_eq!(
        sorted_names(&pins_of(&f)),
        before,
        "undo restores the old pins"
    );
}

#[test]
fn sync_marks_the_sheet_dirty_and_rederives_the_netlist() {
    let mut f = fixture(Some(child_with_labels()));
    assert!(f.app.ui_state.project_netlist.is_none(), "precondition");

    sync(&mut f.app);

    assert!(
        f.app.ui_state.project_netlist.is_some(),
        "the netlist is re-derived after the sync"
    );
    assert!(!f.app.document_state.dirty_paths.is_empty());
}

#[test]
fn missing_child_file_reports_a_message_and_changes_nothing() {
    setup_logger();
    let mut f = fixture(None);
    let before = pins_of(&f).len();

    sync(&mut f.app);

    assert_eq!(pins_of(&f).len(), before);
    assert!(
        panel_has(&f.app, "Sync Sheet Pins: child schematic not found"),
        "a missing child must be visible in the Messages panel"
    );
}

#[test]
fn unparseable_child_file_reports_a_message_and_changes_nothing() {
    setup_logger();
    let mut f = fixture(None);
    std::fs::write(&f.child_path, "this is not a schematic").expect("write garbage");
    let before = pins_of(&f).len();

    sync(&mut f.app);

    assert_eq!(pins_of(&f).len(), before);
    assert!(panel_has(&f.app, "could not be parsed"));
}

#[test]
fn child_without_port_labels_reports_a_message_and_changes_nothing() {
    setup_logger();
    let mut child = empty_sheet();
    child.labels = vec![label("LOCAL_ONLY", LabelType::Net, "")];
    let mut f = fixture(Some(child));
    let before = pins_of(&f).len();

    sync(&mut f.app);

    assert_eq!(pins_of(&f).len(), before);
    assert!(panel_has(&f.app, "has no hierarchical or global labels"));
}

fn hier_pin_messages(app: &Signex) -> Vec<String> {
    app.ui_state
        .erc_violations
        .iter()
        .filter(|v| v.rule == signex_erc::RuleKind::BadHierSheetPin)
        .map(|v| v.message.clone())
        .collect()
}

#[test]
fn sync_clears_the_bad_hier_sheet_pin_drift_on_the_next_erc_run() {
    let mut f = fixture(Some(child_with_labels()));
    let _ = f.app.update(Message::Erc(ErcMsg::Run));
    let drift = hier_pin_messages(&f.app);
    assert!(
        drift.iter().any(|m| m.contains("Sheet pin 'OLD_AUTO'")),
        "precondition: stale pin flagged, got {drift:?}"
    );
    assert!(
        drift.iter().any(|m| m.contains("Hierarchical label 'SDA'")),
        "precondition: unexposed label flagged, got {drift:?}"
    );

    sync(&mut f.app);
    let _ = f.app.update(Message::Erc(ErcMsg::Run));

    let after = hier_pin_messages(&f.app);
    assert!(
        !after
            .iter()
            .any(|m| m.contains("'OLD_AUTO'") || m.contains("label 'SDA'")),
        "the synced drift is gone, got {after:?}"
    );
}
