//! A held key runs a cycle or toggle once (Hakan, 2026-11-19): the
//! auto-repeat `KeyPressed` events that follow the first press are swallowed
//! for a catalog command marked `KeyRepeat::Ignored`, and still run for the
//! rest. Every event goes the app's own key path, `Message::Input(KeyInput)`.

use super::*;
use crate::app::{TabInfo, TabKind, Tool, ToolMessage};
use crate::keymap::{
    AppCommandId, CompiledKeymap, Platform, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};
use iced::keyboard::{Key, Modifiers as Held, key::Code, key::Named, key::Physical};
use signex_types::corner::DragWiringMode;

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
    press(
        Key::Character("/".into()),
        Code::Slash,
        Held::empty(),
        repeat,
    )
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

/// A booted app with one empty schematic sheet, bound to `bindings`.
fn app_with(bindings: &[(&str, &str)]) -> Signex {
    let path = std::path::PathBuf::from("key-repeat.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
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

/// A command that is not a cycle or toggle still runs on every repeat.
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

/// A chord that ends on a cycle or toggle fires on its real strokes, and a
/// held last stroke does not fire it again.
#[test]
fn a_chord_to_a_toggle_fires_once_and_ignores_the_held_last_key() {
    let mut app = app_with(&[("G V", "toggle_visible_grid")]);
    send(&mut app, letter("g", Code::KeyG, false));
    assert!(
        matches!(
            routed(&app, &letter("v", Code::KeyV, false)),
            Some(Message::Ui(UiMsg::KeymapStroke { .. }))
        ),
        "the chord's last stroke reaches the keymap"
    );
    assert!(
        routed(&app, &letter("v", Code::KeyV, true)).is_none(),
        "a held last stroke of the chord is swallowed"
    );
    // The chord prefix itself is no command: its repeat still routes.
    let app = app_with(&[("G V", "toggle_visible_grid")]);
    assert!(
        routed(&app, &letter("g", Code::KeyG, true)).is_some(),
        "a held chord prefix still reaches the keymap"
    );
}
