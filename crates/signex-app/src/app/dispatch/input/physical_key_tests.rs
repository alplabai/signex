//! The physical-key fallback of the keymap claim (#1645, decision D1): the
//! logical stroke keeps priority, and the US-QWERTY key of the same
//! physical position is retried only on a miss with Ctrl, Cmd or Alt held,
//! or, for a non-Latin layout (Cyrillic, Greek), on any miss (#2095).

use super::*;
use crate::keymap::{
    AppCommandId, CompiledKeymap, Platform, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};
use iced::keyboard::{Key, Modifiers as Held, key::Code, key::Physical};

fn app_with(platform: Platform, bindings: &[(&str, &str)]) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
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
        std::sync::Arc::new(CompiledKeymap::compile_for(&profile, platform));
    app
}

fn press(logical: &str, code: Code, held: Held) -> keyboard::Event {
    let key = Key::Character(logical.into());
    keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers: held,
        text: None,
        repeat: false,
    }
}

/// The stroke the keymap claim forwards for `event`.
fn routed(app: &Signex, event: &keyboard::Event) -> KeyStroke {
    let window = app.ui_state.main_window_id.expect("main window");
    match app.route_key(window, event) {
        Some(Message::Ui(UiMsg::KeymapStroke { stroke, .. })) => stroke,
        other => panic!("expected a keymap stroke, got {other:?}"),
    }
}

fn stroke(text: &str) -> KeyStroke {
    text.parse().unwrap()
}

fn resolves_to(app: &Signex, stroke: &KeyStroke) -> Option<String> {
    app.ui_state
        .active_keymap
        .lookup(std::slice::from_ref(stroke), &[ShortcutContext::Global])
        .command
        .map(|c| c.as_str().to_string())
}

#[test]
fn a_user_binding_on_a_non_ascii_key_still_fires() {
    let app = app_with(Platform::Windows, &[("Ctrl+\u{f6}", "save_document")]);
    let sent = routed(&app, &press("\u{f6}", Code::Comma, Held::CTRL));
    assert_eq!(sent, stroke("Ctrl+\u{f6}"));
    assert_eq!(resolves_to(&app, &sent).as_deref(), Some("save_document"));
}

#[test]
fn a_bare_or_shift_only_key_never_takes_the_physical_binding() {
    let app = app_with(Platform::Windows, &[("2", "find")]);
    for held in [Held::empty(), Held::SHIFT] {
        let sent = routed(&app, &press("\u{e9}", Code::Digit2, held));
        assert_eq!(sent.key, stroke("\u{e9}").key, "stays the logical key");
        assert_eq!(resolves_to(&app, &sent), None);
    }
}

#[test]
fn ctrl_o_umlaut_fires_ctrl_comma_when_nothing_binds_the_umlaut() {
    let windows = app_with(Platform::Windows, &[("Mod+,", "open_preferences")]);
    let sent = routed(&windows, &press("\u{f6}", Code::Comma, Held::CTRL));
    assert_eq!(sent, stroke("Ctrl+,"));
    assert_eq!(
        resolves_to(&windows, &sent).as_deref(),
        Some("open_preferences")
    );

    let mac = app_with(Platform::MacOs, &[("Mod+,", "open_preferences")]);
    let sent = routed(&mac, &press("\u{f6}", Code::Comma, Held::LOGO));
    assert_eq!(sent, stroke("Cmd+,"));
    assert_eq!(
        resolves_to(&mac, &sent).as_deref(),
        Some("open_preferences")
    );
}

#[test]
fn the_logical_stroke_wins_while_it_continues_a_chord() {
    let mut app = app_with(
        Platform::Windows,
        &[
            ("Ctrl+K Ctrl+\u{f6}", "find"),
            ("Ctrl+K Ctrl+,", "open_preferences"),
        ],
    );
    app.ui_state.keymap_pending_sequence = vec![stroke("Ctrl+K")];
    app.ui_state.keymap_pending_target = Some(InputTarget::Main);
    assert_eq!(
        routed(&app, &press("\u{f6}", Code::Comma, Held::CTRL)),
        stroke("Ctrl+\u{f6}")
    );
}

#[test]
fn the_fallback_is_tried_after_a_chord_prefix_misses() {
    let mut app = app_with(Platform::Windows, &[("Ctrl+K Ctrl+,", "open_preferences")]);
    app.ui_state.keymap_pending_sequence = vec![stroke("Ctrl+K")];
    app.ui_state.keymap_pending_target = Some(InputTarget::Main);
    assert_eq!(
        routed(&app, &press("\u{f6}", Code::Comma, Held::CTRL)),
        stroke("Ctrl+,")
    );
}

/// #2095: a bare key of a non-Latin layout reaches its US binding on a miss.
#[test]
fn a_bare_cyrillic_or_greek_letter_fires_the_us_letter_binding() {
    let app = app_with(Platform::Windows, &[("W", "place_wire")]);
    // Russian ЙЦУКЕН: `ц` on the W key. Greek: final sigma on the W key.
    for logical in ["\u{446}", "\u{3c2}"] {
        let sent = routed(&app, &press(logical, Code::KeyW, Held::empty()));
        assert_eq!(sent, stroke("W"), "{logical}");
        assert_eq!(resolves_to(&app, &sent).as_deref(), Some("place_wire"));
    }
    let sent = routed(&app, &press("\u{426}", Code::KeyW, Held::SHIFT));
    assert_eq!(sent, stroke("Shift+W"));
}

#[test]
fn ctrl_z_and_ctrl_c_fire_on_a_russian_layout() {
    let app = app_with(Platform::Windows, &[("Mod+Z", "undo"), ("Mod+C", "copy")]);
    let undo = routed(&app, &press("\u{44f}", Code::KeyZ, Held::CTRL));
    assert_eq!(resolves_to(&app, &undo).as_deref(), Some("undo"));
    let copy = routed(&app, &press("\u{441}", Code::KeyC, Held::CTRL));
    assert_eq!(resolves_to(&app, &copy).as_deref(), Some("copy"));
}

#[test]
fn a_binding_on_the_cyrillic_letter_itself_keeps_priority() {
    let app = app_with(
        Platform::Windows,
        &[("\u{446}", "find"), ("W", "place_wire")],
    );
    let sent = routed(&app, &press("\u{446}", Code::KeyW, Held::empty()));
    assert_eq!(sent, stroke("\u{446}"));
    assert_eq!(resolves_to(&app, &sent).as_deref(), Some("find"));
}

#[test]
fn dvorak_keeps_its_logical_letter() {
    // Dvorak types `x` on the physical B key; nothing is remapped.
    let app = app_with(Platform::Windows, &[("B", "find")]);
    for held in [Held::empty(), Held::CTRL] {
        let sent = routed(&app, &press("x", Code::KeyB, held));
        assert_eq!(sent.key, stroke("X").key);
        assert_eq!(resolves_to(&app, &sent), None);
    }
}
