//! Per-sheet frame settings (#1134): whether the sheet draws its border
//! (with zone markers) and its title block, and how many zones the border
//! is divided into. The canvas and the PDF both read them, so a sheet looks
//! the same on screen and on paper.

use serde::{Deserialize, Deserializer, Serialize};

/// Most zones along one edge; letters past Z run AA, AB, ….
pub const MAX_ZONES: u8 = 52;

/// How a sheet's frame is drawn. The default is what every sheet drew before
/// these settings existed: border and title block on, 8 × 6 zones. Each
/// setting is written only when it differs from its default, so a sheet at
/// the default stores nothing and older files re-save byte-identical.
///
/// Reading is lenient: a cosmetic setting never stops a sheet from opening.
/// A zone count out of range is clamped to `0..=MAX_ZONES`, and a value of
/// the wrong type reads as that setting's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetFrameSettings {
    /// Border and zone markers.
    #[serde(
        default = "yes",
        deserialize_with = "lenient_flag",
        skip_serializing_if = "is_true"
    )]
    pub show_border: bool,
    #[serde(
        default = "yes",
        deserialize_with = "lenient_flag",
        skip_serializing_if = "is_true"
    )]
    pub show_title_block: bool,
    /// Zones numbered 1, 2, 3… along the top and bottom; 0 draws none.
    #[serde(
        default = "default_horizontal_zones",
        deserialize_with = "lenient_horizontal_zones",
        skip_serializing_if = "is_default_horizontal_zones"
    )]
    pub horizontal_zones: u8,
    /// Zones lettered A, B, C… down the sides; 0 draws none.
    #[serde(
        default = "default_vertical_zones",
        deserialize_with = "lenient_vertical_zones",
        skip_serializing_if = "is_default_vertical_zones"
    )]
    pub vertical_zones: u8,
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn default_horizontal_zones() -> u8 {
    8
}

fn default_vertical_zones() -> u8 {
    6
}

fn is_default_horizontal_zones(n: &u8) -> bool {
    *n == default_horizontal_zones()
}

fn is_default_vertical_zones(n: &u8) -> bool {
    *n == default_vertical_zones()
}

/// A value as written, before it is checked against the setting's type.
#[derive(Deserialize)]
#[serde(untagged)]
enum Raw<T> {
    Value(T),
    Other(serde::de::IgnoredAny),
}

fn lenient_flag<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(match Raw::<bool>::deserialize(d)? {
        Raw::Value(flag) => flag,
        Raw::Other(_) => yes(),
    })
}

fn lenient_zones<'de, D: Deserializer<'de>>(d: D, default: u8) -> Result<u8, D::Error> {
    Ok(match Raw::<i64>::deserialize(d)? {
        Raw::Value(n) => n.clamp(0, i64::from(MAX_ZONES)) as u8,
        Raw::Other(_) => default,
    })
}

fn lenient_horizontal_zones<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    lenient_zones(d, default_horizontal_zones())
}

fn lenient_vertical_zones<'de, D: Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    lenient_zones(d, default_vertical_zones())
}

impl Default for SheetFrameSettings {
    fn default() -> Self {
        Self {
            show_border: true,
            show_title_block: true,
            horizontal_zones: default_horizontal_zones(),
            vertical_zones: default_vertical_zones(),
        }
    }
}

impl SheetFrameSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// These settings with both zone counts clamped to `0..=MAX_ZONES`.
    pub fn clamped(self) -> Self {
        Self {
            horizontal_zones: self.horizontal_zones.min(MAX_ZONES),
            vertical_zones: self.vertical_zones.min(MAX_ZONES),
            ..self
        }
    }
}
