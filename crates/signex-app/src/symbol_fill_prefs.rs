//! The "Fill symbol bodies" preference: whether the body shapes a symbol
//! leaves unfilled are drawn filled, and in which colour. A symbol's own fill
//! always wins (`signex_types::schematic::default_body_fill_shapes`).

use signex_types::theme::{CanvasColors, Color};

/// The `prefs.json` key of the on/off switch.
pub const ENABLED_KEY: &str = "symbol_body_fill";
/// The `prefs.json` key of the user's colour (`#rrggbbaa`); absent or
/// unreadable follows the theme's body tint.
pub const COLOR_KEY: &str = "symbol_body_fill_color";

/// Persisted under [`ENABLED_KEY`] and [`COLOR_KEY`]. On by default, in the
/// theme's tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolBodyFill {
    pub enabled: bool,
    /// The user's colour; `None` follows the theme's `body_tint`.
    pub color: Option<Color>,
}

impl Default for SymbolBodyFill {
    fn default() -> Self {
        Self {
            enabled: true,
            color: None,
        }
    }
}

impl SymbolBodyFill {
    /// `colors` with the body tint this preference draws: the user's colour
    /// over the theme's, or none when the preference is off.
    #[must_use]
    pub fn apply(self, mut colors: CanvasColors) -> CanvasColors {
        colors.body_tint = if self.enabled {
            self.color.or(colors.body_tint)
        } else {
            None
        };
        colors
    }

    /// Read from `prefs.json`; a missing or malformed value reads as its
    /// default.
    #[must_use]
    pub fn from_prefs(json: Option<&serde_json::Value>) -> Self {
        let default = Self::default();
        let Some(json) = json else {
            return default;
        };
        Self {
            enabled: json[ENABLED_KEY].as_bool().unwrap_or(default.enabled),
            color: json[COLOR_KEY].as_str().and_then(Color::parse_hex),
        }
    }

    /// Write both keys into a `prefs.json` object; a theme colour removes
    /// the colour key.
    pub fn write_into(self, prefs: &mut serde_json::Map<String, serde_json::Value>) {
        prefs.insert(ENABLED_KEY.to_string(), serde_json::json!(self.enabled));
        match self.color {
            Some(color) => {
                prefs.insert(COLOR_KEY.to_string(), serde_json::json!(color.to_hex()));
            }
            None => {
                prefs.remove(COLOR_KEY);
            }
        }
    }
}

#[cfg(test)]
#[path = "symbol_fill_prefs_tests.rs"]
mod tests;
