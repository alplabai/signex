//! #533 — the engine owns the undo history.
//!
//! There used to be two histories that had to advance in lockstep and
//! did not:
//!
//! * the **engine's** own, pushed inside `Engine::execute` via
//!   `record_history` — it happens whatever the caller does with the
//!   returned `CommandResult`;
//! * the app's **marker stack** (`UndoStack` in the since-deleted
//!   `crates/signex-app/src/undo.rs`), pushed only by
//!   `mutation_gateway.rs` from `CommandResult::changed`.
//!
//! `apply_engine_undo` was driven by the marker stack: it peeked how
//! many engine steps the top marker covered, then called `Engine::undo`
//! that many times. The Edit menu's Undo item, however, is enabled from
//! the *engine* (`app/view/mod.rs` reads `e.can_undo()`).
//!
//! So a call site that ran `engine.execute(...)` directly and dropped
//! the `CommandResult` pushed an engine entry with no marker to match.
//! The stacks slipped by one, the oldest edit could no longer be
//! reached, and Undo went on claiming to be available while doing
//! nothing. The stack was also one *global* stack across every open
//! document's per-path engine, never cleared on tab switch, so its
//! counts could not be right across tabs even in principle.
//!
//! The marker stack is gone. `apply_engine_undo` / `apply_engine_redo`
//! call `Engine::undo` / `Engine::redo` once, and batches are one engine
//! history entry because `apply_engine_commands` goes through
//! `Engine::execute_batch`. These tests pin down the four properties
//! that used to break.
//!
//! `handle_move_selection_apply` (`app/handlers/erc/modals.rs:304`) is
//! one of the five sites #533 lists under Class B, and is used below
//! precisely because it still runs the engine directly — it is correct
//! now without having been touched. The others are `modals.rs:325` and
//! `erc/annotate.rs:77,268,296`.

use signex_app::app::{EditMsg, Message, MoveSelectionMsg, Signex};
use signex_types::schematic::{
    NoConnect, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn symbol(uuid: uuid::Uuid, reference: &str, x: f64) -> Symbol {
    Symbol {
        uuid,
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(x, 0.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

fn sheet_with(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols,
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

/// Insert an engine for `path` and give it a matching `TabInfo`, which
/// `finish_schematic_mutation` requires or it silently no-ops.
fn add_tab(app: &mut Signex, path: &Path, sheet: SchematicSheet) {
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: path.to_string_lossy().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
}

/// Make the tab whose document lives at `path` the active one — the same
/// two fields the tab-switch handlers set.
fn activate(app: &mut Signex, path: &Path) {
    let idx = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab for path");
    app.document_state.active_tab = idx;
    app.document_state.active_path = Some(path.to_path_buf());
}

fn symbols_of(app: &Signex, path: &Path) -> Vec<Symbol> {
    app.document_state
        .engines
        .get(path)
        .expect("engine")
        .document()
        .symbols
        .clone()
}

/// A loaded, active schematic holding two plain symbols, so a test can
/// spend one on a gateway edit and one on a gateway-bypassing edit.
fn fixture_two_symbols() -> (Signex, uuid::Uuid, uuid::Uuid) {
    let doomed_uuid = uuid::Uuid::new_v4();
    let moved_uuid = uuid::Uuid::new_v4();

    let path = PathBuf::from("undo-marker.snxsch");
    let (mut app, _initial_task) = Signex::new();
    add_tab(
        &mut app,
        &path,
        sheet_with(vec![
            symbol(doomed_uuid, "R1", 0.0),
            symbol(moved_uuid, "R2", 50.0),
        ]),
    );
    activate(&mut app, &path);

    (app, doomed_uuid, moved_uuid)
}

/// Delete one symbol through the gateway, then move the other through
/// the Move Selection dialog, which calls `engine.execute` directly.
fn one_gateway_edit_then_one_bypassing_edit() -> (Signex, uuid::Uuid) {
    let (mut app, doomed_uuid, moved_uuid) = fixture_two_symbols();

    // Edit 1: through the gateway. `handle_selection_delete_requested`
    // routes via `apply_engine_command`.
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(doomed_uuid, SelectedKind::Symbol)];
    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    assert_eq!(
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .symbols
            .len(),
        1,
        "fixture precondition: the gateway delete must have removed one symbol"
    );

    // Edit 2: bypasses the gateway. `handle_move_selection_apply` calls
    // `engine.execute(...)` and drops the `CommandResult`.
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(moved_uuid, SelectedKind::Symbol)];
    app.ui_state.move_selection.dx = "10".to_string();
    app.ui_state.move_selection.dy = "0".to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let moved_x = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == moved_uuid)
        .expect("moved symbol still present")
        .position
        .x;
    assert!(
        (moved_x - 60.0).abs() < 1e-9,
        "fixture precondition: the modal move must have applied (x 50.0 + 10.0), got {moved_x}"
    );

    (app, doomed_uuid)
}

/// The failure a user met: two edits in, two Undos must get back to the
/// starting document. The second Undo used to do nothing, so the deleted
/// symbol never came back.
#[test]
fn two_edits_then_two_undos_restores_both() {
    let (mut app, doomed_uuid) = one_gateway_edit_then_one_bypassing_edit();

    // First Undo reverts the *move*. Under the marker stack it spent the
    // *delete's* marker to do it, because the move never pushed one.
    let _ = app.update(Message::Edit(EditMsg::Undo));
    let after_first = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone();
    assert_eq!(
        after_first.symbols.len(),
        1,
        "the first Undo reverts the move, not the delete — the symbol count stays at 1"
    );
    assert!(
        after_first
            .symbols
            .iter()
            .all(|s| (s.position.x - 50.0).abs() < 1e-9),
        "the first Undo must revert the modal move (x back to 50.0), got {:?}",
        after_first
            .symbols
            .iter()
            .map(|s| s.position.x)
            .collect::<Vec<_>>()
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let doc = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone();
    assert!(
        doc.symbols.iter().any(|s| s.uuid == doomed_uuid),
        "the second Undo must restore the deleted symbol; the engine holds its \
         history entry, and Undo now reads that history directly instead of a \
         marker stack that the Move Selection dialog never fed"
    );
}

/// The sharper statement of the same defect: after the marker stack was
/// exhausted the menu still offered Undo, because `can_undo` is read from
/// the engine while `apply_engine_undo` was driven by the marker stack.
/// An enabled menu item that silently did nothing.
#[test]
fn undo_availability_agrees_with_what_undo_can_actually_do() {
    let (mut app, _doomed_uuid) = one_gateway_edit_then_one_bypassing_edit();

    // Drain every undo the app is willing to perform.
    for _ in 0..8 {
        let _ = app.update(Message::Edit(EditMsg::Undo));
    }

    let engine_offers_undo = app
        .document_state
        .active_engine()
        .expect("engine")
        .can_undo();

    assert!(
        !engine_offers_undo,
        "the Edit menu enables Undo from `Engine::can_undo` (app/view/mod.rs), so \
         once Undo stops doing anything the engine must agree there is nothing left"
    );
}

/// Batch atomicity through real messages: a paste that places three
/// objects is one user action, so one Undo must take all three back.
/// `EditMsg::Paste` routes to `apply_engine_commands`, which is now
/// `Engine::execute_batch`.
///
/// This one is a guard, not a reproduction — the marker stack got this
/// case right, by recording `steps: 3` and undoing three times. What it
/// pins is that moving the grouping into the engine did not lose it.
#[test]
fn a_pasted_batch_is_one_undo_step() {
    let path = PathBuf::from("undo-batch.snxsch");
    let (mut app, _initial_task) = Signex::new();
    add_tab(&mut app, &path, sheet_with(Vec::new()));
    activate(&mut app, &path);

    app.interaction_state.clipboard_no_connects = (0..3)
        .map(|i| NoConnect {
            uuid: uuid::Uuid::new_v4(),
            position: Point::new(f64::from(i) * 10.0, 0.0),
        })
        .collect();

    let _ = app.update(Message::Edit(EditMsg::Paste));

    assert_eq!(
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .no_connects
            .len(),
        3,
        "fixture precondition: the paste must have placed all three no-connects"
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));

    let engine = app.document_state.active_engine().expect("engine");
    assert!(
        engine.document().no_connects.is_empty(),
        "one Undo must take the whole pasted batch back, not one object of it; {} left",
        engine.document().no_connects.len()
    );
    assert!(
        !engine.can_undo(),
        "the batch was one history entry, so nothing is left to undo"
    );
}

/// Undo is per document, and one press spends exactly one of the active
/// document's edits.
///
/// This is the cross-tab corruption in its sharpest form. The marker
/// stack was one *global* stack across every open document's per-path
/// engine and was never cleared on tab switch, so the count on top of it
/// could describe a different document than the one being undone. Three
/// separate edits in tab B, then a three-object paste in tab A, left the
/// top marker reading `steps: 3` — and a single Undo pressed back in tab
/// B ran `Engine::undo` three times on B, wiping all three of B's
/// unrelated edits in one keystroke.
///
/// The count matters: an Undo in a tab with *nothing* to undo was
/// already harmless, because the old `undone_steps != steps` guard
/// short-circuited. It is the mismatched count, not the mere sharing,
/// that destroyed work.
#[test]
fn an_undo_in_one_tab_spends_exactly_one_of_that_tabs_edits() {
    let path_a = PathBuf::from("undo-tab-a.snxsch");
    let path_b = PathBuf::from("undo-tab-b.snxsch");
    let b_symbols: Vec<uuid::Uuid> = (0..3).map(|_| uuid::Uuid::new_v4()).collect();

    let (mut app, _initial_task) = Signex::new();
    add_tab(&mut app, &path_a, sheet_with(Vec::new()));
    add_tab(
        &mut app,
        &path_b,
        sheet_with(
            b_symbols
                .iter()
                .enumerate()
                .map(|(i, uuid)| symbol(*uuid, &format!("R{i}"), f64::from(i as i32) * 10.0))
                .collect(),
        ),
    );

    // Tab B: three separate edits, one marker each under the old scheme.
    activate(&mut app, &path_b);
    for uuid in &b_symbols {
        app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(*uuid, SelectedKind::Symbol)];
        let _ = app.update(Message::Edit(EditMsg::DeleteSelected));
    }
    assert!(
        symbols_of(&app, &path_b).is_empty(),
        "fixture precondition: all three of tab B's symbols must be deleted"
    );

    // Tab A: one paste of three objects — a single marker reading
    // `steps: 3`, sitting on top of the shared stack.
    activate(&mut app, &path_a);
    app.interaction_state.clipboard_no_connects = (0..3)
        .map(|i| NoConnect {
            uuid: uuid::Uuid::new_v4(),
            position: Point::new(f64::from(i) * 10.0, 0.0),
        })
        .collect();
    let _ = app.update(Message::Edit(EditMsg::Paste));
    assert_eq!(
        symbols_of(&app, &path_a).len(),
        0,
        "fixture precondition: the paste places no-connects, not symbols"
    );
    assert_eq!(
        app.document_state
            .engines
            .get(&path_a)
            .expect("engine")
            .document()
            .no_connects
            .len(),
        3,
        "fixture precondition: tab A's paste must have placed all three no-connects"
    );

    // Back in tab B, one Undo must restore exactly one symbol.
    activate(&mut app, &path_b);
    let _ = app.update(Message::Edit(EditMsg::Undo));

    assert_eq!(
        symbols_of(&app, &path_b).len(),
        1,
        "one Undo in tab B must spend exactly one of tab B's own edits; the top \
         marker belonged to tab A's three-object paste, and undoing `steps: 3` \
         against tab B wiped all three of its unrelated deletes at once"
    );
    assert_eq!(
        app.document_state
            .engines
            .get(&path_a)
            .expect("engine")
            .document()
            .no_connects
            .len(),
        3,
        "an Undo pressed in tab B must not reach into tab A's document"
    );
}
