use super::{KeyStroke, KeyToken, ShortcutTrigger};
use std::str::FromStr;

#[test]
fn parses_multi_stroke_shortcut() {
    let trigger = ShortcutTrigger::parse("P V N").unwrap();
    let ShortcutTrigger::KeySequence(strokes) = trigger else {
        panic!("expected key sequence");
    };
    assert_eq!(strokes.len(), 3);
    assert_eq!(strokes[0].to_string(), "P");
}

#[test]
fn parses_modified_named_key() {
    let stroke = KeyStroke::from_str("Ctrl+Shift+F1").unwrap();
    assert!(stroke.modifiers.ctrl);
    assert!(stroke.modifiers.shift);
    assert_eq!(stroke.key, KeyToken::Function(1));
}

#[test]
fn displays_localization_free_named_keys() {
    let cases = [
        ("Escape", "ESC"),
        ("Ctrl+Escape", "Ctrl+ESC"),
        ("Delete", "DEL"),
        ("Insert", "INS"),
        ("PageUp", "PGUP"),
        ("PageDown", "PGDN"),
        ("ArrowUp", "UP"),
        ("ArrowDown", "DOWN"),
        ("ArrowLeft", "LEFT"),
        ("ArrowRight", "RIGHT"),
    ];

    for (input, expected) in cases {
        assert_eq!(KeyStroke::from_str(input).unwrap().to_string(), expected);
    }
}

#[test]
fn accepts_ascii_named_key_aliases() {
    let cases = [
        ("ESC", KeyToken::Escape),
        ("DEL", KeyToken::Delete),
        ("INS", KeyToken::Insert),
        ("PGUP", KeyToken::PageUp),
        ("PGDN", KeyToken::PageDown),
        ("UP", KeyToken::ArrowUp),
        ("DOWN", KeyToken::ArrowDown),
        ("LEFT", KeyToken::ArrowLeft),
        ("RIGHT", KeyToken::ArrowRight),
    ];

    for (input, expected) in cases {
        assert_eq!(KeyStroke::from_str(input).unwrap().key, expected);
    }
}

#[test]
fn preserves_pointer_gestures_as_non_keyboard_triggers() {
    assert!(matches!(
        ShortcutTrigger::parse("Shift+Click").unwrap(),
        ShortcutTrigger::PointerGesture(_)
    ));
}

// #1378 — a live keystroke and a parsed binding must compare equal on every
// OS. iced's `command()` is Control off macOS, which used to set both `ctrl`
// and `command` for a plain Ctrl press while the parser set only `ctrl`.
#[test]
fn live_ctrl_stroke_equals_parsed_ctrl_binding() {
    let live = KeyStroke::from_iced(
        &iced::keyboard::Key::Character("w".into()),
        iced::keyboard::Modifiers::CTRL,
    )
    .unwrap();
    assert_eq!(live, KeyStroke::from_str("Ctrl+W").unwrap());
    assert!(!live.modifiers.command);
}

#[test]
fn live_logo_stroke_equals_parsed_cmd_binding_and_stays_distinct_from_ctrl() {
    let key = iced::keyboard::Key::Character("w".into());
    let logo = KeyStroke::from_iced(&key, iced::keyboard::Modifiers::LOGO).unwrap();
    let ctrl = KeyStroke::from_iced(&key, iced::keyboard::Modifiers::CTRL).unwrap();
    assert_eq!(logo, KeyStroke::from_str("Cmd+W").unwrap());
    assert_ne!(logo, ctrl);
}

// #1378 — the logo key is named per OS, and shown even when Ctrl is held
// too (the old `command && !ctrl` rule dropped it).
#[test]
fn the_logo_key_is_named_per_os_and_shown_alongside_ctrl() {
    let name = crate::keymap::Modifiers::command_key_name();
    assert_eq!(
        KeyStroke::from_str("Cmd+W").unwrap().to_string(),
        format!("{name}+W")
    );
    assert_eq!(
        KeyStroke::from_str("Ctrl+Cmd+W").unwrap().to_string(),
        format!("Ctrl+{name}+W")
    );
    // Whatever this OS prints must parse back to the same stroke.
    let stroke = KeyStroke::from_str("Ctrl+Cmd+W").unwrap();
    assert_eq!(KeyStroke::from_str(&stroke.to_string()).unwrap(), stroke);
}

// #1378 — what is written to a profile file must not depend on the OS the
// profile was saved on; only the on-screen label does.
#[test]
fn config_text_is_identical_on_every_os_while_the_label_is_per_os() {
    let trigger = ShortcutTrigger::parse("Ctrl+Cmd+W S").unwrap();
    assert_eq!(trigger.config_text(), "Ctrl+Cmd+W S");

    let name = crate::keymap::Modifiers::command_key_name();
    assert_eq!(trigger.display_text(), format!("Ctrl+{name}+W S"));

    // Every spelling of the logo key serialises as `Cmd`.
    for spelling in ["Win+S", "Super+S", "Command+S"] {
        assert_eq!(
            ShortcutTrigger::parse(spelling).unwrap().config_text(),
            "Cmd+S"
        );
    }
}
