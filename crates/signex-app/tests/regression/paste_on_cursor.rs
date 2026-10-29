//! #1252 — Paste attaches the copied content to the cursor: Space / X / Y
//! turn it, one click places it as one undo step, and then the session ends.
//! The clipboard carries the library bodies its parts use, so a paste into
//! another sheet brings them along.

use std::path::PathBuf;

use crate::paste_support::{paste, smart_paste};
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
    let (app, other, f) = app_with_copied_part_and_library();
    // `f` owns the temp library; the part's body is already on sheet A.
    std::mem::forget(f);
    (app, other)
}

/// [`app_with_copied_part`], keeping the library fixture for tests that pick
/// another part.
fn app_with_copied_part_and_library() -> (Signex, PathBuf, crate::library_place_symbol::Fixture) {
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
    let _ = app.update(Message::Edit(EditMsg::Copy));
    assert!(app.interaction_state.clipboard.is_some());
    (app, other, f)
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

    paste(&mut app);
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

    paste(&mut app);
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

    paste(&mut app);
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
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.16, 20.32),
        end: Point::new(25.4, 20.32),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
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
    paste(&mut app);
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
                    uuid: uuid::Uuid::now_v7(),
                    start: Point::new(i as f64 * 1.27, 0.0),
                    end: Point::new(i as f64 * 1.27, 1.27),
                    stroke_width: 0.0,
                    unknown_columns: Default::default(),
                    locked: false,
                })
                .collect(),
            ..Default::default()
        }),
        bounds: None,
        origin: Point::new(0.0, 0.0),
    };
    let limit = signex_app::canvas::placement_ghost::MAX_GHOST_SCENE_ITEMS;

    app.interaction_state.clipboard = Some(many(limit + 1));
    paste(&mut app);
    let ghost = &app
        .interaction_state
        .placement
        .as_ref()
        .expect("armed")
        .ghost;
    assert!(ghost.is_bounding_box_only());

    app.interaction_state.clipboard = Some(many(limit));
    paste(&mut app);
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

    smart_paste(&mut app);

    assert!(
        app.interaction_state.placement.is_none(),
        "no ghost: it lands at once"
    );
    assert_eq!(doc(&app).symbols.len(), 2);
    let pasted = &doc(&app).symbols[1];
    assert!((pasted.position.x - (original.x + expect_dx)).abs() < 1e-9);
    assert!((pasted.position.y - (original.y + margin)).abs() < 1e-9);
}

#[test]
fn cut_then_paste_still_works() {
    let (mut app, _other) = app_with_copied_part();
    let original = doc(&app).symbols[0].clone();
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(original.uuid, SelectedKind::Symbol)];

    let _ = app.update(Message::Edit(EditMsg::Cut));
    assert!(doc(&app).symbols.is_empty(), "removed at cut time");

    paste(&mut app);
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

    paste(&mut app);

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

    paste(&mut app);

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

#[test]
fn cut_aborts_and_deletes_nothing_when_the_copy_fails() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.16, 10.16),
        end: Point::new(f64::NAN, 10.16),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let id = wire.uuid;
    app.document_state
        .active_engine_mut()
        .expect("engine")
        .execute(Command::PlaceWireSegment { wire })
        .expect("wire");
    let selection = vec![SelectedItem::new(id, SelectedKind::Wire)];
    app.interaction_state.active_canvas_mut().selected = selection.clone();
    let _ = signex_app::diagnostics::init_logging();
    let before = signex_app::diagnostics::newest_entry_id().unwrap_or(0);

    let _ = app.update(Message::Edit(EditMsg::Cut));

    assert_eq!(doc(&app).wires.len(), 1, "the wire is still on the sheet");
    assert!(app.interaction_state.clipboard.is_none());
    assert_eq!(app.interaction_state.active_canvas().selected, selection);
    let logged = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > before && e.message.contains("Cut cancelled"));
    assert!(logged, "the Messages panel says why");
}

#[test]
fn a_paste_replaces_a_library_read_still_in_flight() {
    let (mut app, _other, f) = app_with_copied_part_and_library();
    let task = app.update(Message::Library(
        signex_app::library::LibraryMessage::PlaceLibraryComponent {
            library_path: f.snxlib.clone(),
            table: "ICs".into(),
            row_id: signex_library::RowId(f.row_id),
        },
    ));

    paste(&mut app);
    for message in crate::library_place_symbol::run(task) {
        let _ = app.update(message);
    }

    let session = app.interaction_state.placement.as_ref().expect("armed");
    assert!(
        session.kind.is_paste(),
        "the late read did not replace the paste"
    );
}

#[test]
fn smart_paste_measures_from_ref_and_value_text_when_they_stick_out() {
    let (mut app, _other) = app_with_copied_part();
    let part = doc(&app).symbols[0].clone();
    {
        let mut sheet = doc(&app).clone();
        let text = sheet.symbols[0].ref_text.as_mut().expect("ref text");
        text.position.x = part.position.x - 30.0;
        app.document_state
            .active_engine_mut()
            .expect("engine")
            .set_document(sheet);
    }
    let _ = app.update(Message::Edit(EditMsg::Copy));
    let content = doc(&app).content_bounds().expect("content");
    let margin = signex_app::canvas::grid::pitch_mm_exact(app.ui_state.grid_size_mm.max(2.54));
    let text_min_x = part.position.x - 30.0 - 1.27;
    assert!(
        text_min_x < part.position.x - 5.08,
        "the text sets the extent"
    );
    let expect_dx = (content.max_x - text_min_x) + margin;

    smart_paste(&mut app);

    let pasted = &doc(&app).symbols[1];
    assert!((pasted.position.x - (part.position.x + expect_dx)).abs() < 1e-9);
}

#[test]
fn duplicate_registers_the_body_offsets_by_5_08_and_is_one_undo_step() {
    let (mut app, _other) = app_with_copied_part();
    let original = doc(&app).symbols[0].clone();

    let _ = app.update(Message::Edit(EditMsg::Duplicate));

    assert!(
        app.interaction_state.placement.is_none(),
        "no ghost: it lands at once"
    );
    assert_eq!(doc(&app).symbols.len(), 2);
    let copy = &doc(&app).symbols[1];
    assert!((copy.position.x - (original.position.x + 5.08)).abs() < 1e-9);
    assert!((copy.position.y - (original.position.y + 5.08)).abs() < 1e-9);
    assert!(doc(&app).lib_symbols.contains_key(&copy.lib_id));
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        doc(&app).symbols.len(),
        1,
        "one undo takes the duplicate back"
    );
}

#[test]
fn two_pastes_from_one_copy_place_two_parts_in_two_undo_steps() {
    let (mut app, _other) = app_with_copied_part();
    for at in [(50.8, 50.8), (76.2, 50.8)] {
        paste(&mut app);
        click(&mut app, at);
    }
    assert_eq!(doc(&app).symbols.len(), 3);
    assert_eq!(doc(&app).lib_symbols.len(), 1, "one body, registered once");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols.len(), 2);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).symbols.len(), 1);
}

#[test]
fn esc_during_a_paste_leaves_nothing_placed() {
    let (mut app, _other) = app_with_copied_part();
    paste(&mut app);
    let _ = app.update(Message::Tool(signex_app::toolbar::ToolMessage::SelectTool(
        signex_app::app::Tool::Select,
    )));
    assert!(app.interaction_state.placement.is_none());
    click(&mut app, (50.8, 50.8));
    assert_eq!(doc(&app).symbols.len(), 1);
}

#[test]
fn a_tab_switch_during_a_paste_leaves_nothing_placed() {
    let (mut app, other) = app_with_copied_part();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    paste(&mut app);

    let _ = app.update(Message::Tab {
        window_id: window,
        msg: signex_app::tab_bar::TabMessage::Select(1),
    });

    assert!(app.interaction_state.placement.is_none());
    click(&mut app, (50.8, 50.8));
    assert_eq!(symbols_on(&app, FIRST), 1);
    assert_eq!(
        app.document_state.engines[&other].document().symbols.len(),
        0
    );
}

#[test]
fn a_paste_into_a_sheet_of_another_project_brings_the_body() {
    use signex_app::app::LoadedProject;
    use signex_types::project::{ProjectData, SheetEntry};
    let (mut app, other) = app_with_copied_part();
    let other_name = other.to_string_lossy().into_owned();
    for (name, file) in [("P1", FIRST), ("P2", other_name.as_str())] {
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: PathBuf::from(format!("{name}.snxprj")),
            data: ProjectData {
                name: name.to_owned(),
                description: None,
                dir: String::new(),
                schematic_root: Some(file.to_owned()),
                pcb_file: None,
                sheets: vec![SheetEntry {
                    name: file.to_owned(),
                    filename: file.to_owned(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    }
    show(&mut app, &other_name);

    paste(&mut app);
    click(&mut app, PLACE_AT);

    let sheet = doc(&app);
    assert_eq!(sheet.symbols.len(), 1);
    assert!(sheet.lib_symbols.contains_key(&sheet.symbols[0].lib_id));
}

#[test]
fn a_press_drag_on_a_selected_original_with_a_paste_on_the_cursor_moves_nothing() {
    use crate::click_click_move::{canvas_event_placing, left_press, screen_at};
    use iced::{event::Event, mouse};
    use signex_app::canvas::CanvasState;

    let (mut app, _other) = app_with_copied_part();
    app.ui_state.snap_enabled = false;
    paste(&mut app);
    assert!(app.interaction_state.placement.is_some());
    assert!(
        !app.interaction_state.active_canvas().selected.is_empty(),
        "arming keeps the originals selected"
    );
    let before = doc(&app).symbols[0].position;
    let start = screen_at(&app, before.x as f32, before.y as f32);
    let end = iced::Point::new(start.x + 40.0, start.y + 25.0);
    let mut state = CanvasState::default();

    let published = [
        canvas_event_placing(&mut app, &mut state, left_press(), start),
        canvas_event_placing(
            &mut app,
            &mut state,
            Event::Mouse(mouse::Event::CursorMoved { position: end }),
            end,
        ),
        canvas_event_placing(
            &mut app,
            &mut state,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            end,
        ),
    ];

    assert!(
        !published
            .iter()
            .flatten()
            .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::MoveSelected { .. }))),
        "no move was published: {published:?}"
    );
    assert_eq!(doc(&app).symbols[0].position, before, "the original stayed");
    assert_eq!(symbols_on(&app, FIRST), 2, "the press placed the paste");
}
