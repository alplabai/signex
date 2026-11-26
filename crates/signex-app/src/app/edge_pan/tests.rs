//! #2872 — edge pan through `Signex::update`: the pointer in the right band
//! with a tool armed pans each canvas kind right on a tick, and what follows
//! the pointer reads the point the move brought under it; outside the band,
//! with nothing armed, off the canvas or with the preference off, nothing
//! pans. A test stands in for the canvas event by writing the pointer to
//! the canvas's link, as the widget does on every event.

use std::path::PathBuf;

use iced::{Point, Size};

use crate::app::handlers::canvas::placement_tests::app_with_tool;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{FootprintDocument, Message, Signex, SymbolDocument, TabInfo, TabKind, Tool};
use crate::canvas::CanvasEvent;
use crate::library::LibraryMessage;
use crate::library::messages::{FootprintEditorMsg, PrimitiveEdit, SymbolEditorMsg, SymbolToolMsg};

const SIZE: Size = Size {
    width: 800.0,
    height: 600.0,
};
/// One pixel in from the right edge, halfway down.
const RIGHT_BAND: Point = Point { x: 799.0, y: 300.0 };
const MIDDLE: Point = Point { x: 400.0, y: 300.0 };

fn tick(app: &mut Signex) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::EdgePanTicked));
}

/// The sheet point under `at` on the active sheet's canvas.
fn sheet_point(app: &Signex, at: Point) -> Point {
    app.interaction_state.active_canvas().screen_to_world(at)
}

fn sheet_with_pointer(tool: Tool, at: Option<Point>) -> Signex {
    let app = app_with_tool(tool);
    app.interaction_state
        .active_canvas()
        .edge_pan
        .observe(at, SIZE);
    app
}

#[test]
fn a_tool_armed_with_the_pointer_in_the_right_band_pans_the_sheet_right_on_a_tick() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));
    let before = sheet_point(&app, RIGHT_BAND);
    assert!(app.edge_pan_motion().is_some(), "the timer runs");

    tick(&mut app);

    let after = sheet_point(&app, RIGHT_BAND);
    assert!(after.x > before.x, "{after:?} is not right of {before:?}");
    assert!(
        (after.y - before.y).abs() < 1e-4,
        "the view moved up or down"
    );
}

#[test]
fn the_placement_point_follows_the_pointer_as_the_sheet_pans() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));

    tick(&mut app);
    tick(&mut app);

    let under = sheet_point(&app, RIGHT_BAND);
    assert!((app.ui_state.cursor_x - f64::from(under.x)).abs() < 1e-3);
    assert!((app.ui_state.cursor_y - f64::from(under.y)).abs() < 1e-3);
}

#[test]
fn outside_the_band_nothing_pans() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(MIDDLE));
    let before = sheet_point(&app, MIDDLE);
    assert!(
        app.edge_pan_motion().is_none(),
        "no timer away from the edges"
    );

    tick(&mut app);

    assert_eq!(sheet_point(&app, MIDDLE), before);
}

#[test]
fn with_no_tool_and_no_drag_nothing_pans() {
    let mut app = sheet_with_pointer(Tool::Select, Some(RIGHT_BAND));
    let before = sheet_point(&app, RIGHT_BAND);
    assert!(app.edge_pan_motion().is_none());

    tick(&mut app);

    assert_eq!(sheet_point(&app, RIGHT_BAND), before);
}

#[test]
fn the_pointer_leaving_the_canvas_stops_the_panning_and_the_tool_stays_armed() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));
    tick(&mut app);
    // Over a panel the canvas no longer has the pointer.
    app.interaction_state
        .active_canvas()
        .edge_pan
        .observe(None, SIZE);
    let before = sheet_point(&app, RIGHT_BAND);
    assert!(app.edge_pan_motion().is_none(), "the timer stops");

    tick(&mut app);

    assert_eq!(sheet_point(&app, RIGHT_BAND), before);
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
}

#[test]
fn with_the_preference_off_nothing_pans() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));
    app.ui_state.edge_pan.enabled = false;
    let before = sheet_point(&app, RIGHT_BAND);
    assert!(app.edge_pan_motion().is_none());

    tick(&mut app);

    assert_eq!(sheet_point(&app, RIGHT_BAND), before);
}

#[test]
fn the_edge_pan_drafts_are_editing_page_drafts() {
    use crate::app::PreferencesMsg;
    use crate::app::edge_pan::EdgePanSpeed;
    use crate::preferences::{PrefMsg, PrefNav};
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftEdgePanEnabled(false),
    )));
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        PrefMsg::DraftEdgePanSpeed(EdgePanSpeed::Fast),
    )));
    assert!(!app.ui_state.preferences_draft_edge_pan.enabled);
    assert_eq!(
        app.ui_state.preferences_draft_edge_pan.speed,
        EdgePanSpeed::Fast
    );
    assert!(app.ui_state.preferences_page_differs(PrefNav::Editing));
    assert!(app.ui_state.edge_pan.enabled, "a draft is not saved");
}

#[test]
fn a_board_tool_with_the_pointer_in_the_right_band_pans_the_board_right_and_its_cursor_follows() {
    let mut app = board_app();
    let canvas = &app.interaction_state.pcb_canvas;
    canvas.edge_pan.observe(Some(RIGHT_BAND), SIZE);
    let before = canvas.screen_to_world(RIGHT_BAND, iced::Rectangle::default());

    tick(&mut app);

    let canvas = &app.interaction_state.pcb_canvas;
    let after = canvas.screen_to_world(RIGHT_BAND, iced::Rectangle::default());
    assert!(after.x > before.x, "{after:?} is not right of {before:?}");
    assert!(
        (app.ui_state.cursor_x - after.x).abs() < 1e-3,
        "the readout follows"
    );
    let tool_cursor = app
        .interaction_state
        .pcb_tool
        .as_ref()
        .and_then(|session| session.cursor);
    assert!(
        tool_cursor.is_some(),
        "the tool's ghost follows the pointer"
    );
}

/// An app with no first-run card over the canvas.
fn bare_app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app
}

/// An empty board tab, active, with Track armed.
fn board_app() -> Signex {
    let mut app = bare_app();
    let board = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "footprints": []
    }))
    .expect("an empty board");
    app.open_pcb_tab(PathBuf::from("edge.snxpcb"), "edge".into(), board);
    app.arm_board_tool(BoardPlaceTool::Track);
    assert!(app.interaction_state.pcb_tool.is_some(), "precondition");
    app
}

/// A symbol editor tab, active, through `Signex::update` armed with Add Pin.
fn symbol_app() -> (Signex, PathBuf) {
    let path = PathBuf::from("edge.snxsym");
    let mut app = bare_app();
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "s".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin)),
    }));
    (app, path)
}

#[test]
fn a_symbol_tool_with_the_pointer_in_the_right_band_pans_the_symbol_right() {
    let (mut app, path) = symbol_app();
    let editor = &app.document_state.symbol_editors[&path];
    assert!(editor.edge_pan_armed(), "precondition: Add Pin is armed");
    let before = editor.gui.camera.offset;
    app.interaction_state
        .symbol_edge_pan
        .observe(Some(RIGHT_BAND), SIZE);

    tick(&mut app);

    let editor = &app.document_state.symbol_editors[&path];
    assert!(
        editor.gui.camera.offset.x < before.x,
        "the symbol did not move left"
    );
    let under = editor
        .gui
        .camera
        .screen_to_world(RIGHT_BAND, iced::Rectangle::default());
    let (x, _) = editor
        .gui
        .cursor_mm
        .expect("the readout follows the pointer");
    assert!((x - f64::from(under.x)).abs() < 1e-3);
}

/// A footprint editor tab, active, through `Signex::update` armed with Place
/// Pad.
fn footprint_app() -> (Signex, PathBuf) {
    let path = PathBuf::from("edge.snxfpt");
    let mut app = bare_app();
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetPadsTool(
            crate::library::editor::footprint::PadsTool::PlacePad,
        )),
    }));
    assert!(
        app.document_state.footprint_editors[&path]
            .state
            .edge_pan_armed(),
        "precondition: Place Pad is armed"
    );
    (app, path)
}

#[test]
fn a_footprint_tool_with_the_pointer_in_the_right_band_owes_its_canvas_a_pan_right() {
    let (mut app, path) = footprint_app();
    let editor = &app.document_state.footprint_editors[&path];
    editor.state.edge_pan.observe(Some(RIGHT_BAND), SIZE);

    tick(&mut app);
    tick(&mut app);

    let (dx, dy) = app.document_state.footprint_editors[&path]
        .state
        .edge_pan
        .owed_pan();
    assert!(dx < 0.0, "the footprint is not owed a move left: {dx}");
    assert_eq!(dy, 0.0);
}

fn focus_lost(app: &mut Signex) {
    let window = iced::window::Id::unique();
    let _ = app.update(Message::Window(crate::app::WindowMsg::WindowUnfocused(
        window,
    )));
}

/// Alt+Tab with the pointer in the band: no tick pans any canvas until the
/// canvas sees the pointer again (#2872).
#[test]
fn losing_the_focus_stops_the_panning_on_every_canvas() {
    let mut sheet = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));
    assert!(sheet.edge_pan_motion().is_some(), "precondition: sheet");
    focus_lost(&mut sheet);
    assert!(sheet.edge_pan_motion().is_none(), "the sheet still pans");
    let before = sheet_point(&sheet, RIGHT_BAND);
    tick(&mut sheet);
    assert_eq!(sheet_point(&sheet, RIGHT_BAND), before);

    let mut board = board_app();
    board
        .interaction_state
        .pcb_canvas
        .edge_pan
        .observe(Some(RIGHT_BAND), SIZE);
    assert!(board.edge_pan_motion().is_some(), "precondition: board");
    focus_lost(&mut board);
    assert!(board.edge_pan_motion().is_none(), "the board still pans");

    let (mut symbol, _) = symbol_app();
    symbol
        .interaction_state
        .symbol_edge_pan
        .observe(Some(RIGHT_BAND), SIZE);
    assert!(symbol.edge_pan_motion().is_some(), "precondition: symbol");
    focus_lost(&mut symbol);
    assert!(symbol.edge_pan_motion().is_none(), "the symbol still pans");

    let (mut footprint, path) = footprint_app();
    footprint.document_state.footprint_editors[&path]
        .state
        .edge_pan
        .observe(Some(RIGHT_BAND), SIZE);
    assert!(
        footprint.edge_pan_motion().is_some(),
        "precondition: footprint"
    );
    focus_lost(&mut footprint);
    assert!(
        footprint.edge_pan_motion().is_none(),
        "the footprint still pans"
    );
    tick(&mut footprint);
    assert_eq!(
        footprint.document_state.footprint_editors[&path]
            .state
            .edge_pan
            .owed_pan(),
        (0.0, 0.0)
    );
}

/// A dialog a key opened over the canvas has the pointer: no tick pans,
/// though the canvas last saw the pointer in the band (#2872).
#[test]
fn a_dialog_over_the_canvas_stops_the_panning() {
    let mut app = sheet_with_pointer(Tool::Wire, Some(RIGHT_BAND));
    app.ui_state.preferences_open = true;
    let before = sheet_point(&app, RIGHT_BAND);
    assert!(app.edge_pan_motion().is_none(), "the timer stops");

    tick(&mut app);

    assert_eq!(sheet_point(&app, RIGHT_BAND), before);
}
