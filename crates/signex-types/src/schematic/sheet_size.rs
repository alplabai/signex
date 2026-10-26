//! The typed sheet-size model: one source of truth for the paper a schematic
//! sheet is drawn and printed on.
//!
//! A sheet stores its size as a text token in `SchematicSheet.paper_size`
//! (the `.snxsch` manifest key `paper_size`). [`SheetSize::parse`] reads that
//! token and [`SheetSize::token`] writes it back, so the canvas frame, the
//! PDF page and the Properties picker all read the same value and no
//! consumer keeps a private table.
//!
//! Token grammar, `<size>[ portrait]`:
//!
//! - `<size>` is a legacy name (`A0`-`A5`, `B5`, `Letter`, `Legal`,
//!   `Tabloid`), an ANSI size (`ANSI A`-`ANSI E`) or `Custom <w>x<h>` in
//!   millimetres.
//! - A named size is landscape unless the token ends in ` portrait`.
//!   Every token written before this model existed is therefore unchanged.
//! - A token this model does not know is kept verbatim as
//!   [`SheetSize::Unrecognized`], so a file written by a newer build keeps
//!   its text through a load and save.

use std::fmt;

/// Which way a sheet's long edge runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetOrientation {
    /// Long edge horizontal. The default, and what every legacy file means.
    #[default]
    Landscape,
    Portrait,
}

/// A schematic sheet's paper size.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetSize {
    IsoA0,
    IsoA1,
    IsoA2,
    IsoA3,
    IsoA4,
    IsoA5,
    IsoB5,
    AnsiA,
    AnsiB,
    AnsiC,
    AnsiD,
    AnsiE,
    Letter,
    Legal,
    Tabloid,
    /// A user-chosen page, in millimetres, exactly as drawn: orientation
    /// does not rotate it.
    Custom {
        width_mm: f64,
        height_mm: f64,
    },
    /// A token this build does not recognise, kept as written.
    Unrecognized(String),
}

/// Largest custom edge accepted from a file, matching the PDF page limit.
const MAX_CUSTOM_EDGE_MM: f64 = 5080.0;

impl SheetSize {
    /// Every named size the picker offers, in display order.
    pub const PICKABLE: [SheetSize; 15] = [
        SheetSize::IsoA0,
        SheetSize::IsoA1,
        SheetSize::IsoA2,
        SheetSize::IsoA3,
        SheetSize::IsoA4,
        SheetSize::IsoA5,
        SheetSize::IsoB5,
        SheetSize::AnsiA,
        SheetSize::AnsiB,
        SheetSize::AnsiC,
        SheetSize::AnsiD,
        SheetSize::AnsiE,
        SheetSize::Letter,
        SheetSize::Legal,
        SheetSize::Tabloid,
    ];

    /// Read a stored `paper_size` token into its size and orientation.
    pub fn parse(token: &str) -> (SheetSize, SheetOrientation) {
        let (name, portrait) = match token.strip_suffix(" portrait") {
            Some(name) => (name, true),
            None => (token, false),
        };
        let orientation = if portrait {
            SheetOrientation::Portrait
        } else {
            SheetOrientation::Landscape
        };
        let named = match name {
            "A0" => Some(SheetSize::IsoA0),
            "A1" => Some(SheetSize::IsoA1),
            "A2" => Some(SheetSize::IsoA2),
            "A3" => Some(SheetSize::IsoA3),
            "A4" => Some(SheetSize::IsoA4),
            "A5" => Some(SheetSize::IsoA5),
            "B5" => Some(SheetSize::IsoB5),
            "ANSI A" => Some(SheetSize::AnsiA),
            "ANSI B" => Some(SheetSize::AnsiB),
            "ANSI C" => Some(SheetSize::AnsiC),
            "ANSI D" => Some(SheetSize::AnsiD),
            "ANSI E" => Some(SheetSize::AnsiE),
            "Letter" => Some(SheetSize::Letter),
            "Legal" => Some(SheetSize::Legal),
            "Tabloid" => Some(SheetSize::Tabloid),
            _ => None,
        };
        if let Some(size) = named {
            return (size, orientation);
        }
        if let Some(custom) = parse_custom(name) {
            let orientation = custom.orientation();
            return (custom, orientation);
        }
        (
            SheetSize::Unrecognized(token.to_string()),
            SheetOrientation::Landscape,
        )
    }

    /// The token to store for this size in `orientation`.
    ///
    /// Landscape named sizes use the bare legacy name, so a sheet that never
    /// chose a portrait page serialises exactly as it did before.
    pub fn token(&self, orientation: SheetOrientation) -> String {
        match self {
            SheetSize::Unrecognized(text) => text.clone(),
            SheetSize::Custom {
                width_mm,
                height_mm,
            } => format!("Custom {width_mm}x{height_mm}"),
            named => {
                let name = named.name();
                match orientation {
                    SheetOrientation::Landscape => name.to_string(),
                    SheetOrientation::Portrait => format!("{name} portrait"),
                }
            }
        }
    }

    /// The bare name of a named size; custom and unrecognised sizes have no
    /// fixed name and return their token text.
    fn name(&self) -> &str {
        match self {
            SheetSize::IsoA0 => "A0",
            SheetSize::IsoA1 => "A1",
            SheetSize::IsoA2 => "A2",
            SheetSize::IsoA3 => "A3",
            SheetSize::IsoA4 => "A4",
            SheetSize::IsoA5 => "A5",
            SheetSize::IsoB5 => "B5",
            SheetSize::AnsiA => "ANSI A",
            SheetSize::AnsiB => "ANSI B",
            SheetSize::AnsiC => "ANSI C",
            SheetSize::AnsiD => "ANSI D",
            SheetSize::AnsiE => "ANSI E",
            SheetSize::Letter => "Letter",
            SheetSize::Legal => "Legal",
            SheetSize::Tabloid => "Tabloid",
            SheetSize::Custom { .. } => "Custom",
            SheetSize::Unrecognized(text) => text,
        }
    }

    /// Short edge and long edge in millimetres, for a named size.
    fn short_long_mm(&self) -> Option<(f64, f64)> {
        Some(match self {
            SheetSize::IsoA0 => (841.0, 1189.0),
            SheetSize::IsoA1 => (594.0, 841.0),
            SheetSize::IsoA2 => (420.0, 594.0),
            SheetSize::IsoA3 => (297.0, 420.0),
            SheetSize::IsoA4 => (210.0, 297.0),
            SheetSize::IsoA5 => (148.0, 210.0),
            SheetSize::IsoB5 => (182.0, 257.0),
            SheetSize::AnsiA | SheetSize::Letter => (215.9, 279.4),
            SheetSize::AnsiB | SheetSize::Tabloid => (279.4, 431.8),
            SheetSize::AnsiC => (431.8, 558.8),
            SheetSize::AnsiD => (558.8, 863.6),
            SheetSize::AnsiE => (863.6, 1117.6),
            SheetSize::Legal => (215.9, 355.6),
            SheetSize::Custom { .. } | SheetSize::Unrecognized(_) => return None,
        })
    }

    /// Whether this is a size the model understands (not [`Unrecognized`]).
    ///
    /// [`Unrecognized`]: SheetSize::Unrecognized
    pub fn is_recognized(&self) -> bool {
        !matches!(self, SheetSize::Unrecognized(_))
    }

    /// Page `(width_mm, height_mm)` in `orientation`. A custom size is
    /// returned as drawn; an unrecognised one is drawn as A4 landscape, which
    /// is what the canvas has always shown for it.
    pub fn dimensions_mm(&self, orientation: SheetOrientation) -> (f64, f64) {
        match self {
            SheetSize::Custom {
                width_mm,
                height_mm,
            } => (*width_mm, *height_mm),
            SheetSize::Unrecognized(_) => {
                SheetSize::IsoA4.dimensions_mm(SheetOrientation::Landscape)
            }
            named => {
                let (short, long) = named.short_long_mm().unwrap_or((210.0, 297.0));
                match orientation {
                    SheetOrientation::Landscape => (long, short),
                    SheetOrientation::Portrait => (short, long),
                }
            }
        }
    }

    /// The orientation a custom page's own dimensions describe.
    fn orientation(&self) -> SheetOrientation {
        match self {
            SheetSize::Custom {
                width_mm,
                height_mm,
            } if height_mm > width_mm => SheetOrientation::Portrait,
            _ => SheetOrientation::Landscape,
        }
    }
}

fn parse_custom(name: &str) -> Option<SheetSize> {
    let dims = name.strip_prefix("Custom ")?;
    let (w, h) = dims.split_once('x')?;
    let width_mm: f64 = w.parse().ok()?;
    let height_mm: f64 = h.parse().ok()?;
    let valid = |edge: f64| edge.is_finite() && edge > 0.0 && edge <= MAX_CUSTOM_EDGE_MM;
    (valid(width_mm) && valid(height_mm)).then_some(SheetSize::Custom {
        width_mm,
        height_mm,
    })
}

impl fmt::Display for SheetSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SheetSize::Custom {
                width_mm,
                height_mm,
            } => write!(f, "Custom {width_mm} x {height_mm} mm"),
            other => f.write_str(other.name()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: [&str; 10] = [
        "A0", "A1", "A2", "A3", "A4", "A5", "B5", "Letter", "Legal", "Tabloid",
    ];

    #[test]
    fn every_legacy_token_parses_and_writes_back_unchanged() {
        for token in LEGACY {
            let (size, orientation) = SheetSize::parse(token);
            assert!(size.is_recognized(), "{token}");
            assert_eq!(orientation, SheetOrientation::Landscape, "{token}");
            assert_eq!(size.token(orientation), token);
        }
    }

    #[test]
    fn legacy_landscape_dimensions_match_the_old_canvas_table() {
        let expect = [
            ("A0", (1189.0, 841.0)),
            ("A4", (297.0, 210.0)),
            ("A5", (210.0, 148.0)),
            ("B5", (257.0, 182.0)),
            ("Letter", (279.4, 215.9)),
            ("Legal", (355.6, 215.9)),
            ("Tabloid", (431.8, 279.4)),
        ];
        for (token, dims) in expect {
            let (size, orientation) = SheetSize::parse(token);
            assert_eq!(size.dimensions_mm(orientation), dims, "{token}");
        }
    }

    #[test]
    fn ansi_sizes_and_portrait_round_trip() {
        for token in ["ANSI A", "ANSI E", "A3 portrait", "ANSI C portrait"] {
            let (size, orientation) = SheetSize::parse(token);
            assert!(size.is_recognized(), "{token}");
            assert_eq!(size.token(orientation), token);
        }
        let (size, orientation) = SheetSize::parse("A4 portrait");
        assert_eq!(orientation, SheetOrientation::Portrait);
        assert_eq!(size.dimensions_mm(orientation), (210.0, 297.0));
    }

    #[test]
    fn custom_size_round_trips_and_is_not_rotated() {
        let (size, orientation) = SheetSize::parse("Custom 300x200.5");
        assert_eq!(
            size,
            SheetSize::Custom {
                width_mm: 300.0,
                height_mm: 200.5
            }
        );
        assert_eq!(size.dimensions_mm(orientation), (300.0, 200.5));
        assert_eq!(size.token(orientation), "Custom 300x200.5");
    }

    #[test]
    fn unknown_and_malformed_tokens_keep_their_text() {
        for token in [
            "",
            "a4",
            "Foolscap",
            "Custom 0x5",
            "Custom axb",
            "Custom 9999x10",
        ] {
            let (size, orientation) = SheetSize::parse(token);
            assert!(!size.is_recognized(), "{token:?}");
            assert_eq!(size.token(orientation), token);
        }
    }

    #[test]
    fn unrecognized_draws_as_a4_landscape() {
        let (size, orientation) = SheetSize::parse("Foolscap");
        assert_eq!(size.dimensions_mm(orientation), (297.0, 210.0));
    }

    #[test]
    fn every_pickable_size_has_dimensions_and_a_stable_token() {
        for size in SheetSize::PICKABLE {
            let token = size.token(SheetOrientation::Landscape);
            assert_eq!(SheetSize::parse(&token).0, size);
            let (w, h) = size.dimensions_mm(SheetOrientation::Landscape);
            assert!(w > h, "{token}");
        }
    }
}
