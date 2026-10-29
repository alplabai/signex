//! #1250 — a locked symbol refuses delete and move from the app, and says
//! why in the Messages panel.

use signex_app::app::{EditMsg, Message, MoveSelectionMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::diagnostics;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use std::path::PathBuf;

fn locked_symbol_app(reference: &str) -> (Signex, uuid::Uuid) {
    let _ = diagnostics::init_logging();
    let uuid = uuid::Uuid::new_v4();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: reference.to_string(),
            position: Point::new(50.0, 0.0),
            locked: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = PathBuf::from("symbol-locked.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "symbol-locked".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    (app, uuid)
}

fn symbols(app: &Signex) -> Vec<Symbol> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .symbols
        .clone()
}

/// How many logged entries mention `needle`. The ring is shared between
/// tests, so each uses its own reference and compares counts.
fn logged(needle: &str) -> usize {
    diagnostics::recent_entries()
        .iter()
        .filter(|entry| entry.message.contains(needle))
        .count()
}

#[test]
fn delete_is_refused_and_reported() {
    let reference = "RLOCKDEL1250";
    let (mut app, _) = locked_symbol_app(reference);
    let needle = format!("{reference} is locked and cannot be");
    let before = logged(&needle);

    let _ = app.update(Message::Edit(EditMsg::DeleteSelected));

    assert_eq!(symbols(&app).len(), 1, "the locked symbol was deleted");
    assert_eq!(logged(&needle), before + 1, "no note for the refusal");
}

#[test]
fn the_move_dialog_is_refused_and_reported() {
    let reference = "RLOCKMOVE1250";
    let (mut app, _) = locked_symbol_app(reference);
    let needle = format!("{reference} is locked and cannot be");
    let before = logged(&needle);
    app.ui_state.move_selection.dx = "10".to_string();
    app.ui_state.move_selection.dy = "0".to_string();

    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let after = symbols(&app);
    assert!(
        (after[0].position.x - 50.0).abs() < 1e-9,
        "the locked symbol moved to {}",
        after[0].position.x
    );
    assert_eq!(logged(&needle), before + 1, "no note for the refusal");
}

/// A net label `GND_RAIL` beside the locked symbol (value `GND`) and a free
/// resistor valued `GND` too.
fn find_replace_app(reference: &str) -> (Signex, uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let (mut app, locked) = locked_symbol_app(reference);
    let label = uuid::Uuid::new_v4();
    let free = uuid::Uuid::new_v4();
    let engine = app.document_state.active_engine_mut().expect("engine");
    let mut sheet = engine.document().clone();
    sheet.symbols[0].value = "GND".to_string();
    sheet.symbols.push(Symbol {
        uuid: free,
        lib_id: "Device:R".to_string(),
        reference: "RFREE1250".to_string(),
        value: "GND".to_string(),
        position: Point::new(100.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(signex_types::schematic::Label {
        unknown_columns: Default::default(),
        uuid: label,
        text: "GND_RAIL".to_string(),
        position: Point::new(0.0, 50.0),
        rotation: 0.0,
        label_type: signex_types::schematic::LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
        font_family: None,
        locked: false,
    });
    engine.set_document(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    (app, locked, free, label)
}

fn replace_all(app: &mut Signex, query: &str, replacement: &str) {
    use signex_app::find_replace::FindReplaceMsg;
    for msg in [
        FindReplaceMsg::QueryChanged(query.into()),
        FindReplaceMsg::ReplacementChanged(replacement.into()),
        FindReplaceMsg::ReplaceAll,
    ] {
        let _ = app.update(Message::FindReplaceMsg(msg));
    }
}

#[test]
fn replace_all_skips_a_locked_symbol_and_still_replaces_the_rest() {
    let reference = "RLOCKREPL1250";
    let (mut app, locked, free, label) = find_replace_app(reference);
    let needle = format!("Skipped 1 locked object(s): {reference}");
    let before = logged(&needle);

    replace_all(&mut app, "GND", "VSS");

    let engine = app.document_state.active_engine().expect("engine");
    let sheet = engine.document();
    let value = |id| {
        sheet
            .symbols
            .iter()
            .find(|s| s.uuid == id)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(value(locked), "GND", "the locked symbol was edited");
    assert_eq!(value(free), "VSS", "the free symbol was not replaced");
    let text = &sheet.labels.iter().find(|l| l.uuid == label).unwrap().text;
    assert_eq!(text, "VSS_RAIL", "one locked match cancelled the rest");
    assert_eq!(
        logged(&needle),
        before + 1,
        "the skip was not reported once"
    );
}

#[test]
fn a_mixed_selection_moves_the_free_symbol_and_skips_the_locked_one() {
    let reference = "RLOCKMIX1250";
    let (mut app, locked, free, _) = find_replace_app(reference);
    let needle = format!("Skipped 1 locked object(s): {reference}");
    let before = logged(&needle);
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(locked, SelectedKind::Symbol),
        SelectedItem::new(free, SelectedKind::Symbol),
    ];
    app.ui_state.move_selection.dx = "10".to_string();
    app.ui_state.move_selection.dy = "0".to_string();

    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));

    let after = symbols(&app);
    let x = |id| after.iter().find(|s| s.uuid == id).unwrap().position.x;
    assert!((x(locked) - 50.0).abs() < 1e-9, "the locked symbol moved");
    assert!((x(free) - 110.0).abs() < 1e-9, "the free symbol stayed put");
    assert_eq!(
        logged(&needle),
        before + 1,
        "the skip was not reported once"
    );
}

fn locked_position_and_rotation(app: &Signex) -> (f64, f64, f64) {
    let symbol = &symbols(app)[0];
    (symbol.position.x, symbol.position.y, symbol.rotation)
}

#[test]
fn a_canvas_drag_leaves_a_locked_symbol_where_it_is() {
    let (mut app, _) = locked_symbol_app("RLOCKDRAG1250");

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 10.0,
        dy: 5.0,
        connections: Default::default(),
    }));

    assert_eq!(locked_position_and_rotation(&app), (50.0, 0.0, 0.0));
}

#[test]
fn keyboard_rotate_and_mirror_leave_a_locked_symbol_unchanged() {
    let (mut app, _) = locked_symbol_app("RLOCKKEYS1250");
    let before = symbols(&app)[0].clone();

    for msg in [
        EditMsg::RotateSelectedClockwise,
        EditMsg::RotateSelectedCounterclockwise,
        EditMsg::MirrorSelectedX,
        EditMsg::MirrorSelectedY,
    ] {
        let _ = app.update(Message::Edit(msg));
    }

    let after = &symbols(&app)[0];
    assert_eq!(after.rotation, before.rotation);
    assert_eq!(after.mirror_x, before.mirror_x);
    assert_eq!(after.mirror_y, before.mirror_y);
    assert_eq!(after.position, before.position);
}

#[test]
fn cut_skips_a_locked_symbol_without_touching_the_clipboard() {
    let reference = "RLOCKCUT1250";
    let (mut app, _) = locked_symbol_app(reference);
    let needle = format!("Skipped 1 locked object(s): {reference}");
    let before = logged(&needle);

    let _ = app.update(Message::Edit(EditMsg::Cut));

    assert_eq!(symbols(&app).len(), 1, "the locked symbol was cut");
    assert!(
        app.interaction_state.clipboard.is_none(),
        "the locked symbol reached the clipboard"
    );
    assert_eq!(logged(&needle), before + 1, "the skip was not reported");
    assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
}

#[test]
fn cut_takes_the_free_symbol_of_a_mixed_selection_only() {
    let (mut app, locked, free, _) = find_replace_app("RLOCKCUTMIX1250");
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(locked, SelectedKind::Symbol),
        SelectedItem::new(free, SelectedKind::Symbol),
    ];

    let _ = app.update(Message::Edit(EditMsg::Cut));

    let left: Vec<_> = symbols(&app).iter().map(|s| s.uuid).collect();
    assert_eq!(left, vec![locked], "only the free symbol should be cut");
    let copied: Vec<_> = app
        .interaction_state
        .clipboard
        .iter()
        .flat_map(|c| c.payload.fragment().symbols.iter())
        .map(|s| s.uuid)
        .collect();
    assert_eq!(copied, vec![free]);
}
