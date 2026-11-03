//! #1716 — what the drag ghost shows is what the drop commits, for every way
//! a lock holds a drag back: a locked symbol, a symbol on a locked wire, and
//! a wire on a locked symbol's pin. The ghost is `MovePreview::moved_with`,
//! the call the canvas makes; the drop goes through `Signex::update`.

use signex_app::app::{Message, MoveSelectionMsg, Signex, TabInfo, TabKind, UiMsg};
use signex_app::canvas::CanvasEvent;
use signex_app::diagnostics;
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};
use std::path::PathBuf;

const DY: f64 = 10.0;

fn resistor_lib() -> LibSymbol {
    let pin = LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(-5.0, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    };
    LibSymbol {
        id: "R".to_string(),
        reference: String::new(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![pin],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

struct Rig {
    app: Signex,
    symbol: SelectedItem,
    wire: SelectedItem,
    other: SelectedItem,
}

/// `reference` at (50, 50) with a wire into its pin at (45, 50), and a free
/// unlocked `R2` far away. `lock_symbol` / `lock_wire` pick what is locked.
fn rig(reference: &str, lock_symbol: bool, lock_wire: bool) -> Rig {
    let _ = diagnostics::init_logging();
    let (symbol, other, wire) = (
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
        uuid::Uuid::now_v7(),
    );
    let place = |uuid, reference: &str, x, locked| Symbol {
        uuid,
        lib_id: "R".to_string(),
        reference: reference.to_string(),
        position: Point::new(x, 50.0),
        locked,
        ..Default::default()
    };
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![
            place(symbol, reference, 50.0, lock_symbol),
            place(other, "R2", 150.0, false),
        ],
        wires: vec![Wire {
            uuid: wire,
            start: Point::new(30.0, 50.0),
            end: Point::new(45.0, 50.0),
            locked: lock_wire,
            ..Default::default()
        }],
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), resistor_lib());
    let path = PathBuf::from("drag-ghost-parity.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "drag-ghost-parity".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.ui_state.snap_enabled = false;
    app.ui_state.move_keeps_wires = true;
    let item = |uuid, kind| SelectedItem::new(uuid, kind);
    Rig {
        app,
        symbol: item(symbol, SelectedKind::Symbol),
        wire: item(wire, SelectedKind::Wire),
        other: item(other, SelectedKind::Symbol),
    }
}

fn sheet(app: &Signex) -> SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .clone()
}

/// What the ghost shows for dragging `selection` down by `DY`.
fn ghost(app: &Signex, selection: &[SelectedItem]) -> SchematicSheet {
    let moves: Vec<_> = selection.iter().map(|item| (*item, 0.0, DY)).collect();
    signex_engine::MovePreview::new(&sheet(app))
        .moved_with(&moves, true, &Default::default())
        .0
}

fn drop_selection(app: &mut Signex, selection: &[SelectedItem]) {
    app.interaction_state.active_canvas_mut().selected = selection.to_vec();
    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 0.0,
        dy: DY,
        connections: Default::default(),
    }));
}

/// Ghost and drop leave the same wires and symbols.
fn assert_parity(app: &mut Signex, selection: &[SelectedItem]) -> SchematicSheet {
    let ghost = ghost(app, selection);
    drop_selection(app, selection);
    let committed = sheet(app);
    assert_eq!(
        committed.wires, ghost.wires,
        "wires: drop differs from ghost"
    );
    let at = |s: &SchematicSheet| s.symbols.iter().map(|s| s.position).collect::<Vec<_>>();
    assert_eq!(
        at(&committed),
        at(&ghost),
        "symbols: drop differs from ghost"
    );
    committed
}

fn logged(needle: &str) -> usize {
    diagnostics::recent_entries()
        .iter()
        .filter(|entry| entry.message.contains(needle))
        .count()
}

#[test]
fn a_wire_on_a_locked_pin_stays_put_on_the_drop_as_the_ghost_shows() {
    let mut rig = rig("RPIN1716A", true, false);
    let needle = "Skipped 1 locked object(s): RPIN1716A";
    let before = logged(needle);
    let start = sheet(&rig.app).wires[0].clone();

    let after = assert_parity(&mut rig.app, &[rig.wire]);

    assert_eq!(after.wires[0], start, "the wire must not move");
    assert_eq!(logged(needle), before + 1, "the drop names what it skipped");
}

#[test]
fn a_free_symbol_moves_and_the_wire_on_a_locked_pin_stays_as_the_ghost_shows() {
    let mut rig = rig("RPIN1716B", true, false);
    let start = sheet(&rig.app);

    let after = assert_parity(&mut rig.app, &[rig.wire, rig.other]);

    assert_eq!(after.wires[0], start.wires[0], "the held wire stays");
    assert_eq!(after.symbols[1].position, Point::new(150.0, 50.0 + DY));
}

#[test]
fn a_locked_symbol_stays_put_on_the_drop_as_the_ghost_shows() {
    let mut rig = rig("RSYM1716", true, false);
    let start = sheet(&rig.app);

    let after = assert_parity(&mut rig.app, &[rig.symbol]);

    assert_eq!(after.symbols[0].position, start.symbols[0].position);
    assert_eq!(after.wires, start.wires, "no bend wire follows a held part");
}

#[test]
fn a_symbol_on_a_locked_wire_stays_put_on_the_drop_as_the_ghost_shows() {
    let mut rig = rig("RWIRE1716", false, true);
    let start = sheet(&rig.app);

    let after = assert_parity(&mut rig.app, &[rig.symbol]);

    assert_eq!(after.symbols[0].position, start.symbols[0].position);
    assert_eq!(after.wires, start.wires);
}

/// A rig whose only selection is the wire on the locked pin, with the
/// keymap the rotate and mirror keys need.
fn all_held_rig(reference: &str) -> Rig {
    let mut rig = rig(reference, true, false);
    rig.app.ui_state.first_run_tour_open = false;
    rig.app
        .ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    rig.app.ui_state.active_keymap = rig.app.ui_state.keymap_profiles.compile_active().into();
    rig.app.interaction_state.active_canvas_mut().selected = vec![rig.wire];
    rig
}

fn assert_nothing_happened(app: &Signex, before: &SchematicSheet, undo: usize, what: &str) {
    let engine = app.document_state.active_engine().expect("engine");
    assert_eq!(
        engine.document().wires,
        before.wires,
        "{what}: the wire moved"
    );
    assert_eq!(
        engine.undo_position(),
        undo,
        "{what}: an undo step was made"
    );
    let path = app.document_state.active_path.clone().expect("path");
    assert!(
        !app.document_state.is_document_dirty(&path),
        "{what}: the sheet was marked dirty"
    );
}

fn key(token: KeyToken) -> UiMsg {
    UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers::default(),
            key: token,
        },
    }
}

#[test]
fn rotate_and_mirror_keys_skip_a_wire_on_a_locked_pin_without_an_undo_step() {
    let mut rig = all_held_rig("RKEYS1716");
    let before = sheet(&rig.app);
    let undo = rig
        .app
        .document_state
        .active_engine()
        .expect("engine")
        .undo_position();
    let needle = "Skipped 1 locked object(s): RKEYS1716";
    let logged_before = logged(needle);

    for token in [
        KeyToken::Space,
        KeyToken::Character("x".into()),
        KeyToken::Character("y".into()),
    ] {
        let _ = rig.app.update(Message::Ui(key(token)));
    }

    assert_nothing_happened(&rig.app, &before, undo, "rotate/mirror keys");
    assert_eq!(logged(needle), logged_before + 3, "each key names the skip");
}

#[test]
fn the_move_dialog_skips_a_wire_on_a_locked_pin_without_an_undo_step() {
    let mut rig = all_held_rig("RDIALOG1716");
    let before = sheet(&rig.app);
    let undo = rig
        .app
        .document_state
        .active_engine()
        .expect("engine")
        .undo_position();
    let needle = "Skipped 1 locked object(s): RDIALOG1716";
    let logged_before = logged(needle);
    rig.app.ui_state.move_selection.dx = "10".to_string();
    rig.app.ui_state.move_selection.dy = "0".to_string();

    let _ = rig
        .app
        .update(Message::MoveSelection(MoveSelectionMsg::Apply));

    assert_nothing_happened(&rig.app, &before, undo, "the Move Selection dialog");
    assert_eq!(logged(needle), logged_before + 1);
}

#[test]
fn the_ghost_names_each_kind_of_hold_in_its_own_words() {
    let rig = rig("RHOLD1716", true, false);
    let held = |selection: &[SelectedItem]| {
        let moves: Vec<_> = selection.iter().map(|item| (*item, 0.0, DY)).collect();
        signex_engine::MovePreview::new(&sheet(&rig.app))
            .moved_with(&moves, true, &Default::default())
            .1
    };
    assert_eq!(held(&[rig.symbol]), ["RHOLD1716"], "the part's own lock");
    assert_eq!(
        held(&[rig.wire]),
        ["a wire on RHOLD1716's pin (RHOLD1716 is locked)"]
    );

    let by_wire = self::rig("RHOLD1716W", false, true);
    let moves = [(by_wire.symbol, 0.0, DY)];
    let held = signex_engine::MovePreview::new(&sheet(&by_wire.app))
        .moved_with(&moves, true, &Default::default())
        .1;
    assert_eq!(held, ["RHOLD1716W (a wire attached to it is locked)"]);

    let _ = diagnostics::init_logging();
    diagnostics::log_locked_holds(&held);
    assert_eq!(
        logged(
            "Will not move when dropped: RHOLD1716W (a wire attached to it is locked); \
             unlock the locked object in Properties to move it"
        ),
        1
    );
}
