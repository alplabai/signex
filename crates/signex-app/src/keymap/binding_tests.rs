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
fn preserves_pointer_gestures_as_non_keyboard_triggers() {
    assert!(matches!(
        ShortcutTrigger::parse("Shift+Click").unwrap(),
        ShortcutTrigger::PointerGesture(_)
    ));
}
