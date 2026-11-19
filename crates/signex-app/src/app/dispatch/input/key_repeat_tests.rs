//! A held key runs a run-once command once (`docs/DECISIONS.md`, "Held
//! keys"): the auto-repeat `KeyPressed` events after the first press are
//! swallowed for a catalog command marked `KeyRepeat::Ignored`, still run for
//! the rest, and never abandon a pending chord. Every event goes the app's own
//! key path, `Message::Input(KeyInput)`.

use super::*;
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

/// A booted app showing `sheet`, bound to `bindings` only.
fn app_with_sheet(bindings: &[(&str, &str)], sheet: SchematicSheet) -> Signex {
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

/// A held flip key flips the selected symbol once: one undo step.
#[test]
fn a_held_flip_key_flips_the_selection_once() {
    let uuid = uuid::Uuid::now_v7();
    let sheet = SchematicSheet {
        symbols: vec![Symbol {
            uuid,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: Point::new(50.0, 50.0),
            ..Default::default()
        }],
        ..empty_sheet()
    };
    let mut app = app_with_sheet(&[("L", "flip_selected_x")], sheet);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(uuid, SelectedKind::Symbol)];
    let mirror = |app: &Signex| {
        let engine = app.document_state.active_engine().expect("engine");
        let symbol = &engine.document().symbols[0];
        (symbol.mirror_x, symbol.mirror_y)
    };
    let before = mirror(&app);
    send(&mut app, letter("l", Code::KeyL, false));
    let flipped = mirror(&app);
    assert_ne!(flipped, before, "the first press flips");
    for _ in 0..REPEATS {
        send(&mut app, letter("l", Code::KeyL, true));
        assert_eq!(mirror(&app), flipped, "a repeat must not flip again");
    }
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(mirror(&app), before, "one undo takes the flip back");
    let engine = app.document_state.active_engine().expect("engine");
    assert!(!engine.can_undo(), "the held key made one undo step");
}
