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
        unknown_columns: Default::default(),
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
    let symbol = symbol_for(
        "child.snxsch",
        vec![
            pin("OLD_AUTO", "input", true, false, (100.0, 102.0)),
            pin("MANUAL", "input", false, false, (100.0, 106.0)),
            pin("MOVED", "output", true, true, (123.0, 117.0)),
        ],
    );
    fixture_with(child, vec![symbol])
}

/// `fixture` over explicit sheet symbols; the first one is selected.
fn fixture_with(child: Option<SchematicSheet>, symbols: Vec<ChildSheet>) -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    let parent_path = tmp.path().join("parent.snxsch");
    let child_path = tmp.path().join("child.snxsch");
    if let Some(child) = child {
        write_sheet(&child_path, child);
    }

    let symbol_uuid = symbols[0].uuid;
    let mut parent = empty_sheet();
    parent.child_sheets.extend(symbols);
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
fn child_without_port_labels_drops_auto_pins_keeps_manual_and_reports() {
    setup_logger();
    let mut child = empty_sheet();
    child.labels = vec![label("LOCAL_ONLY", LabelType::Net, "")];
    let mut f = fixture(Some(child));

    sync(&mut f.app);

    let pins = pins_of(&f);
    assert_eq!(
        sorted_names(&pins),
        vec!["MANUAL".to_string()],
        "auto-generated OLD_AUTO and MOVED go, the hand-added pin stays"
    );
    assert!(panel_has(&f.app, "exposes no ports"));
}

#[test]
fn deleting_the_last_child_label_drops_its_stale_auto_pin_and_the_erc_finding() {
    setup_logger();
    let mut child = empty_sheet();
    child.labels = vec![label("SDA", LabelType::Hierarchical, "")];
    let mut f = fixture(Some(child));
    sync(&mut f.app);
    assert!(named(&pins_of(&f), "SDA").is_some(), "SDA pin created");

    // The user deletes the only label in the child and saves it.
    write_sheet(&f.child_path, empty_sheet());
    sync(&mut f.app);
    crate::settle_support::settle(&mut f.app, Message::Erc(ErcMsg::Run));

    assert!(
        named(&pins_of(&f), "SDA").is_none(),
        "stale auto pin SDA dropped"
    );
    let after = hier_pin_messages(&f.app);
    assert!(
        !after.iter().any(|m| m.contains("'SDA'")),
        "ERC no longer reports SDA, got {after:?}"
    );
}

#[test]
fn padded_label_is_cleared_by_erc_and_synced_under_the_trimmed_name() {
    setup_logger();
    let mut child = empty_sheet();
    child.labels = vec![label(" CLK ", LabelType::Hierarchical, "")];
    let mut f = fixture(Some(child));

    sync(&mut f.app);
    crate::settle_support::settle(&mut f.app, Message::Erc(ErcMsg::Run));

    let pins = pins_of(&f);
    assert!(
        named(&pins, "CLK").is_some(),
        "synced under the trimmed name"
    );
    assert!(named(&pins, " CLK ").is_none());
    let after = hier_pin_messages(&f.app);
    assert!(
        !after.iter().any(|m| m.contains("'CLK'")),
        "ERC treats the padded label as bound, got {after:?}"
    );
    // A second sync is a no-op: the pin the first one made already matches.
    sync(&mut f.app);
    assert_eq!(pins_of(&f).len(), pins.len());
}

fn hier_pin_messages(app: &Signex) -> Vec<String> {
    // Every sheet's list: a finding about a child's label is filed under the
    // child sheet, where its coordinate lives (#956).
    app.ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .filter(|v| v.rule == signex_erc::RuleKind::BadHierSheetPin)
        .map(|v| v.message.clone())
        .collect()
}

#[test]
fn sync_clears_the_bad_hier_sheet_pin_drift_on_the_next_erc_run() {
    let mut f = fixture(Some(child_with_labels()));
    crate::settle_support::settle(&mut f.app, Message::Erc(ErcMsg::Run));
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
    crate::settle_support::settle(&mut f.app, Message::Erc(ErcMsg::Run));

    let after = hier_pin_messages(&f.app);
    assert!(
        !after
            .iter()
            .any(|m| m.contains("'OLD_AUTO'") || m.contains("label 'SDA'")),
        "the synced drift is gone, got {after:?}"
    );
}

#[test]
fn resync_after_a_label_is_added_keeps_pins_in_place_and_reports_the_change() {
    setup_logger();
    let mut child = empty_sheet();
    child.labels = vec![
        label("B", LabelType::Hierarchical, ""),
        label("C", LabelType::Hierarchical, ""),
    ];
    let mut f = fixture(Some(child.clone()));
    sync(&mut f.app);
    let before = pins_of(&f);
    let b = named(&before, "B").expect("B").clone();
    let c = named(&before, "C").expect("C").clone();

    child.labels.push(label("A", LabelType::Hierarchical, ""));
    write_sheet(&f.child_path, child);
    sync(&mut f.app);

    let after = pins_of(&f);
    assert_eq!(named(&after, "B").expect("B").position, b.position);
    assert_eq!(named(&after, "C").expect("C").position, c.position);
    assert!(named(&after, "A").is_some_and(|p| p.auto_generated));
    assert!(
        panel_has(&f.app, "added A"),
        "the Messages panel names the added pin"
    );
}

#[test]
fn hand_added_pin_matching_a_label_stays_put_and_hand_added() {
    let mut child = empty_sheet();
    child.labels = vec![label("MANUAL", LabelType::Hierarchical, "")];
    let mut f = fixture(Some(child));
    let manual = named(&pins_of(&f), "MANUAL").expect("MANUAL").clone();

    sync(&mut f.app);

    let pins = pins_of(&f);
    let after = named(&pins, "MANUAL").expect("MANUAL");
    assert_eq!(after.uuid, manual.uuid);
    assert_eq!(after.position, manual.position);
    assert!(!after.auto_generated);
}

/// A 6 mm tall symbol has two edge slots, (100, 102.54) and (100, 103.46);
/// these hand-added pins fill both.
fn full_edge_symbol(name: &str, extra: &str) -> ChildSheet {
    let mut symbol = symbol_for(
        "child.snxsch",
        vec![
            pin(extra, "input", false, false, (100.0, 102.54)),
            pin("OTHER", "input", false, false, (100.0, 103.46)),
        ],
    );
    symbol.name = name.to_string();
    symbol.size = (30.0, 6.0);
    symbol
}

fn clk_child() -> SchematicSheet {
    let mut child = empty_sheet();
    child.labels = vec![label("CLK", LabelType::Hierarchical, "")];
    child
}

#[test]
fn a_full_edge_reports_the_unplaced_port_with_the_exact_message() {
    setup_logger();
    let mut f = fixture_with(Some(clk_child()), vec![full_edge_symbol("FullEdgeA", "X")]);

    sync(&mut f.app);

    assert!(named(&pins_of(&f), "CLK").is_none(), "no slot, no pin");
    assert!(panel_has(
        &f.app,
        "Sync Sheet Pins: no free slot on sheet symbol 'FullEdgeA' ('child.snxsch') for CLK; enlarge the sheet symbol or move the wires on its edge, then sync again"
    ));
}

#[test]
fn a_padded_hand_added_pin_is_not_reported_as_unplaced() {
    setup_logger();
    let mut f = fixture_with(
        Some(clk_child()),
        vec![full_edge_symbol("FullEdgePadded", " CLK ")],
    );

    sync(&mut f.app);

    assert!(
        !panel_has(&f.app, "no free slot on sheet symbol 'FullEdgePadded'"),
        "the padded pin already carries CLK"
    );
}

#[test]
fn only_the_symbol_that_lacks_the_pin_is_reported() {
    setup_logger();
    let mut roomy = symbol_for("child.snxsch", Vec::new());
    roomy.name = "RoomyMulti".to_string();
    roomy.position = Point::new(200.0, 200.0);
    let mut f = fixture_with(
        Some(clk_child()),
        vec![full_edge_symbol("FullMulti", "X"), roomy],
    );

    sync(&mut f.app);

    assert!(panel_has(
        &f.app,
        "no free slot on sheet symbol 'FullMulti' ('child.snxsch') for CLK"
    ));
    assert!(!panel_has(
        &f.app,
        "no free slot on sheet symbol 'RoomyMulti'"
    ));
}

/// Everything about a pin the sync can change, comparable across undo/redo.
fn pin_state(pins: &[SheetPin]) -> Vec<(String, String, String, f64, f64, bool, bool)> {
    let mut state: Vec<_> = pins
        .iter()
        .map(|p| {
            (
                p.uuid.to_string(),
                p.name.clone(),
                p.direction.clone(),
                p.position.x,
                p.position.y,
                p.auto_generated,
                p.user_moved,
            )
        })
        .collect();
    state.sort_by(|a, b| a.partial_cmp(b).expect("finite coordinates"));
    state
}

#[test]
fn sync_is_undoable() {
    let mut f = fixture(Some(child_with_labels()));
    let before = pin_state(&pins_of(&f));

    sync(&mut f.app);
    let synced = pin_state(&pins_of(&f));
    assert_ne!(synced, before, "the sync changed pins");

    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(pin_state(&pins_of(&f)), before, "undo restores every field");

    let _ = f.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(
        pin_state(&pins_of(&f)),
        synced,
        "redo reapplies every field"
    );
}

fn select_symbols(f: &mut Fixture, symbols: &[uuid::Uuid]) {
    f.app.interaction_state.active_canvas_mut().selected = symbols
        .iter()
        .map(|id| SelectedItem::new(*id, SelectedKind::ChildSheet))
        .collect();
}

fn write_child(f: &Fixture, filename: &str, label_text: &str) {
    let dir = f.child_path.parent().expect("dir");
    let mut child = empty_sheet();
    child.labels = vec![label(label_text, LabelType::Hierarchical, "")];
    write_sheet(&dir.join(filename), child);
}

fn pin_names_of_symbol(f: &Fixture, id: uuid::Uuid) -> Vec<String> {
    let doc = f
        .app
        .document_state
        .active_engine()
        .expect("engine")
        .document();
    let symbol = doc
        .child_sheets
        .iter()
        .find(|c| c.uuid == id)
        .expect("symbol");
    sorted_names(&symbol.pins)
}

#[test]
fn every_selected_child_file_is_synced_and_reported() {
    setup_logger();
    let a = symbol_for("a.snxsch", Vec::new());
    let mut b = symbol_for("b.snxsch", Vec::new());
    b.position = Point::new(200.0, 200.0);
    let mut twin = symbol_for("a.snxsch", Vec::new());
    twin.position = Point::new(300.0, 300.0);
    let ids = [a.uuid, b.uuid, twin.uuid];
    let mut f = fixture_with(None, vec![a, b, twin]);
    write_child(&f, "a.snxsch", "ALPHA");
    write_child(&f, "b.snxsch", "BRAVO");
    select_symbols(&mut f, &ids);

    sync(&mut f.app);

    assert_eq!(pin_names_of_symbol(&f, ids[0]), vec!["ALPHA".to_string()]);
    assert_eq!(pin_names_of_symbol(&f, ids[1]), vec!["BRAVO".to_string()]);
    assert_eq!(pin_names_of_symbol(&f, ids[2]), vec!["ALPHA".to_string()]);
    assert!(panel_has(&f.app, "updated pins for 'a.snxsch'"));
    assert!(panel_has(&f.app, "updated pins for 'b.snxsch'"));
}

#[test]
fn a_missing_file_among_several_selected_does_not_stop_the_others() {
    setup_logger();
    let a = symbol_for("gone.snxsch", Vec::new());
    let mut b = symbol_for("b2.snxsch", Vec::new());
    b.position = Point::new(200.0, 200.0);
    let ids = [a.uuid, b.uuid];
    let mut f = fixture_with(None, vec![a, b]);
    write_child(&f, "b2.snxsch", "BRAVO2");
    select_symbols(&mut f, &ids);

    sync(&mut f.app);

    assert!(panel_has(&f.app, "child schematic not found"));
    assert!(panel_has(&f.app, "updated pins for 'b2.snxsch'"));
}

#[test]
fn a_blank_hierarchical_label_is_reported_not_silently_ignored() {
    setup_logger();
    let symbol = symbol_for("blank_labels.snxsch", Vec::new());
    let id = symbol.uuid;
    let mut f = fixture_with(None, vec![symbol]);
    let mut child = empty_sheet();
    child.labels = vec![
        label("REAL", LabelType::Hierarchical, ""),
        label("   ", LabelType::Hierarchical, ""),
        label("", LabelType::Global, ""),
        label("  ", LabelType::Net, ""),
    ];
    write_sheet(&f.child_path.with_file_name("blank_labels.snxsch"), child);

    sync(&mut f.app);

    assert_eq!(pin_names_of_symbol(&f, id), vec!["REAL".to_string()]);
    assert!(
        panel_has(
            &f.app,
            "Sync Sheet Pins: 'blank_labels.snxsch' has 2 Hierarchical or Global label(s) with blank text"
        ),
        "blank port labels (not Net ones) are named in the Messages panel"
    );
}

#[test]
fn a_clean_resync_says_already_match_and_has_no_blank_note() {
    setup_logger();
    let symbol = symbol_for("steady.snxsch", Vec::new());
    let mut f = fixture_with(None, vec![symbol]);
    write_child(&f, "steady.snxsch", "STEADY");
    sync(&mut f.app);

    sync(&mut f.app);

    assert!(panel_has(
        &f.app,
        "Sync Sheet Pins: pins for 'steady.snxsch' already match its labels"
    ));
    assert!(!panel_has(&f.app, "Sync Sheet Pins: 'steady.snxsch' has "));
}

#[test]
fn syncing_several_files_undoes_and_redoes_as_one_step() {
    let a = symbol_for("a.snxsch", Vec::new());
    let mut b = symbol_for("b.snxsch", Vec::new());
    b.position = Point::new(200.0, 200.0);
    let ids = [a.uuid, b.uuid];
    let mut f = fixture_with(None, vec![a, b]);
    write_child(&f, "a.snxsch", "ALPHA");
    write_child(&f, "b.snxsch", "BRAVO");
    select_symbols(&mut f, &ids);

    sync(&mut f.app);
    assert_eq!(pin_names_of_symbol(&f, ids[0]), vec!["ALPHA".to_string()]);
    assert_eq!(pin_names_of_symbol(&f, ids[1]), vec!["BRAVO".to_string()]);

    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert!(
        pin_names_of_symbol(&f, ids[0]).is_empty(),
        "one undo reverts the first file"
    );
    assert!(
        pin_names_of_symbol(&f, ids[1]).is_empty(),
        "one undo reverts the second file too"
    );

    let _ = f.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(pin_names_of_symbol(&f, ids[0]), vec!["ALPHA".to_string()]);
    assert_eq!(pin_names_of_symbol(&f, ids[1]), vec!["BRAVO".to_string()]);
}
