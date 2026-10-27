//! #1252 — Paste attaches the copied content to the cursor: Space / X / Y
//! turn it, one click places it as one undo step, and then the session ends.
//! The clipboard carries the library bodies its parts use, so a paste into
//! another sheet brings them along.

use std::path::PathBuf;

use signex_app::app::{EditMsg, Message, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::canvas::placement_ghost::ghost_anchor;
use signex_engine::Command;
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Wire};

use crate::library_place_symbol::{PLACE_AT, app_with_library, click, doc, fixture, place};
use crate::placement_session::second_sheet;

const FIRST: &str = "place-364.snxsch";

/// Sheet A with one placed library part (selected and copied), and an empty
/// sheet B. Returns the app and B's path; A is the active sheet.
fn app_with_copied_part() -> (Signex, PathBuf) {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    // Esc: let go of the library part, which stays on the cursor after a click.
    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        signex_app::app::Tool::Select,
    )));
    let part = doc(&app).symbols[0].uuid;
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(part, SelectedKind::Symbol)];
    let other = second_sheet(&mut app);
    // `f` owns the temp library; the part's body is already on sheet A.
    std::mem::forget(f);
    let _ = app.update(Message::Edit(EditMsg::Copy));
    assert!(app.interaction_state.clipboard.is_some());
    (app, other)
}

fn show(app: &mut Signex, sheet: &str) {
    app.document_state.active_path = Some(PathBuf::from(sheet));
}

fn symbols_on(app: &Signex, sheet: &str) -> usize {
    app.document_state.engines[&PathBuf::from(sheet)]
        .document()
        .symbols
        .len()
}

#[test]
fn paste_into_other_sheet_registers_lib_symbol() {
    let (mut app, other) = app_with_copied_part();
    let other_name = other.to_string_lossy().into_owned();
    show(&mut app, &other_name);

    let _ = app.update(Message::Edit(EditMsg::Paste));
    click(&mut app, PLACE_AT);

    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 1);
    let pasted = &sheet.symbols[0];
    assert!(
        sheet.lib_symbols.contains_key(&pasted.lib_id),
        "the pasted part's body must exist on the target sheet"
    );
}

#[test]
fn paste_is_one_undo_step_and_single_shot() {
    let (mut app, _other) = app_with_copied_part();

    let _ = app.update(Message::Edit(EditMsg::Paste));
    assert!(
        app.interaction_state.placement.is_some(),
        "the copy is on the cursor"
    );
    assert_eq!(
        symbols_on(&app, FIRST),
        1,
        "nothing placed before the click"
    );
    click(&mut app, (50.8, 50.8));
    assert_eq!(symbols_on(&app, FIRST), 2);
    assert!(
        app.interaction_state.placement.is_none(),
        "paste is single-shot"
    );

    click(&mut app, (76.2, 50.8));
    assert_eq!(symbols_on(&app, FIRST), 2, "a second click places nothing");

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        symbols_on(&app, FIRST),
        1,
        "one undo takes the whole paste back"
    );
}

#[test]
fn paste_lands_where_the_ghost_was_drawn() {
    let (mut app, _other) = app_with_copied_part();
    let copied = app.interaction_state.clipboard.clone().expect("clipboard");
    let relative = copied.payload.fragment().symbols[0].position;
    let cursor = (60.3, 45.1);

    let _ = app.update(Message::Edit(EditMsg::Paste));
    click(&mut app, cursor);

    let grid = signex_app::canvas::grid::pitch_mm_exact(app.ui_state.grid_size_mm);
    let anchor = ghost_anchor(cursor, app.ui_state.snap_enabled, grid);
    let pasted = &doc(&app).symbols[1];
    assert!((pasted.position.x - (anchor.0 + relative.x)).abs() < 1e-9);
    assert!((pasted.position.y - (anchor.1 + relative.y)).abs() < 1e-9);
}

#[test]
fn space_x_y_turn_a_pasted_group() {
    let (mut app, _other) = app_with_copied_part();
    // Add a wire to the copy: a group, not a lone part.
    let wire = Wire {
        uuid: uuid::Uuid::new_v4(),
        start: Point::new(10.16, 20.32),
        end: Point::new(25.4, 20.32),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
    };
    let wire_id = wire.uuid;
    let part = doc(&app).symbols[0].uuid;
    app.document_state
        .active_engine_mut()
        .expect("engine")
        .execute(Command::PlaceWireSegment { wire })
        .expect("wire");
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(part, SelectedKind::Symbol),
        SelectedItem::new(wire_id, SelectedKind::Wire),
    ];
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let _ = app.update(Message::Edit(EditMsg::Paste));
    let fragment = |app: &Signex| {
        app.interaction_state
            .placement
            .as_ref()
            .expect("paste armed")
            .payload
            .fragment()
            .clone()
    };
    let before = fragment(&app);

    let _ = app.update(Message::Edit(EditMsg::RotateSelectedCounterclockwise));
    let turned = fragment(&app);
    assert_eq!(turned.symbols[0].rotation, 90.0);
    assert_ne!(
        (turned.wires[0].start.x, turned.wires[0].start.y),
        (before.wires[0].start.x, before.wires[0].start.y),
        "the wire turned with the group"
    );
    let _ = app.update(Message::Edit(EditMsg::MirrorSelectedX));
    assert!(fragment(&app).symbols[0].mirror_x);
    let _ = app.update(Message::Edit(EditMsg::MirrorSelectedY));
    assert!(fragment(&app).symbols[0].mirror_y);

    // The sheet's own selection did not turn.
    assert_eq!(doc(&app).symbols[0].rotation, 0.0);
}

#[test]
fn a_large_paste_draws_only_its_bounding_box() {
    let (mut app, _other) = app_with_copied_part();
    let many = |n: usize| signex_engine::CopiedSelection {
        payload: signex_engine::PlacementPayload::new(signex_types::schematic::SchematicSheet {
            wires: (0..n)
                .map(|i| Wire {
                    uuid: uuid::Uuid::new_v4(),
                    start: Point::new(i as f64 * 1.27, 0.0),
                    end: Point::new(i as f64 * 1.27, 1.27),
                    stroke_width: 0.0,
                    unknown_columns: Default::default(),
                })
                .collect(),
            ..Default::default()
        }),
        origin: Point::new(0.0, 0.0),
    };
    let limit = signex_app::canvas::placement_ghost::MAX_GHOST_SCENE_ITEMS;

    app.interaction_state.clipboard = Some(many(limit + 1));
    let _ = app.update(Message::Edit(EditMsg::Paste));
    let ghost = &app
        .interaction_state
        .placement
        .as_ref()
        .expect("armed")
        .ghost;
    assert!(ghost.is_bounding_box_only());

    app.interaction_state.clipboard = Some(many(limit));
    let _ = app.update(Message::Edit(EditMsg::Paste));
    let ghost = &app
        .interaction_state
        .placement
        .as_ref()
        .expect("armed")
        .ghost;
    assert!(!ghost.is_bounding_box_only());
}

#[test]
fn smart_paste_keeps_its_fixed_offset_from_the_original() {
    let (mut app, _other) = app_with_copied_part();
    let original = doc(&app).symbols[0].position;
    let content = doc(&app).content_bounds().expect("content");
    let margin = signex_app::canvas::grid::pitch_mm_exact(app.ui_state.grid_size_mm.max(2.54));
    // The old rule: shift right of the sheet content, down by one margin.
    let clipboard_min_x = original.x - 5.08;
    let expect_dx = (content.max_x - clipboard_min_x) + margin;

    let _ = app.update(Message::Edit(EditMsg::SmartPaste));

    assert!(
        app.interaction_state.placement.is_none(),
        "no ghost: it lands at once"
    );
    assert_eq!(doc(&app).symbols.len(), 2);
    let pasted = &doc(&app).symbols[1];
    assert!((pasted.position.x - (original.x + expect_dx)).abs() < 1e-6);
    assert!((pasted.position.y - (original.y + margin)).abs() < 1e-6);
}

#[test]
fn cut_then_paste_still_works() {
    let (mut app, _other) = app_with_copied_part();
    let original = doc(&app).symbols[0].clone();
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(original.uuid, SelectedKind::Symbol)];

    let _ = app.update(Message::Edit(EditMsg::Cut));
    assert!(doc(&app).symbols.is_empty(), "removed at cut time");

    let _ = app.update(Message::Edit(EditMsg::Paste));
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: original.position.x,
        world_y: original.position.y,
    }));
    let back = &doc(&app).symbols;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].reference, original.reference);
    assert!(doc(&app).lib_symbols.contains_key(&back[0].lib_id));
}

#[test]
fn paste_is_refused_on_a_newer_minor_sheet() {
    let (mut app, other) = app_with_copied_part();
    let other_name = other.to_string_lossy().into_owned();
    {
        let engine = app.document_state.engines.get_mut(&other).expect("engine");
        let mut sheet = engine.document().clone();
        sheet.unknown.newer_minor_source = Some("snxsch/1.9".to_owned());
        engine.set_document(sheet);
    }
    show(&mut app, &other_name);

    let _ = app.update(Message::Edit(EditMsg::Paste));

    assert!(app.interaction_state.placement.is_none());
    let notice = app.document_state.error_notice.as_ref().expect("says why");
    assert_eq!(notice.title, "Paste Failed");
    assert!(notice.detail.contains("read-only"), "{}", notice.detail);
    assert!(doc(&app).symbols.is_empty());
}

#[test]
fn a_body_conflict_warns_and_the_ghost_shows_the_sheets_copy() {
    let (mut app, other) = app_with_copied_part();
    let other_name = other.to_string_lossy().into_owned();
    let copied_body = app
        .interaction_state
        .clipboard
        .as_ref()
        .expect("clipboard")
        .payload
        .fragment()
        .lib_symbols
        .values()
        .next()
        .expect("body")
        .clone();
    let mut sheets_body = copied_body.clone();
    sheets_body.pins.clear();
    app.document_state
        .engines
        .get_mut(&other)
        .expect("engine")
        .execute(Command::RegisterLibSymbol {
            lib_symbol: sheets_body.clone(),
        })
        .expect("register");
    show(&mut app, &other_name);
    let _ = signex_app::diagnostics::init_logging();
    let before = signex_app::diagnostics::newest_entry_id().unwrap_or(0);

    let _ = app.update(Message::Edit(EditMsg::Paste));

    let warned = signex_app::diagnostics::recent_entries().iter().any(|e| {
        e.id > before
            && e.message
                .contains("differs from the copy this sheet already holds")
    });
    assert!(warned, "the conflict reaches the Messages panel");
    let session = app.interaction_state.placement.as_ref().expect("armed");
    let ghost_body = session
        .payload
        .fragment()
        .lib_symbols
        .values()
        .next()
        .expect("body");
    assert_eq!(ghost_body.pins.len(), sheets_body.pins.len());
    assert!(copied_body.pins.len() > ghost_body.pins.len());
}
