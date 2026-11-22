//! Documents, selections and log readers for the Replay tests.

use std::path::{Path, PathBuf};

use signex_engine::Command;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Symbol};

use crate::app::command::CommandArgs;
use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;
use crate::library::editor::symbol::SymbolSelection;
use crate::library::messages::{FootprintEditorMsg, PrimitiveEdit, SymbolEditorMsg};
use crate::panels::command_log::{CommandLogMsg, LogOutcome, LogRow};

fn part(reference: &str, id: u128) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::from_u128(id),
        lib_id: "lib:r".to_string(),
        reference: reference.to_string(),
        unit: 1,
        in_bom: true,
        on_board: true,
        ..Symbol::default()
    }
}

pub(super) fn item(id: u128) -> SelectedItem {
    SelectedItem::new(uuid::Uuid::from_u128(id), SelectedKind::Symbol)
}

pub(super) fn add_sheet(app: &mut Signex, path: &Path, ids: &[(&str, u128)]) {
    let sheet = SchematicSheet {
        symbols: ids.iter().map(|(r, id)| part(r, *id)).collect(),
        ..SchematicSheet::default()
    };
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("an engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
}

/// One open sheet in view holding R1 (id 1) and R2 (id 2).
pub(super) fn app_with_parts() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("replay-a.snxsch");
    add_sheet(&mut app, &path, &[("R1", 1), ("R2", 2)]);
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    (app, path)
}

pub(super) fn select(app: &mut Signex, ids: &[u128]) {
    app.interaction_state.active_canvas_mut().selected = ids.iter().copied().map(item).collect();
}

pub(super) fn selected(app: &Signex) -> Vec<SelectedItem> {
    app.interaction_state.active_canvas().selected.clone()
}

pub(super) fn has(app: &Signex, path: &Path, id: u128) -> bool {
    app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .any(|symbol| symbol.uuid == uuid::Uuid::from_u128(id))
}

pub(super) fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    let _ = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
}

pub(super) fn replay(app: &mut Signex, seq: u64) {
    let _ = app.handle_command_log_message(CommandLogMsg::ReplayRequested(seq));
}

pub(super) fn newest(app: &Signex) -> LogRow {
    app.ui_state
        .command_log
        .log
        .rows()
        .next_back()
        .expect("a row")
        .clone()
}

pub(super) fn undo(app: &mut Signex, path: &Path) {
    app.document_state
        .engines
        .get_mut(path)
        .expect("the sheet")
        .undo()
        .expect("an undo step");
}

/// The newest row is the refusal of a command off the safe list (DECISIONS
/// 2026-11-22, "refuses unless proven safe").
pub(super) fn assert_not_replayable(app: &Signex) {
    let row = newest(app);
    let reason = failure(&row);
    assert!(reason.contains("can't be replayed"), "{reason}");
}

pub(super) fn failure(row: &LogRow) -> &str {
    match &row.outcome {
        LogOutcome::Failed(reason) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

pub(super) fn position(app: &Signex, path: &Path, id: u128) -> (f64, f64) {
    let symbol = app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .find(|symbol| symbol.uuid == uuid::Uuid::from_u128(id))
        .expect("the part");
    (symbol.position.x, symbol.position.y)
}

/// What a finished drag or Move runs through the gateway.
pub(super) fn move_part(app: &mut Signex, id: u128, dx: f64, dy: f64) {
    let moved = app.apply_engine_command(
        Command::MoveSelection {
            items: vec![item(id)],
            dx,
            dy,
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        },
        true,
        true,
    );
    assert!(moved, "the move landed");
}

/// A symbol tab with pins `1` and `2`.
pub(super) fn app_with_symbol() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("replay.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
    app.document_state.symbol_editors.insert(
        path.clone(),
        crate::app::SymbolDocument::new(path.clone(), file),
    );
    for (at, number) in [(0.0, "1"), (5.08, "2")] {
        symbol_edit(&mut app, &path, SymbolEditorMsg::AddPin { x: at, y: 0.0 });
        let idx = pins(&app, &path).len() - 1;
        symbol_edit(
            &mut app,
            &path,
            SymbolEditorMsg::SetPinNumber {
                idx,
                number: number.to_string(),
            },
        );
    }
    (app, path)
}

pub(super) fn symbol_edit(app: &mut Signex, path: &Path, msg: SymbolEditorMsg) {
    let _ = app.handle_primitive_editor_event(path.to_path_buf(), PrimitiveEdit::Symbol(msg));
}

pub(super) fn pins(app: &Signex, path: &Path) -> Vec<String> {
    app.document_state.symbol_editors[path]
        .primitive()
        .pins
        .iter()
        .map(|pin| pin.number.clone())
        .collect()
}

pub(super) fn pick_pin(app: &mut Signex, path: &Path, index: usize) {
    let editor = app
        .document_state
        .symbol_editors
        .get_mut(path)
        .expect("the editor");
    editor.selected = Some(SymbolSelection::Pin(index));
}

pub(super) fn board_with_two_parts() -> signex_types::pcb::PcbBoard {
    let footprint = |id: u128, reference: &str, x: f64| {
        serde_json::json!({
            "uuid": uuid::Uuid::from_u128(id), "reference": reference,
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "pads": [{"uuid": uuid::Uuid::from_u128(id + 100), "number": "1",
                "pad_type": "smd", "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
                "net": null}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(500),
        "setup": {"grid_size": 1.0, "trace_width": 0.3, "via_diameter": 0.8,
                  "via_drill": 0.4, "clearance": 0.2},
        "nets": [],
        "footprints": [footprint(1, "U1", 10.0), footprint(2, "U2", 30.0)]
    }))
    .expect("a board")
}

pub(super) fn footprint_x(app: &Signex, path: &Path, id: u128) -> f64 {
    app.document_state
        .pcb_board(path)
        .expect("the board")
        .footprints
        .iter()
        .find(|fp| fp.uuid == uuid::Uuid::from_u128(id))
        .expect("the footprint")
        .position
        .x
}

/// A footprint tab in view with two pads.
pub(super) fn app_with_footprint() -> (Signex, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("replay.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("F"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "F".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    for x_mm in [0.0, 5.08] {
        footprint_edit(
            &mut app,
            &path,
            FootprintEditorMsg::AddPad { x_mm, y_mm: 0.0 },
        );
    }
    assert_eq!(pads(&app, &path).len(), 2, "two pads");
    (app, path)
}

pub(super) fn footprint_edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    let _ = app.handle_primitive_editor_event(path.to_path_buf(), PrimitiveEdit::Footprint(msg));
}

pub(super) fn pads(app: &Signex, path: &Path) -> Vec<(String, (f64, f64))> {
    app.document_state.footprint_editors[path]
        .state
        .pads
        .iter()
        .map(|pad| (pad.number.clone(), pad.position_mm))
        .collect()
}

pub(super) fn pick_pad(app: &mut Signex, path: &Path, index: usize) {
    let editor = app
        .document_state
        .footprint_editors
        .get_mut(path)
        .expect("the editor");
    editor.state.selected_pad = Some(index);
    editor.state.selected_pads_extra.clear();
}
