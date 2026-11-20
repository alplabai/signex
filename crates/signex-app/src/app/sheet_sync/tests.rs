//! Create Sheet Symbol From Sheet and the sync dialog, driven through
//! `Signex::update` with every returned task run to completion: the parent
//! sheet is open, the child sheet is a closed file in a temp folder.

use std::path::PathBuf;

use signex_engine::SheetSyncDirection;
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
    SheetPin,
};
use uuid::Uuid;

use super::SheetSyncMsg;
use super::open::previews_of;
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::{Message, Signex, TabInfo, TabKind};

const SYMBOL: u128 = 5;

fn pin(name: &str, direction: &str, y: f64, auto_generated: bool) -> SheetPin {
    SheetPin {
        uuid: Uuid::now_v7(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(if direction == "output" { 30.0 } else { 0.0 }, y),
        rotation: if direction == "output" { 180.0 } else { 0.0 },
        auto_generated,
        user_moved: false,
    }
}

fn sheet_symbol(pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::from_u128(SYMBOL),
        name: "child".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(0.0, 0.0),
        size: (30.0, 30.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn hierarchical(text: &str, shape: &str, y: f64) -> Label {
    Label {
        shape: shape.to_string(),
        ..Label::new(text, Point::new(10.0, y), LabelType::Hierarchical)
    }
}

fn write_sheet(path: &std::path::Path, sheet: SchematicSheet) -> String {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise");
    std::fs::write(path, &text).expect("write sheet");
    text
}

struct Rig {
    app: Signex,
    child: PathBuf,
    child_text: String,
    _dir: tempfile::TempDir,
}

/// The parent open and active with its sheet symbol selected; the child a
/// closed file beside it.
fn rig(symbol_pins: Vec<SheetPin>, child_labels: Vec<Label>) -> Rig {
    let dir = tempfile::tempdir().expect("tempdir");
    let parent_path = dir.path().join("parent.snxsch");
    let child = dir.path().join("child.snxsch");
    let child_text = write_sheet(
        &child,
        SchematicSheet {
            labels: child_labels,
            ..SchematicSheet::default()
        },
    );
    let parent = SchematicSheet {
        child_sheets: vec![sheet_symbol(symbol_pins)],
        ..SchematicSheet::default()
    };
    write_sheet(&parent_path, parent.clone());
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let engine =
        signex_engine::Engine::new_with_path(parent, Some(parent_path.clone())).expect("engine");
    app.document_state
        .engines
        .insert(parent_path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "parent".to_string(),
        path: parent_path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(parent_path);
    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(
        Uuid::from_u128(SYMBOL),
        SelectedKind::ChildSheet,
    )];
    Rig {
        app,
        child,
        child_text,
        _dir: dir,
    }
}

/// Run `task` the way the iced runtime does and feed every message back.
fn settle(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(next) = action {
                    messages.push(next);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

fn send(app: &mut Signex, msg: SheetSyncMsg) {
    let task = app.update(Message::from(msg));
    settle(app, task);
}

fn parent_pins(app: &Signex) -> Vec<String> {
    let engine = app.document_state.active_engine().expect("parent");
    let mut names: Vec<String> = engine.document().child_sheets[0]
        .pins
        .iter()
        .map(|p| p.name.clone())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn the_previews_list_each_directions_changes() {
    let symbol = sheet_symbol(vec![
        pin("NEW", "output", 2.54, false),
        pin("GONE", "input", 5.08, true),
    ]);
    let child = SchematicSheet {
        labels: vec![
            hierarchical("OLD", "output", 10.0),
            hierarchical("SDA", "input", 20.0),
        ],
        ..SchematicSheet::default()
    };
    let rename = |from: &str, to: &str| signex_engine::SheetEntryRename {
        from: from.to_string(),
        to: to.to_string(),
    };
    let previews = previews_of(&symbol, &child);
    // Sheet to symbol: the auto entry GONE becomes SDA (the lone input pair);
    // NEW was placed by hand, so it stays, and OLD is new to the symbol.
    assert_eq!(previews.to_symbol.added, ["OLD"]);
    assert!(previews.to_symbol.removed.is_empty());
    assert_eq!(previews.to_symbol.renamed, [rename("GONE", "SDA")]);
    // Symbol to sheet: each direction has one port gone and one entry new.
    assert!(previews.to_sheet.added.is_empty());
    assert!(previews.to_sheet.removed.is_empty());
    assert_eq!(
        previews.to_sheet.renamed,
        [rename("OLD", "NEW"), rename("SDA", "GONE")]
    );
}

#[test]
fn sheet_to_symbol_previews_then_applies_as_one_undo_step() {
    let mut rig = rig(
        vec![pin("GONE", "passive", 2.54, true)],
        vec![hierarchical("SDA", "input", 10.0)],
    );
    send(&mut rig.app, SheetSyncMsg::Requested);
    let state = rig.app.ui_state.sheet_sync.as_ref().expect("dialog open");
    let preview = state.preview().expect("the child was read");
    assert_eq!(preview.added, ["SDA"]);
    assert_eq!(preview.removed, ["GONE"]);
    assert!(state.can_apply());
    assert_eq!(parent_pins(&rig.app), ["GONE"], "a preview changes nothing");

    send(&mut rig.app, SheetSyncMsg::ApplyRequested);
    assert!(
        rig.app.ui_state.sheet_sync.is_none(),
        "Apply closes the dialog"
    );
    assert_eq!(parent_pins(&rig.app), ["SDA"]);
    rig.app
        .document_state
        .active_engine_mut()
        .expect("parent")
        .undo()
        .expect("undo");
    assert_eq!(parent_pins(&rig.app), ["GONE"], "one undo step");
}

#[test]
fn cancel_closes_the_dialog_without_a_change() {
    let mut rig = rig(
        vec![pin("GONE", "passive", 2.54, true)],
        vec![hierarchical("SDA", "input", 10.0)],
    );
    send(&mut rig.app, SheetSyncMsg::Requested);
    send(&mut rig.app, SheetSyncMsg::CancelRequested);
    assert!(rig.app.ui_state.sheet_sync.is_none());
    assert_eq!(parent_pins(&rig.app), ["GONE"]);
}

#[test]
fn symbol_to_sheet_changes_the_closed_child_in_memory_only() {
    let mut rig = rig(
        vec![
            pin("NEW", "output", 2.54, false),
            pin("IN1", "input", 2.54, false),
        ],
        vec![hierarchical("OLD", "output", 10.0)],
    );
    send(&mut rig.app, SheetSyncMsg::Requested);
    send(
        &mut rig.app,
        SheetSyncMsg::DirectionPicked(SheetSyncDirection::SymbolToSheet),
    );
    let state = rig.app.ui_state.sheet_sync.as_ref().expect("dialog open");
    let preview = state.preview().expect("the child was read");
    assert_eq!(preview.added, ["IN1"]);
    assert_eq!(preview.renamed.len(), 1);

    send(&mut rig.app, SheetSyncMsg::ApplyRequested);
    assert!(
        rig.app.ui_state.sheet_sync.is_none(),
        "Apply closes the dialog"
    );
    let child = rig
        .app
        .document_state
        .engines
        .get(&rig.child)
        .expect("the closed child is held in memory");
    let mut ports: Vec<String> = signex_engine::exposed_sheet_ports(child.document())
        .into_iter()
        .map(|p| p.name)
        .collect();
    ports.sort_unstable();
    assert_eq!(ports, ["IN1", "NEW"]);
    assert!(rig.app.document_state.is_document_dirty(&rig.child));
    let on_disk = std::fs::read_to_string(&rig.child).expect("child file");
    assert_eq!(on_disk, rig.child_text, "nothing is written until Save");
    assert_eq!(
        parent_pins(&rig.app),
        ["IN1", "NEW"],
        "the symbol is left alone"
    );
}

#[test]
fn a_symbol_from_a_sheet_lands_with_every_port_as_one_step() {
    let mut rig = rig(Vec::new(), Vec::new());
    let other = rig.child.with_file_name("power.snxsch");
    write_sheet(
        &other,
        SchematicSheet {
            labels: vec![
                hierarchical("VIN", "input", 10.0),
                hierarchical("VOUT", "output", 20.0),
            ],
            ..SchematicSheet::default()
        },
    );
    send(&mut rig.app, SheetSyncMsg::CreateSymbolPicked(Some(other)));
    let symbols = |app: &Signex| {
        app.document_state
            .active_engine()
            .expect("parent")
            .document()
            .child_sheets
            .clone()
    };
    let placed = symbols(&rig.app);
    assert_eq!(placed.len(), 2, "one new sheet symbol");
    let made = &placed[1];
    assert_eq!(
        (made.name.as_str(), made.filename.as_str()),
        ("power", "power.snxsch")
    );
    let mut entries: Vec<&str> = made.pins.iter().map(|p| p.name.as_str()).collect();
    entries.sort_unstable();
    assert_eq!(entries, ["VIN", "VOUT"]);
    rig.app
        .document_state
        .active_engine_mut()
        .expect("parent")
        .undo()
        .expect("undo");
    assert_eq!(symbols(&rig.app).len(), 1, "one undo step");
}

#[test]
fn a_file_already_placed_on_the_sheet_is_refused() {
    let mut rig = rig(Vec::new(), vec![hierarchical("A", "input", 10.0)]);
    let child = rig.child.clone();
    send(&mut rig.app, SheetSyncMsg::CreateSymbolPicked(Some(child)));
    let engine = rig.app.document_state.active_engine().expect("parent");
    assert_eq!(engine.document().child_sheets.len(), 1);
}

fn node(dir: &std::path::Path, file: &str, children: Vec<SheetNode>) -> SheetNode {
    SheetNode {
        name: file.into(),
        filename: file.into(),
        placed_as: Some(Uuid::now_v7()),
        path: dir.join(file),
        kind: SheetNodeKind::Present,
        detail: None,
        children,
    }
}

/// top places mid, mid places parent (the sheet in view): a symbol for top
/// on parent would loop two levels up, which no one-level check sees.
#[test]
fn a_grandparent_sheet_is_refused_as_a_loop() {
    let mut rig = rig(Vec::new(), Vec::new());
    let dir = rig.child.parent().expect("dir").to_path_buf();
    let top = dir.join("top.snxsch");
    write_sheet(
        &top,
        SchematicSheet {
            labels: vec![hierarchical("VIN", "input", 10.0)],
            ..SchematicSheet::default()
        },
    );
    let mid = node(
        &dir,
        "mid.snxsch",
        vec![node(&dir, "parent.snxsch", Vec::new())],
    );
    rig.app.document_state.panel_ctx.navigator.hierarchy = Some(SheetHierarchy {
        root: Some(node(&dir, "top.snxsch", vec![mid])),
        ..Default::default()
    });
    send(&mut rig.app, SheetSyncMsg::CreateSymbolPicked(Some(top)));
    let engine = rig.app.document_state.active_engine().expect("parent");
    assert_eq!(engine.document().child_sheets.len(), 1, "nothing placed");
}

/// A sheet symbol on `parent` placing `file`.
fn placing(file: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: file.trim_end_matches(".snxsch").to_string(),
        filename: file.to_string(),
        ..sheet_symbol(Vec::new())
    }
}

/// parent (A, in view) is open; b places c and c places parent, both closed
/// and in no hierarchy a window holds. A symbol for b on parent would close
/// A→B→C→A: the dialog refuses it as the operand form does
/// (`sheet_sync_operand_tests::a_loop_two_levels_down_is_refused`).
#[test]
fn a_loop_through_two_closed_sheets_is_refused() {
    let mut rig = rig(Vec::new(), Vec::new());
    let dir = rig.child.parent().expect("dir").to_path_buf();
    let b = dir.join("b.snxsch");
    write_sheet(
        &b,
        SchematicSheet {
            child_sheets: vec![placing("c.snxsch")],
            labels: vec![hierarchical("VIN", "input", 10.0)],
            ..SchematicSheet::default()
        },
    );
    write_sheet(
        &dir.join("c.snxsch"),
        SchematicSheet {
            child_sheets: vec![placing("parent.snxsch")],
            ..SchematicSheet::default()
        },
    );
    let _ = crate::diagnostics::init_logging();
    let mark = crate::diagnostics::newest_entry_id();
    send(&mut rig.app, SheetSyncMsg::CreateSymbolPicked(Some(b)));
    let engine = rig.app.document_state.active_engine().expect("parent");
    assert_eq!(engine.document().child_sheets.len(), 1, "nothing placed");
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|e| e.message)
        .filter(|m| {
            m.starts_with("Create Sheet Symbol From Sheet:")
                && m.contains("places this sheet below it")
        })
        .collect();
    assert!(
        !said.is_empty(),
        "refused as a loop, not for another reason"
    );
}
