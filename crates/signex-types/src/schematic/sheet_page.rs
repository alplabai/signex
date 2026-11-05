//! Per-sheet page settings (#2034): the paper colour and the corner the
//! displayed coordinates count from. Saved in `[extras.sheet.page]` only
//! when not the default, so older files load and re-save unchanged.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::theme::Color;

/// The corner shown X/Y count from. Display only: stored coordinates are
/// always measured from the upper-left corner, Y down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageOrigin {
    UpperLeft,
    /// Y counts up from the bottom edge of the paper.
    #[default]
    LowerLeft,
}

impl PageOrigin {
    /// The Y shown for stored `y` on a sheet `paper_height_mm` tall, and the
    /// stored Y for a typed `y`: the mapping is its own inverse, so every
    /// display and parse site goes through this one function.
    #[must_use]
    pub fn map_y(self, y: f64, paper_height_mm: f64) -> f64 {
        match self {
            PageOrigin::UpperLeft => y,
            PageOrigin::LowerLeft => paper_height_mm - y,
        }
    }
}

impl std::fmt::Display for PageOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PageOrigin::UpperLeft => "Upper Left",
            PageOrigin::LowerLeft => "Lower Left",
        })
    }
}

/// How a sheet's page is shown. Reading is lenient: a colour that is not
/// `#RRGGBB[AA]` or an unknown origin reads as the default, so a cosmetic
/// setting never stops a sheet from opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SheetPageSettings {
    /// Paper fill; `None` follows the theme's paper colour.
    #[serde(
        default,
        serialize_with = "write_color",
        deserialize_with = "lenient_color",
        skip_serializing_if = "Option::is_none"
    )]
    pub color: Option<Color>,
    #[serde(
        default,
        deserialize_with = "lenient_origin",
        skip_serializing_if = "is_default_origin"
    )]
    pub origin: PageOrigin,
}

impl SheetPageSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

fn is_default_origin(origin: &PageOrigin) -> bool {
    *origin == PageOrigin::default()
}

fn write_color<S: Serializer>(color: &Option<Color>, s: S) -> Result<S::Ok, S::Error> {
    match color {
        Some(color) => s.serialize_str(&color.to_hex()),
        None => s.serialize_none(),
    }
}

/// A value as written, before it is checked against the setting's type.
#[derive(Deserialize)]
#[serde(untagged)]
enum Raw<T> {
    Value(T),
    Other(serde::de::IgnoredAny),
}

fn lenient_color<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Color>, D::Error> {
    Ok(match Raw::<String>::deserialize(d)? {
        Raw::Value(hex) => Color::parse_hex(&hex),
        Raw::Other(_) => None,
    })
}

fn lenient_origin<'de, D: Deserializer<'de>>(d: D) -> Result<PageOrigin, D::Error> {
    Ok(match Raw::<PageOrigin>::deserialize(d)? {
        Raw::Value(origin) => origin,
        Raw::Other(_) => PageOrigin::default(),
    })
}

impl super::SchematicSheet {
    /// Stored Y to the Y shown for this sheet's page origin, and a typed Y
    /// back to the stored one (the mapping is its own inverse). The paper
    /// height comes from the sheet's paper size, Custom included.
    #[must_use]
    pub fn page_y(&self, y: f64) -> f64 {
        let (size, orientation) = super::SheetSize::parse(&self.paper_size);
        self.page.origin.map_y(y, size.dimensions_mm(orientation).1)
    }
}
