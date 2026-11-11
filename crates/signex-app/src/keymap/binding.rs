use crate::keymap::{AppCommandId, Platform, physical::us_qwerty_char};
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
    /// The `Mod` token of a profile: the platform's standard modifier,
    /// Cmd on macOS and Ctrl elsewhere (#1645). Only parsed or recorded
    /// strokes carry it; a live keystroke never does. Spelling is kept so a
    /// saved profile reads `Mod+…` on every OS. Compare strokes through
    /// [`KeyStroke::resolved_for`], never raw.
    #[serde(default)]
    pub primary: bool,
}

/// The macOS modifier symbols in Apple's order: ⌃ ⌥ ⇧ ⌘.
const MAC_SYMBOLS: [char; 4] = ['\u{2303}', '\u{2325}', '\u{21e7}', '\u{2318}'];

/// Whether `ch` is one of the macOS modifier symbols.
fn is_mac_modifier_symbol(ch: char) -> bool {
    MAC_SYMBOLS.contains(&ch)
}

/// One macOS-style stroke label (`⇧⌘S`) as its key caps (`⇧`, `⌘`, `S`);
/// `None` when it does not start with a modifier symbol.
pub fn mac_stroke_caps(label: &str) -> Option<Vec<String>> {
    if !label.starts_with(is_mac_modifier_symbol) {
        return None;
    }
    let rest = label.trim_start_matches(is_mac_modifier_symbol);
    let mut caps: Vec<String> = label[..label.len() - rest.len()]
        .chars()
        .map(String::from)
        .collect();
    if !rest.is_empty() {
        caps.push(rest.to_owned());
    }
    Some(caps)
}

impl Modifiers {
    /// `primary` replaced by the concrete key it means on `platform`: Cmd
    /// on macOS, Ctrl elsewhere, never Super. Idempotent.
    pub fn resolved_for(self, platform: Platform) -> Self {
        if !self.primary {
            return self;
        }
        let mut resolved = Self {
            primary: false,
            ..self
        };
        match platform {
            Platform::MacOs => resolved.command = true,
            Platform::Windows | Platform::Linux => resolved.ctrl = true,
        }
        resolved
    }

    /// Canonical form shared by live keystrokes and parsed bindings (#1378).
    ///
    /// `ctrl` is the physical Control key and `command` is the logo key
    /// (Cmd on macOS, Win/Super elsewhere), on every OS. iced's own
    /// `Modifiers::command()` is Control off macOS, which would set both
    /// flags for a plain Ctrl press while a parsed `Ctrl+W` sets only
    /// `ctrl`, so the two never compared equal. On macOS Cmd and Ctrl
    /// stay distinct.
    pub fn from_iced(modifiers: keyboard::Modifiers) -> Self {
        Self {
            ctrl: modifiers.control(),
            alt: modifiers.alt(),
            shift: modifiers.shift(),
            command: modifiers.logo(),
            primary: false,
        }
    }

    /// The held modifiers as `platform` writes them, without a key: Apple
    /// symbols in Apple order on macOS (`⇧⌘`), `Ctrl+Shift` elsewhere.
    pub fn display_for(self, platform: Platform) -> String {
        let m = self.resolved_for(platform);
        if platform == Platform::MacOs {
            return [
                (m.ctrl, '\u{2303}'),
                (m.alt, '\u{2325}'),
                (m.shift, '\u{21e7}'),
                (m.command, '\u{2318}'),
            ]
            .into_iter()
            .filter_map(|(on, symbol)| on.then_some(symbol))
            .collect();
        }
        [
            (m.ctrl, "Ctrl"),
            (m.command, platform.logo_key_name()),
            (m.alt, "Alt"),
            (m.shift, "Shift"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect::<Vec<_>>()
        .join("+")
    }

    /// The form the Preferences recorder stores: on macOS a Cmd press is
    /// `Mod`, so a profile made on a Mac also works elsewhere. Other OSes
    /// record exactly what was pressed.
    pub fn recorded_for(self, platform: Platform) -> Self {
        if platform == Platform::MacOs && self.command {
            return Self {
                command: false,
                primary: true,
                ..self
            };
        }
        self
    }

    /// The logo key's spelling in profile files, on every OS.
    pub const CONFIG_COMMAND_NAME: &'static str = "Cmd";

    /// What this OS calls the logo key: Cmd on macOS, Win on Windows,
    /// Super elsewhere. The parser accepts all three spellings on every OS.
    pub fn command_key_name() -> &'static str {
        Platform::current().logo_key_name()
    }

    pub fn is_empty(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.command && !self.primary
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KeyStroke {
    pub modifiers: Modifiers,
    pub key: KeyToken,
}

impl KeyStroke {
    /// The stroke for the key the layout produced (the logical key).
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

impl KeyStroke {
    /// The stroke to try when the logical one matches nothing: the key's
    /// US-QWERTY character, for a layout whose logical key is not ASCII
    /// (Turkish `ö` on the comma key, Cyrillic). Only offered while Ctrl,
    /// Cmd or Alt is held; a bare or Shift-only key is never remapped.
    /// `None` when there is nothing to retry with (#1645).
    pub fn physical_fallback(
        key: &keyboard::Key,
        physical: keyboard::key::Physical,
        modifiers: keyboard::Modifiers,
    ) -> Option<Self> {
        if !(modifiers.control() || modifiers.logo() || modifiers.alt()) {
            return None;
        }
        let keyboard::Key::Character(value) = key.as_ref() else {
            return None;
        };
        if value.to_lowercase().is_ascii() {
            return None;
        }
        let us = us_qwerty_char(physical)?;
        Some(Self {
            modifiers: Modifiers::from_iced(modifiers),
            key: KeyToken::Character(us.to_string()),
        })
    }
}

impl KeyStroke {
    /// The spelling written to a profile file. Always `Cmd` for the logo
    /// key, whatever the OS, so a profile saved on one machine is
    /// byte-identical to the same profile saved on another (#1378).
    /// [`fmt::Display`] is the per-OS label for the UI instead.
    pub fn to_config_string(&self) -> String {
        self.labelled(Modifiers::CONFIG_COMMAND_NAME)
    }

    /// This stroke with `Mod` replaced by the concrete modifier of `platform`.
    pub fn resolved_for(&self, platform: Platform) -> Self {
        Self {
            modifiers: self.modifiers.resolved_for(platform),
            key: self.key.clone(),
        }
    }

    /// The stroke as the Preferences recorder stores it on `platform`.
    pub fn recorded_for(&self, platform: Platform) -> Self {
        Self {
            modifiers: self.modifiers.recorded_for(platform),
            key: self.key.clone(),
        }
    }

    /// The label shown in the UI on `platform`: Apple symbols in Apple
    /// order without `+` on macOS (`⇧⌘S`), text elsewhere (`Ctrl+Shift+S`).
    pub fn display_for(&self, platform: Platform) -> String {
        let stroke = self.resolved_for(platform);
        if platform != Platform::MacOs {
            return stroke.labelled(platform.logo_key_name());
        }
        let mut text = stroke.modifiers.display_for(platform);
        text.push_str(&stroke.key.to_string());
        text
    }

    fn labelled(&self, command_name: &str) -> String {
        let mut text = String::new();
        if self.modifiers.primary {
            text.push_str("Mod+");
        }
        if self.modifiers.ctrl {
            text.push_str("Ctrl+");
        }
        if self.modifiers.command {
            text.push_str(command_name);
            text.push('+');
        }
        if self.modifiers.alt {
            text.push_str("Alt+");
        }
        if self.modifiers.shift {
            text.push_str("Shift+");
        }
        text.push_str(&self.key.to_string());
        text
    }
}

impl fmt::Display for KeyStroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_for(Platform::current()))
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

    /// The text written to a profile file: [`KeyStroke::to_config_string`]
    /// per stroke, so it is the same on every OS.
    pub fn config_text(&self) -> String {
        match self {
            Self::KeySequence(strokes) => strokes
                .iter()
                .map(KeyStroke::to_config_string)
                .collect::<Vec<_>>()
                .join(" "),
            Self::PointerGesture(gesture) => gesture.clone(),
        }
    }

    /// The per-OS label shown in the UI (Cmd / Win / Super for the logo key).
    pub fn display_text(&self) -> String {
        self.display_text_for(Platform::current())
    }

    /// [`Self::display_text`] for an explicit platform.
    pub fn display_text_for(&self, platform: Platform) -> String {
        match self {
            Self::KeySequence(strokes) => strokes
                .iter()
                .map(|stroke| stroke.display_for(platform))
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
        // The plus key itself: `+` alone, or `Shift++` after the modifiers.
        let (modifier_text, plus_key) = match source.strip_suffix("++") {
            Some(modifiers) => (modifiers, true),
            None if source == "+" => ("", true),
            None => (source, false),
        };
        if plus_key {
            key = Some(KeyToken::Character("+".to_owned()));
        }
        let parts = modifier_text
            .split('+')
            .filter(|_| !modifier_text.is_empty());
        for part in parts {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "cmd" | "command" | "super" | "win" => modifiers.command = true,
                "mod" => modifiers.primary = true,
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
    /// The only OS this binding applies on; `None` is every OS. A binding
    /// for another OS stays in the profile (and is saved back with its tag)
    /// but is not compiled, listed or edited here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
}

impl ShortcutBinding {
    pub fn applies_on(&self, platform: Platform) -> bool {
        self.platform.is_none_or(|only| only == platform)
    }
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
