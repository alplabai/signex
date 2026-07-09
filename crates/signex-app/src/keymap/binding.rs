use crate::keymap::AppCommandId;
use iced::keyboard;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str::FromStr};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutContext {
    #[default]
    Global,
    Schematic,
    Footprint,
    Pcb,
    Library,
    Modal,
    TextInput,
    CommandPalette,
    Placement,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyToken {
    Character(String),
    Escape,
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Function(u8),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default,
)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub command: bool,
}

impl Modifiers {
    pub fn from_iced(modifiers: keyboard::Modifiers) -> Self {
        Self {
            ctrl: modifiers.control(),
            alt: modifiers.alt(),
            shift: modifiers.shift(),
            command: modifiers.command(),
        }
    }

    pub fn is_empty(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.command
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KeyStroke {
    pub modifiers: Modifiers,
    pub key: KeyToken,
}

impl KeyStroke {
    pub fn from_iced(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Self> {
        let key = match key.as_ref() {
            keyboard::Key::Character(value) => {
                let normalized = value.to_lowercase();
                if normalized.chars().count() != 1 {
                    return None;
                }
                KeyToken::Character(normalized)
            }
            keyboard::Key::Named(named) => match named {
                keyboard::key::Named::Escape => KeyToken::Escape,
                keyboard::key::Named::Enter => KeyToken::Enter,
                keyboard::key::Named::Tab => KeyToken::Tab,
                keyboard::key::Named::Space => KeyToken::Space,
                keyboard::key::Named::Backspace => KeyToken::Backspace,
                keyboard::key::Named::Delete => KeyToken::Delete,
                keyboard::key::Named::Insert => KeyToken::Insert,
                keyboard::key::Named::Home => KeyToken::Home,
                keyboard::key::Named::End => KeyToken::End,
                keyboard::key::Named::PageUp => KeyToken::PageUp,
                keyboard::key::Named::PageDown => KeyToken::PageDown,
                keyboard::key::Named::ArrowUp => KeyToken::ArrowUp,
                keyboard::key::Named::ArrowDown => KeyToken::ArrowDown,
                keyboard::key::Named::ArrowLeft => KeyToken::ArrowLeft,
                keyboard::key::Named::ArrowRight => KeyToken::ArrowRight,
                keyboard::key::Named::F1 => KeyToken::Function(1),
                keyboard::key::Named::F2 => KeyToken::Function(2),
                keyboard::key::Named::F3 => KeyToken::Function(3),
                keyboard::key::Named::F4 => KeyToken::Function(4),
                keyboard::key::Named::F5 => KeyToken::Function(5),
                keyboard::key::Named::F6 => KeyToken::Function(6),
                keyboard::key::Named::F7 => KeyToken::Function(7),
                keyboard::key::Named::F8 => KeyToken::Function(8),
                keyboard::key::Named::F9 => KeyToken::Function(9),
                keyboard::key::Named::F10 => KeyToken::Function(10),
                keyboard::key::Named::F11 => KeyToken::Function(11),
                keyboard::key::Named::F12 => KeyToken::Function(12),
                _ => return None,
            },
            _ => return None,
        };

        Some(Self {
            modifiers: Modifiers::from_iced(modifiers),
            key,
        })
    }
}

impl fmt::Display for KeyStroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.command && !self.modifiers.ctrl {
            f.write_str("Cmd+")?;
        }
        if self.modifiers.alt {
            f.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            f.write_str("Shift+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl fmt::Display for KeyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Character(value) => f.write_str(&value.to_uppercase()),
            Self::Escape => f.write_str("ESC"),
            Self::Enter => f.write_str("ENTER"),
            Self::Tab => f.write_str("TAB"),
            Self::Space => f.write_str("SPACE"),
            Self::Backspace => f.write_str("BACKSPACE"),
            Self::Delete => f.write_str("DEL"),
            Self::Insert => f.write_str("INS"),
            Self::Home => f.write_str("HOME"),
            Self::End => f.write_str("END"),
            Self::PageUp => f.write_str("PGUP"),
            Self::PageDown => f.write_str("PGDN"),
            Self::ArrowUp => f.write_str("UP"),
            Self::ArrowDown => f.write_str("DOWN"),
            Self::ArrowLeft => f.write_str("LEFT"),
            Self::ArrowRight => f.write_str("RIGHT"),
            Self::Function(number) => write!(f, "F{number}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortcutTrigger {
    KeySequence(Vec<KeyStroke>),
    PointerGesture(String),
}

impl ShortcutTrigger {
    pub fn parse(source: &str) -> Result<Self, KeyParseError> {
        let source = source.trim();
        if source.is_empty() {
            return Err(KeyParseError::Empty);
        }
        if source.eq_ignore_ascii_case("doubleclick")
            || source.to_ascii_lowercase().ends_with("+click")
            || source.eq_ignore_ascii_case("click")
        {
            return Ok(Self::PointerGesture(source.to_string()));
        }
        let strokes = source
            .split_whitespace()
            .map(KeyStroke::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::KeySequence(strokes))
    }

    pub fn display_text(&self) -> String {
        match self {
            Self::KeySequence(strokes) => strokes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
            Self::PointerGesture(gesture) => gesture.clone(),
        }
    }
}

impl FromStr for KeyStroke {
    type Err = KeyParseError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let source = source.trim();
        if source.is_empty() {
            return Err(KeyParseError::Empty);
        }

        let mut modifiers = Modifiers::default();
        let mut key = None;
        for part in source.split('+') {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "cmd" | "command" | "super" | "win" => modifiers.command = true,
                "alt" | "option" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                _ if key.is_none() => key = Some(parse_key_token(part)?),
                _ => return Err(KeyParseError::InvalidStroke(source.to_string())),
            }
        }

        Ok(Self {
            modifiers,
            key: key.ok_or_else(|| KeyParseError::MissingKey(source.to_string()))?,
        })
    }
}

fn parse_key_token(part: &str) -> Result<KeyToken, KeyParseError> {
    let lower = part.to_ascii_lowercase();
    let token = match lower.as_str() {
        "esc" | "escape" => KeyToken::Escape,
        "enter" | "return" => KeyToken::Enter,
        "tab" => KeyToken::Tab,
        "space" => KeyToken::Space,
        "backspace" => KeyToken::Backspace,
        "del" | "delete" => KeyToken::Delete,
        "ins" | "insert" => KeyToken::Insert,
        "home" => KeyToken::Home,
        "end" => KeyToken::End,
        "pageup" | "pgup" => KeyToken::PageUp,
        "pagedown" | "pgdn" => KeyToken::PageDown,
        "up" | "arrowup" => KeyToken::ArrowUp,
        "down" | "arrowdown" => KeyToken::ArrowDown,
        "left" | "arrowleft" => KeyToken::ArrowLeft,
        "right" | "arrowright" => KeyToken::ArrowRight,
        _ if lower.starts_with('f') && lower.len() > 1 => {
            let number = lower[1..]
                .parse::<u8>()
                .map_err(|_| KeyParseError::UnknownKey(part.to_string()))?;
            if !(1..=24).contains(&number) {
                return Err(KeyParseError::UnknownKey(part.to_string()));
            }
            KeyToken::Function(number)
        }
        _ if part.chars().count() == 1 => KeyToken::Character(lower),
        _ => return Err(KeyParseError::UnknownKey(part.to_string())),
    };
    Ok(token)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyParseError {
    Empty,
    MissingKey(String),
    InvalidStroke(String),
    UnknownKey(String),
}

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("shortcut trigger must not be empty"),
            Self::MissingKey(value) => write!(f, "shortcut `{value}` is missing a key"),
            Self::InvalidStroke(value) => write!(f, "shortcut `{value}` has more than one key"),
            Self::UnknownKey(value) => write!(f, "unknown key `{value}`"),
        }
    }
}

impl Error for KeyParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutBinding {
    pub action: ShortcutBindingAction,
    #[serde(default)]
    pub context: ShortcutContext,
    pub triggers: Vec<ShortcutTrigger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShortcutBindingAction {
    Command(AppCommandId),
    Unbind(AppCommandId),
    NoAction,
}

impl ShortcutBindingAction {
    pub fn command(&self) -> Option<&AppCommandId> {
        match self {
            Self::Command(command) | Self::Unbind(command) => Some(command),
            Self::NoAction => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyBindingSource {
    BuiltIn(String),
    Custom(String),
}
