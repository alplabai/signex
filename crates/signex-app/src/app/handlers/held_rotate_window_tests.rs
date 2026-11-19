//! A held rotate key is judged in the window it is pressed in
//! (`docs/DECISIONS.md`, "Held keys"): it runs once while the rotate
//! handler bends a run for that window, and repeats while it turns a
//! selection. Every key goes the app's own path, `Message::Input(KeyInput)`.

use iced::keyboard::{self, Key, Modifiers, key::Code, key::Named, key::Physical};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};

use super::pcb_placement_window_tests::{click, in_main, rig};
use super::pcb_tool::BoardToolSession;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::WindowKind;
use crate::app::{InputMsg, Message, Signex, TabInfo, TabKind, Tool};

const REPEATS: usize = 5;

fn shift_space(repeat: bool) -> keyboard::Event {
    let key = Key::Named(Named::Space);
    keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: Physical::Code(Code::Space),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::SHIFT,
        text: None,
        repeat,
    }
}

fn send(app: &mut Signex, window: iced::window::Id, event: keyboard::Event) {
    let _ = app.update(Message::Input(InputMsg::KeyInput { window, event }));
}

/// Hold Shift+Space in `window`: the first press steps the corner mode and
/// no repeat steps it again.
fn assert_held_shift_space_bends_once(app: &mut Signex, window: iced::window::Id) {
    let before = app.interaction_state.corner_rule;
    send(app, window, shift_space(false));
    let stepped = app.interaction_state.corner_rule;
    assert_ne!(stepped.mode, before.mode, "the first press bends the run");
    for _ in 0..REPEATS {
        send(app, window, shift_space(true));
        assert_eq!(app.interaction_state.corner_rule, stepped, "held: once");
    }
}

/// (a) A Track run in the main window: a held Shift+Space there bends it
/// once.
#[test]
fn a_held_shift_space_bends_a_main_window_track_run_once() {
    let mut rig = rig();
    let main = rig.app.ui_state.main_window_id.expect("main window");
    let _ = rig
        .app
        .update(Message::Menu(crate::menu_bar::MenuMessage::command(
            "place_board_track",
        )));
    in_main(&mut rig.app, click(12.0, 10.0));
    assert!(rig.app.is_routing_board_track_in(None), "the run is open");
    assert_held_shift_space_bends_once(&mut rig.app, main);
}

/// (b) A Track run open in an undocked board window while the main window
/// shows another board: a held Shift+Space there bends it once. The session
/// is set as the Track tool leaves it after a first click in that window.
#[test]
fn a_held_shift_space_bends_an_undocked_track_run_once() {
    let mut rig = rig();
    let window = rig.window;
    rig.app.interaction_state.pcb_tool = Some(BoardToolSession {
        tool: BoardPlaceTool::Track,
        path: rig.a.clone(),
        window: Some(window),
        points: vec![signex_types::pcb::Point::new(12.0, 10.0)],
        cursor: None,
        route: None,
        router: None,
        paused: None,
    });
    assert!(
        rig.app.is_routing_board_track_in(Some(window)),
        "open there"
    );
    assert!(!rig.app.is_routing_board_track_in(None), "not in the main");
    assert_held_shift_space_bends_once(&mut rig.app, window);
}

fn tab(path: &std::path::Path) -> TabInfo {
    TabInfo {
        title: "t".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

/// A main window drawing a wire run on sheet A, and sheet B, with one symbol
/// selected, in an undocked window; returns the app, B's window and the
/// symbol.
fn wire_run_beside_an_undocked_selection() -> (Signex, iced::window::Id, uuid::Uuid) {
    let (a, b) = (
        std::path::PathBuf::from("a.snxsch"),
        std::path::PathBuf::from("b.snxsch"),
    );
    let symbol = uuid::Uuid::now_v7();
    let sheet_b = SchematicSheet {
        symbols: vec![Symbol {
            uuid: symbol,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: Point::new(50.8, 50.8),
            ..Default::default()
        }],
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    for (path, sheet) in [(&a, SchematicSheet::default()), (&b, sheet_b)] {
        let engine = signex_engine::Engine::new(sheet).expect("engine");
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(tab(path));
    }
    // B undocked the way the PCB rig undocks a board: its window and canvas.
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: b,
            title: "b".into(),
        },
    );
    let mut canvas = crate::canvas::CanvasSlot::new();
    canvas.selected = vec![SelectedItem::new(symbol, SelectedKind::Symbol)];
    app.interaction_state.canvases.insert(window, canvas);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(a);
    app.interaction_state.current_tool = Tool::Wire;
    let _ = app.update(Message::CanvasEvent(crate::canvas::CanvasEvent::Clicked {
        world_x: 25.4,
        world_y: 25.4,
    }));
    assert!(app.is_drawing_wire_run(), "the main window's run is open");
    (app, window, symbol)
}

fn rotation(app: &Signex, path: &str, symbol: uuid::Uuid) -> f64 {
    let engine = &app.document_state.engines[std::path::Path::new(path)];
    (engine.document().symbols.iter())
        .find(|s| s.uuid == symbol)
        .expect("the symbol")
        .rotation
}

/// (c) A wire run open in the main window and a selection in an undocked
/// window: the rotate handler bends the open wire run, which is app-wide,
/// for a key from any window (#2372), so a held Shift+Space there bends it
/// once and the selection is not turned. The held-key rule agrees with the
/// handler rather than turning the selection on every repeat.
#[test]
fn a_held_shift_space_beside_a_main_wire_run_bends_it_once() {
    let (mut app, window, symbol) = wire_run_beside_an_undocked_selection();
    assert_held_shift_space_bends_once(&mut app, window);
    assert_eq!(rotation(&app, "b.snxsch", symbol), 0.0, "not turned");
}

/// (d) A wire run open in the main window and a footprint selected in an
/// undocked board window: the rotate handler takes the board path there and
/// never reaches the wire run, so a held Shift+Space turns the footprint on
/// every repeat (#2553).
#[test]
fn a_held_shift_space_in_a_board_window_beside_a_wire_run_keeps_turning() {
    let mut rig = rig();
    let window = rig.window;
    let u1 = uuid::Uuid::from_u128(0x10);
    super::pcb_placement_window_tests::in_window(&mut rig.app, window, click(10.0, 10.0));
    // The main window's wire run, as the Wire tool leaves it mid-run.
    rig.app.interaction_state.current_tool = Tool::Wire;
    rig.app.interaction_state.wire_drawing = true;
    assert!(rig.app.is_drawing_wire_run(), "a wire run is open");
    let turn = |app: &Signex| {
        (app.document_state
            .pcb_board(&rig.a)
            .expect("board")
            .footprints
            .iter())
        .find(|f| f.uuid == u1)
        .expect("U1")
        .rotation
    };
    send(&mut rig.app, window, shift_space(false));
    let mut last = turn(&rig.app);
    assert_ne!(last, 0.0, "the first press turns U1");
    for _ in 0..REPEATS {
        send(&mut rig.app, window, shift_space(true));
        let now = turn(&rig.app);
        assert_ne!(now, last, "held: U1 turns on every repeat");
        last = now;
    }
}
