//! Layout-independent shortcut characters (#2095): `shortcut_char` and the
//! bare-key retry of `KeyStroke::physical_fallback` for non-Latin layouts.

use super::{KeyStroke, shortcut_char};
use iced::keyboard::{Key, Modifiers as Held, key::Code, key::NativeCode, key::Physical};

fn letter(logical: &str, code: Code) -> Option<char> {
    shortcut_char(&Key::Character(logical.into()), Physical::Code(code))
}

fn retried(logical: &str, code: Code, held: Held) -> Option<String> {
    KeyStroke::physical_fallback(&Key::Character(logical.into()), Physical::Code(code), held)
        .map(|stroke| stroke.to_config_string())
}

#[test]
fn cyrillic_letters_read_at_their_physical_position() {
    assert_eq!(letter("\u{441}", Code::KeyC), Some('c'));
    assert_eq!(letter("\u{44f}", Code::KeyZ), Some('z'));
    assert_eq!(letter("\u{42f}", Code::KeyZ), Some('z'), "uppercase Я");
    assert_eq!(letter("\u{446}", Code::KeyW), Some('w'));
}

#[test]
fn greek_letters_read_at_their_physical_position() {
    assert_eq!(letter("\u{3c8}", Code::KeyC), Some('c'));
    assert_eq!(letter("\u{3b6}", Code::KeyZ), Some('z'));
    assert_eq!(letter("\u{3c2}", Code::KeyW), Some('w'));
}

#[test]
fn latin_letters_keep_their_logical_character() {
    // Dvorak types `x` on the physical B key; AZERTY `z` on the W key.
    assert_eq!(letter("x", Code::KeyB), Some('x'));
    assert_eq!(letter("z", Code::KeyW), Some('z'));
    assert_eq!(letter("Z", Code::KeyW), Some('z'), "lowercased");
    assert_eq!(letter("\u{e9}", Code::Digit2), Some('\u{e9}'));
}

#[test]
fn named_or_unmapped_keys_have_no_shortcut_char() {
    let space = Key::Named(iced::keyboard::key::Named::Space);
    assert_eq!(shortcut_char(&space, Physical::Code(Code::Space)), None);
    let unidentified = Physical::Unidentified(NativeCode::Unidentified);
    assert_eq!(
        shortcut_char(&Key::Character("\u{446}".into()), unidentified),
        None
    );
}

#[test]
fn a_bare_or_shift_only_non_latin_key_is_retried_as_its_us_letter() {
    assert_eq!(
        retried("\u{446}", Code::KeyW, Held::empty()).as_deref(),
        Some("W")
    );
    assert_eq!(
        retried("\u{426}", Code::KeyW, Held::SHIFT).as_deref(),
        Some("Shift+W")
    );
    assert_eq!(
        retried("\u{3c2}", Code::KeyW, Held::empty()).as_deref(),
        Some("W")
    );
}

#[test]
fn a_bare_latin_key_is_never_retried() {
    assert_eq!(retried("x", Code::KeyB, Held::empty()), None);
    assert_eq!(retried("\u{f6}", Code::Comma, Held::empty()), None);
    assert_eq!(retried("\u{c7}", Code::Period, Held::SHIFT), None);
}
