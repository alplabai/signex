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
