//! The US-QWERTY character of a physical key, for layouts whose logical key
//! is not ASCII (Turkish `ö` on the comma key, Cyrillic letters).

use iced::keyboard::key::{Code, Physical};

pub(super) fn us_qwerty_char(physical: Physical) -> Option<char> {
    let Physical::Code(code) = physical else {
        return None;
    };
    match code {
        Code::KeyA => Some('a'),
        Code::KeyB => Some('b'),
        Code::KeyC => Some('c'),
        Code::KeyD => Some('d'),
        Code::KeyE => Some('e'),
        Code::KeyF => Some('f'),
        Code::KeyG => Some('g'),
        Code::KeyH => Some('h'),
        Code::KeyI => Some('i'),
        Code::KeyJ => Some('j'),
        Code::KeyK => Some('k'),
        Code::KeyL => Some('l'),
        Code::KeyM => Some('m'),
        Code::KeyN => Some('n'),
        Code::KeyO => Some('o'),
        Code::KeyP => Some('p'),
        Code::KeyQ => Some('q'),
        Code::KeyR => Some('r'),
        Code::KeyS => Some('s'),
        Code::KeyT => Some('t'),
        Code::KeyU => Some('u'),
        Code::KeyV => Some('v'),
        Code::KeyW => Some('w'),
        Code::KeyX => Some('x'),
        Code::KeyY => Some('y'),
        Code::KeyZ => Some('z'),
        Code::Digit0 => Some('0'),
        Code::Digit1 => Some('1'),
        Code::Digit2 => Some('2'),
        Code::Digit3 => Some('3'),
        Code::Digit4 => Some('4'),
        Code::Digit5 => Some('5'),
        Code::Digit6 => Some('6'),
        Code::Digit7 => Some('7'),
        Code::Digit8 => Some('8'),
        Code::Digit9 => Some('9'),
        Code::Comma => Some(','),
        Code::Period => Some('.'),
        Code::Slash => Some('/'),
        Code::Semicolon => Some(';'),
        Code::Quote => Some('\''),
        Code::BracketLeft => Some('['),
        Code::BracketRight => Some(']'),
        Code::Backslash => Some('\\'),
        Code::Minus => Some('-'),
        Code::Equal => Some('='),
        Code::Backquote => Some('`'),
        _ => None,
    }
}
