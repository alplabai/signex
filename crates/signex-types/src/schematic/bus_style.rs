//! How a bus row stores its style keys in `.snxsch`. Format:
//! `docs/architecture/snxsch-wire-color.md`, "Bus width and colour".

/// Whether a stored bus width is the unset 0, which the row omits.
pub(super) fn is_unset(width_mm: &f64) -> bool {
    *width_mm == 0.0
}

/// The `color` key: `#rrggbbaa` on write, `#RRGGBB[AA]` on read. Another value
/// refuses the file, because the row has no slot to keep it for the next save.
pub(super) mod color {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    use crate::theme::Color;

    /// How much of a bad value the error shows.
    const VALUE_SHOWN_CHARS: usize = 32;

    pub(in crate::schematic) fn serialize<S: Serializer>(
        color: &Option<Color>,
        out: S,
    ) -> Result<S::Ok, S::Error> {
        match color {
            Some(color) => out.serialize_str(&color.to_hex()),
            None => out.serialize_none(),
        }
    }

    pub(in crate::schematic) fn deserialize<'de, D: Deserializer<'de>>(
        input: D,
    ) -> Result<Option<Color>, D::Error> {
        let Some(value) = Option::<String>::deserialize(input)? else {
            return Ok(None);
        };
        Color::parse_hex(&value).map(Some).ok_or_else(|| {
            let shown: String = value.chars().take(VALUE_SHOWN_CHARS).collect();
            D::Error::custom(format!("bus color {shown:?} is not #RRGGBB or #RRGGBBAA"))
        })
    }
}
