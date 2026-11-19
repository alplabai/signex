//! A held key runs some commands once (`docs/DECISIONS.md`, "Held keys"):
//! the auto-repeat `KeyPressed` events after the first press are swallowed
//! when `key_repeat::runs_on_key_repeat` says so, still run for the
//! rest, and never abandon a pending chord. Every event goes the app's own
//! key path, `Message::Input(KeyInput)`.

use super::*;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, grid};
use crate::app::{EditMsg, TabInfo, TabKind, Tool, ToolMessage};
use crate::keymap::{
    AppCommandId, CompiledKeymap, Platform, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};
use iced::keyboard::{Key, Modifiers as Held, key::Code, key::Named, key::Physical};
use signex_types::corner::DragWiringMode;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};

const REPEATS: usize = 5;

fn press(key: Key, code: Code, held: Held, repeat: bool) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers: held,
        text: None,
        repeat,
    }
}

fn ctrl_space(repeat: bool) -> keyboard::Event {
    press(Key::Named(Named::Space), Code::Space, Held::CTRL, repeat)
}

fn shift_space(repeat: bool) -> keyboard::Event {
    press(Key::Named(Named::Space), Code::Space, Held::SHIFT, repeat)
}

fn slash(repeat: bool) -> keyboard::Event {
    letter("/", Code::Slash, repeat)
}

fn letter(c: &str, code: Code, repeat: bool) -> keyboard::Event {
    press(Key::Character(c.into()), code, Held::empty(), repeat)
}

fn send(app: &mut Signex, event: keyboard::Event) {
    let window = app.ui_state.main_window_id.expect("main window");
    let _ = app.update(Message::Input(InputMsg::KeyInput { window, event }));
}

fn routed(app: &Signex, event: &keyboard::Event) -> Option<Message> {
    let window = app.ui_state.main_window_id.expect("main window");
    app.route_key(window, event)
}

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// A booted app, with its default keymap, showing `sheet`.
fn showing(sheet: SchematicSheet) -> Signex {
    let path = std::path::PathBuf::from("key-repeat.snxsch");
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "key-repeat".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

/// A sheet with one symbol, R1, selected.
fn showing_selected_symbol() -> Signex {
    let uuid = uuid::Uuid::now_v7();
    let sheet = SchematicSheet {
        symbols: vec![Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: Point::new(50.8, 50.8),
            ..Default::default()
        }],
        ..empty_sheet()
    };
    let mut app = showing(sheet);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    app
}

fn symbol(app: &Signex) -> Symbol {
    let engine = app.document_state.active_engine().expect("engine");
    engine.document().symbols[0].clone()
}

fn can_undo(app: &Signex) -> bool {
    app.document_state
        .active_engine()
        .expect("engine")
        .can_undo()
}

/// A booted app showing `sheet`, bound to `bindings` only.
fn app_with_sheet(bindings: &[(&str, &str)], sheet: SchematicSheet) -> Signex {
    let mut app = showing(sheet);
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: bindings
            .iter()
            .map(|(trigger, id)| ShortcutBinding {
                action: ShortcutBindingAction::Command(AppCommandId::new(*id).unwrap()),
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                platform: None,
            })
            .collect(),
    };
    app.ui_state.active_keymap =
        std::sync::Arc::new(CompiledKeymap::compile_for(&profile, Platform::MacOs));
    app
}

fn app_with(bindings: &[(&str, &str)]) -> Signex {
    app_with_sheet(bindings, empty_sheet())
}

/// The booted keymap (Altium on every platform) binds ⌃Space to
/// `cycle_wiring_mode`; holding it steps the drag wiring mode once.
#[test]
fn a_held_ctrl_space_steps_the_wiring_mode_once() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    assert_eq!(app.interaction_state.drag_wiring, DragWiringMode::Auto);
    send(&mut app, ctrl_space(false));
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::HorizontalFirst
    );
    for _ in 0..REPEATS {
        send(&mut app, ctrl_space(true));
    }
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::HorizontalFirst,
        "auto-repeat must not cycle the mode"
    );
    // The next real press steps again.
    send(&mut app, ctrl_space(false));
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::VerticalFirst
    );
}

/// While a wire is drawn, holding the corner-mode or posture key steps the
/// corner rule once.
#[test]
fn held_corner_keys_step_the_corner_rule_once_while_drawing() {
    let mut app = app_with(&[
        ("Shift+SPACE", "cycle_wire_bus_graphic_mode"),
        ("/", "switch_segment_posture"),
    ]);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    for key in [shift_space, slash] {
        let before = app.interaction_state.corner_rule;
        send(&mut app, key(false));
        let stepped = app.interaction_state.corner_rule;
        assert_ne!(stepped, before, "the first press steps the rule");
        for _ in 0..REPEATS {
            send(&mut app, key(true));
        }
        assert_eq!(
            app.interaction_state.corner_rule, stepped,
            "auto-repeat must not step the rule again"
        );
    }
}

/// A command that is not a cycle, toggle or flip still runs on every repeat.
#[test]
fn a_held_zoom_key_still_repeats() {
    let app = app_with(&[("PGUP", "zoom_in_at_cursor")]);
    let page_up = |repeat| {
        press(
            Key::Named(Named::PageUp),
            Code::PageUp,
            Held::empty(),
            repeat,
        )
    };
    for repeat in [false, true] {
        assert!(
            matches!(
                routed(&app, &page_up(repeat)),
                Some(Message::Ui(UiMsg::KeymapStroke { .. }))
            ),
            "repeat {repeat}: the zoom stroke reaches the keymap"
        );
    }
}

/// `G` is a run-once command and the prefix of `G V`: pressing it waits for
/// the chord. Holding it must neither drop that wait nor fire the command
/// early; the timeout then runs it exactly once.
#[test]
fn a_held_chord_prefix_runs_its_command_once_at_the_timeout() {
    let mut app = app_with(&[
        ("G", "cycle_wiring_mode"),
        ("G V", "open_net_color_palette"),
    ]);
    send(&mut app, letter("g", Code::KeyG, false));
    let generation = app
        .ui_state
        .keymap_chord_wait
        .expect("G waits on G V")
        .generation;
    for _ in 0..REPEATS {
        send(&mut app, letter("g", Code::KeyG, true));
    }
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::Auto,
        "nothing fires while the key is held"
    );
    let _ = app.update(Message::Ui(UiMsg::KeymapChordTimedOut { generation }));
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::HorizontalFirst,
        "the timeout runs the held command exactly once"
    );
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
    assert!(app.ui_state.keymap_chord_wait.is_none());
}

/// `G V` toggles the grid and `V` alone is a run-once command: the chord
/// fires once on its real strokes, and holding its last key runs neither the
/// chord again nor `V`'s own command.
#[test]
fn a_chord_fires_once_and_its_held_last_key_runs_nothing() {
    // `V` alone is bound only as a probe: a held V that ran anything would
    // step the wiring mode.
    let mut app = app_with(&[("G V", "toggle_visible_grid"), ("V", "cycle_wiring_mode")]);
    let grid = app.ui_state.grid_visible;
    send(&mut app, letter("g", Code::KeyG, false));
    send(&mut app, letter("v", Code::KeyV, false));
    assert_ne!(
        app.ui_state.grid_visible, grid,
        "the chord toggled the grid"
    );
    for _ in 0..REPEATS {
        send(&mut app, letter("v", Code::KeyV, true));
        assert_ne!(app.ui_state.grid_visible, grid, "the chord ran once");
    }
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::Auto,
        "a held V does not run V's own command"
    );
}

/// Only the keymap's own run-once swallow keeps a pending chord: a repeat
/// that another consumer (here the open command palette) swallows abandons
/// it, as any other non-keymap key does (#1378).
#[test]
fn a_repeat_another_consumer_swallows_still_cancels_the_chord() {
    let mut app = app_with(&[
        ("G", "cycle_wiring_mode"),
        ("G V", "open_net_color_palette"),
    ]);
    send(&mut app, letter("g", Code::KeyG, false));
    assert!(app.ui_state.keymap_chord_wait.is_some(), "G waits on G V");
    app.ui_state.command_palette.open = true;
    let event = letter("g", Code::KeyG, true);
    assert!(routed(&app, &event).is_none(), "the palette swallows it");
    send(&mut app, event);
    assert!(app.ui_state.keymap_chord_wait.is_none());
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
}

/// Hold `key` on the selected symbol: the first press mirrors it, no repeat
/// mirrors it again, and one undo takes the whole hold back.
fn assert_held_key_mirrors_once(mut app: Signex, key: fn(bool) -> keyboard::Event) {
    let mirror = |app: &Signex| {
        let symbol = symbol(app);
        (symbol.mirror_x, symbol.mirror_y)
    };
    let before = mirror(&app);
    send(&mut app, key(false));
    let mirrored = mirror(&app);
    assert_ne!(mirrored, before, "the first press mirrors");
    for _ in 0..REPEATS {
        send(&mut app, key(true));
        assert_eq!(mirror(&app), mirrored, "a repeat must not mirror again");
    }
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(mirror(&app), before, "one undo takes the mirror back");
    assert!(!can_undo(&app), "the held key made one undo step");
}

/// A held flip key flips the selected symbol once: one undo step.
#[test]
fn a_held_flip_key_flips_the_selection_once() {
    let mut app = showing_selected_symbol();
    let bound = app_with(&[("L", "flip_selected_x")]);
    app.ui_state.active_keymap = bound.ui_state.active_keymap.clone();
    assert_held_key_mirrors_once(app, |repeat| letter("l", Code::KeyL, repeat));
}

/// The default profile's X (`mirror_x`), held, mirrors the selection once.
#[test]
fn a_held_mirror_key_mirrors_the_selection_once() {
    assert_held_key_mirrors_once(showing_selected_symbol(), |repeat| {
        letter("x", Code::KeyX, repeat)
    });
}

/// The default profile's Shift+Space (`rotate_clockwise`), held mid wire run,
/// steps the corner mode once: the run is what it turns (#2372).
#[test]
fn a_held_shift_space_mid_wire_steps_the_corner_mode_once() {
    let mut app = app_with_tool(Tool::Wire);
    let start = grid(0.0, 0.0);
    click(&mut app, (start.x, start.y));
    assert!(app.has_open_run_in(None), "the run is open");
    let before = app.interaction_state.corner_rule;
    send(&mut app, shift_space(false));
    let stepped = app.interaction_state.corner_rule;
    assert_ne!(stepped.mode, before.mode, "the first press steps the mode");
    for _ in 0..REPEATS {
        send(&mut app, shift_space(true));
        assert_eq!(app.interaction_state.corner_rule, stepped, "held: once");
    }
}

/// With no run open, the same held Shift+Space turns the selected symbol on
/// every repeat.
#[test]
fn a_held_shift_space_on_a_selection_rotates_on_every_repeat() {
    let mut app = showing_selected_symbol();
    assert!(!app.has_open_run_in(None));
    let mut rotation = symbol(&app).rotation;
    for repeat in std::iter::once(false).chain([true; REPEATS]) {
        send(&mut app, shift_space(repeat));
        let turned = symbol(&app).rotation;
        assert_ne!(turned, rotation, "repeat {repeat}: the symbol turned");
        rotation = turned;
    }
}

/// Hold `key` mid wire run in `app` while the mouse moves: it fixes one
/// vertex, not one per repeat.
fn assert_held_key_adds_one_vertex(mut app: Signex, key: impl Fn(bool) -> keyboard::Event) {
    let start = grid(0.0, 0.0);
    click(&mut app, (start.x, start.y));
    let move_cursor = |app: &mut Signex, i: f64| {
        // Each repeat's cursor is a new corner: a repeat that ran would fix it.
        let cursor = grid(10.0 + i, 3.0 * i);
        app.ui_state.cursor_x = cursor.x;
        app.ui_state.cursor_y = cursor.y;
        app.ui_state.cursor_document = app.document_state.active_path.clone();
    };
    move_cursor(&mut app, 0.0);
    let points = app.interaction_state.wire_points.clone();
    send(&mut app, key(false));
    let added = app.interaction_state.wire_points.clone();
    let wires = wire_count(&app);
    assert_ne!(added, points, "the first press fixes a vertex");
    for i in 1..=REPEATS {
        move_cursor(&mut app, i as f64);
        send(&mut app, key(true));
        assert_eq!(app.interaction_state.wire_points, added, "held: once");
        assert_eq!(wire_count(&app), wires, "held: one segment");
    }
}

/// The default profile's Insert (`copy_attributes_or_add_vertex`), held mid
/// wire run, adds one vertex.
#[test]
fn a_held_insert_mid_wire_adds_one_vertex() {
    assert_held_key_adds_one_vertex(app_with_tool(Tool::Wire), |repeat| {
        press(
            Key::Named(Named::Insert),
            Code::Insert,
            Held::empty(),
            repeat,
        )
    });
}

/// The Altium profile on macOS, which has no Insert key, also binds ⌥I to
/// add vertex; held, it adds one vertex like Insert.
#[test]
fn a_held_option_i_on_macos_adds_one_vertex() {
    let mut app = app_with_tool(Tool::Wire);
    app.ui_state.active_keymap = altium_on_macos();
    assert_held_key_adds_one_vertex(app, |repeat| {
        press(Key::Character("i".into()), Code::KeyI, Held::ALT, repeat)
    });
}

/// The built-in Altium profile as macOS compiles it.
fn altium_on_macos() -> std::sync::Arc<CompiledKeymap> {
    let set = crate::keymap::ShortcutProfileSet::built_ins().expect("built-in profiles");
    let altium = set.profile("altium").expect("the Altium profile");
    std::sync::Arc::new(CompiledKeymap::compile_for(altium, Platform::MacOs))
}

/// The Altium profile on macOS also cycles the drag wiring mode on ⇧Tab
/// (macOS may take ⌃Space); held, it steps the mode once.
#[test]
fn a_held_shift_tab_on_macos_steps_the_wiring_mode_once() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.active_keymap = altium_on_macos();
    let shift_tab = |repeat| press(Key::Named(Named::Tab), Code::Tab, Held::SHIFT, repeat);
    send(&mut app, shift_tab(false));
    assert_eq!(
        app.interaction_state.drag_wiring,
        DragWiringMode::HorizontalFirst
    );
    for _ in 0..REPEATS {
        send(&mut app, shift_tab(true));
        assert_eq!(
            app.interaction_state.drag_wiring,
            DragWiringMode::HorizontalFirst,
            "held: once"
        );
    }
}

fn wire_count(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .wires
        .len()
}
