//! The symbol editor's pin-grab preference.

/// How a click selects/drags a pin in the symbol editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinSelectionMode {
    /// Default: only the pin body/tip is grabbable.
    #[default]
    PinOnly,
    /// The pin is also grabbable by its name or number label, and a
    /// selected pin's labels glow with it.
    TextAndPin,
}

impl PinSelectionMode {
    pub const ALL: [PinSelectionMode; 2] =
        [PinSelectionMode::PinOnly, PinSelectionMode::TextAndPin];
    /// True when name/number labels are grabbable + glow.
    pub fn allows_label_grab(self) -> bool {
        matches!(self, PinSelectionMode::TextAndPin)
    }

    /// Stable token used to persist this mode to `prefs.json`.
    pub fn pref_token(self) -> &'static str {
        match self {
            PinSelectionMode::PinOnly => "pin_only",
            PinSelectionMode::TextAndPin => "text_and_pin",
        }
    }

    /// Parse a persisted token back into a mode — unknown/legacy values
    /// fall back to the `PinOnly` default.
    pub fn from_pref_token(s: &str) -> Self {
        match s {
            "text_and_pin" => PinSelectionMode::TextAndPin,
            _ => PinSelectionMode::PinOnly,
        }
    }
}
