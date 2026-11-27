//! The Gerber settings of an Export Job's Gerber output (#1299, mockup row
//! 114): units, number format, zero suppression, the layers written and the
//! aperture form. Every setting left out reads as its default, and the
//! defaults are the files the fabrication writer wrote before the settings
//! existed, byte for byte. Rules: `docs/OUTPUT_JOBS.md`, "Gerber settings".

use serde::{Deserialize, Serialize};

/// The unit a Gerber file's coordinates and apertures are in (`%MO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GerberUnits {
    #[default]
    Mm,
    Inch,
}

impl GerberUnits {
    pub const ALL: [GerberUnits; 2] = [GerberUnits::Mm, GerberUnits::Inch];

    /// The value a `.snxout` file and a command operand name it by.
    pub fn id(self) -> &'static str {
        match self {
            GerberUnits::Mm => "mm",
            GerberUnits::Inch => "inch",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|units| units.id() == id.trim())
    }
}

/// Which zeros a coordinate leaves out (`%FS`). Leading is the writer's own
/// form: a coordinate is a plain integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GerberZeros {
    /// Leading zeros left out (`%FSLA`), the plain integer.
    #[default]
    Leading,
    /// Trailing zeros left out (`%FST`): every integer digit written.
    Trailing,
    /// No zero left out: every digit of the format written (`%FSLA`).
    None,
}

impl GerberZeros {
    pub const ALL: [GerberZeros; 3] = [
        GerberZeros::Leading,
        GerberZeros::Trailing,
        GerberZeros::None,
    ];

    pub fn id(self) -> &'static str {
        match self {
            GerberZeros::Leading => "leading",
            GerberZeros::Trailing => "trailing",
            GerberZeros::None => "none",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|zeros| zeros.id() == id.trim())
    }
}

/// How the apertures are defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GerberApertures {
    /// The writer's own: the standard circle, rectangle and obround where a
    /// pad has one, a macro for every other shape.
    #[default]
    Standard,
    /// Every aperture a macro, for a reader that wants macros only.
    Macros,
}

impl GerberApertures {
    pub const ALL: [GerberApertures; 2] = [GerberApertures::Standard, GerberApertures::Macros];

    pub fn id(self) -> &'static str {
        match self {
            GerberApertures::Standard => "standard",
            GerberApertures::Macros => "macros",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|apertures| apertures.id() == id.trim())
    }
}

/// The digits of a coordinate before and after the point, `4:6` by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GerberFormat {
    pub integer: u8,
    pub decimal: u8,
}

impl Default for GerberFormat {
    fn default() -> Self {
        Self {
            integer: 4,
            decimal: 6,
        }
    }
}

/// The integer digits a format may have.
pub const GERBER_INTEGER_DIGITS: std::ops::RangeInclusive<u8> = 1..=6;
/// The decimal digits a format may have.
pub const GERBER_DECIMAL_DIGITS: std::ops::RangeInclusive<u8> = 4..=6;

impl GerberFormat {
    /// `4:6`: integer digits, a colon, decimal digits, each in its range.
    pub fn parse(text: &str) -> Option<Self> {
        let (integer, decimal) = text.trim().split_once(':')?;
        let integer: u8 = integer.trim().parse().ok()?;
        let decimal: u8 = decimal.trim().parse().ok()?;
        (GERBER_INTEGER_DIGITS.contains(&integer) && GERBER_DECIMAL_DIGITS.contains(&decimal))
            .then_some(Self { integer, decimal })
    }
}

impl std::fmt::Display for GerberFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.integer, self.decimal)
    }
}

impl TryFrom<String> for GerberFormat {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text).ok_or_else(|| {
            format!("the Gerber format {text:?} is not integer:decimal digits, 1-6 and 4-6")
        })
    }
}

impl From<GerberFormat> for String {
    fn from(format: GerberFormat) -> Self {
        format.to_string()
    }
}

/// The layer word every copper layer answers to in a layer list.
pub const GERBER_ALL_COPPER: &str = "copper";

/// Whether `id` can name a layer in a layer list: lowercase letters,
/// digits and `-`. Which ids a board has: `docs/OUTPUT_JOBS.md`.
pub fn is_gerber_layer_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A layer list as typed: `all` (or nothing) is every layer, `None`;
/// otherwise layer ids, comma separated. `Err` names the first word that is
/// no layer id.
pub fn parse_gerber_layers(text: &str) -> Result<Option<Vec<String>>, String> {
    let text = text.trim();
    if text.is_empty() || text == "all" {
        return Ok(None);
    }
    let mut ids = Vec::new();
    for id in text.split(',').map(str::trim).filter(|id| !id.is_empty()) {
        if !is_gerber_layer_id(id) {
            return Err(id.to_string());
        }
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_string());
        }
    }
    Ok(Some(ids))
}

/// A layer list as the tab's field and a command's answer show it: `all`,
/// or the ids comma separated.
pub fn gerber_layers_text(layers: Option<&[String]>) -> String {
    match layers {
        None => "all".to_string(),
        Some(ids) => ids.join(", "),
    }
}
/// Everything a Gerber output is written with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GerberSettings {
    pub units: GerberUnits,
    pub format: GerberFormat,
    pub zeros: GerberZeros,
    /// The layers written, by id; every layer when `None`.
    pub layers: Option<Vec<String>>,
    pub apertures: GerberApertures,
}

impl GerberSettings {
    /// Whether the image with layer id `id` is written; `is_copper` lets
    /// [`GERBER_ALL_COPPER`] take every copper layer.
    pub fn writes_layer(&self, id: &str, is_copper: bool) -> bool {
        match &self.layers {
            None => true,
            Some(list) => list
                .iter()
                .any(|named| named == id || (is_copper && named == GERBER_ALL_COPPER)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_settings_are_the_writers_own() {
        let settings = GerberSettings::default();
        assert_eq!(settings.units, GerberUnits::Mm);
        assert_eq!(settings.format.to_string(), "4:6");
        assert_eq!(settings.zeros, GerberZeros::Leading);
        assert_eq!(settings.layers, None);
        assert_eq!(settings.apertures, GerberApertures::Standard);
    }

    #[test]
    fn a_format_is_two_digit_counts_in_their_ranges() {
        assert_eq!(
            GerberFormat::parse(" 2:5 "),
            Some(GerberFormat {
                integer: 2,
                decimal: 5
            })
        );
        for bad in [
            "", "4", "4:", ":6", "0:6", "7:6", "4:3", "4:7", "a:b", "4:6:1",
        ] {
            assert_eq!(GerberFormat::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn every_setting_names_itself_back() {
        for units in GerberUnits::ALL {
            assert_eq!(GerberUnits::from_id(units.id()), Some(units));
        }
        for zeros in GerberZeros::ALL {
            assert_eq!(GerberZeros::from_id(zeros.id()), Some(zeros));
        }
        for apertures in GerberApertures::ALL {
            assert_eq!(GerberApertures::from_id(apertures.id()), Some(apertures));
        }
        assert_eq!(GerberUnits::from_id("mil"), None);
    }
    #[test]
    fn a_typed_layer_list_is_all_or_layer_ids() {
        assert_eq!(parse_gerber_layers("all"), Ok(None));
        assert_eq!(parse_gerber_layers(" "), Ok(None));
        assert_eq!(
            parse_gerber_layers("copper-top, profile,copper-top"),
            Ok(Some(vec!["copper-top".to_string(), "profile".to_string()]))
        );
        assert_eq!(
            parse_gerber_layers("copper-top, Top"),
            Err("Top".to_string())
        );
        let ids = ["copper".to_string(), "profile".to_string()];
        assert_eq!(gerber_layers_text(Some(&ids[..])), "copper, profile");
        assert_eq!(gerber_layers_text(None), "all");
    }

    #[test]
    fn a_layer_list_names_layers_and_copper_takes_every_copper_layer() {
        let settings = GerberSettings {
            layers: Some(vec!["copper".into(), "profile".into()]),
            ..GerberSettings::default()
        };
        assert!(settings.writes_layer("copper-inner2", true));
        assert!(settings.writes_layer("profile", false));
        assert!(!settings.writes_layer("legend-top", false));
        assert!(GerberSettings::default().writes_layer("legend-top", false));
        assert!(is_gerber_layer_id("soldermask-top"));
        assert!(!is_gerber_layer_id("Top Copper"));
        assert!(!is_gerber_layer_id(""));
    }
}
