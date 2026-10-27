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
//!   millimetres. The short names `A`-`E`, `USLetter` and `USLegal` read as
//!   `ANSI A`-`ANSI E`, `Letter` and `Legal`; their text is not rewritten.
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

/// Largest page edge a PDF viewer accepts (14 400 pt). A custom edge never
/// exceeds it, so the PDF page-setup modal can always take the sheet's size.
pub const MAX_PAGE_EDGE_MM: f64 = 5080.0;
/// Smallest custom page edge. Below this the default PDF margins would
/// consume the whole page.
pub const MIN_CUSTOM_EDGE_MM: f64 = 10.0;

impl fmt::Display for SheetOrientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SheetOrientation::Landscape => "Landscape",
            SheetOrientation::Portrait => "Portrait",
        })
    }
}

impl SheetSize {
    /// A custom page with each edge clamped to
    /// [`MIN_CUSTOM_EDGE_MM`]..=[`MAX_PAGE_EDGE_MM`] and rounded to 0.1 mm,
    /// so the model never holds a size the PDF page-setup modal would
    /// refuse and the stored token stays short. A non-finite edge reads as
    /// the minimum.
    pub fn custom(width_mm: f64, height_mm: f64) -> SheetSize {
        let edge = |mm: f64| {
            if !mm.is_finite() {
                return MIN_CUSTOM_EDGE_MM;
            }
            ((mm * 10.0).round() / 10.0).clamp(MIN_CUSTOM_EDGE_MM, MAX_PAGE_EDGE_MM)
        };
        SheetSize::Custom {
            width_mm: edge(width_mm),
            height_mm: edge(height_mm),
        }
    }

    /// The token after the user turns `token`'s sheet to `target`. A named
    /// size gains or loses the ` portrait` suffix; a custom page swaps its
    /// edges; an unrecognised token is returned as written.
    pub fn reoriented_token(token: &str, target: SheetOrientation) -> String {
        let (size, current) = SheetSize::parse(token);
        match size {
            SheetSize::Unrecognized(text) => text,
            SheetSize::Custom {
                width_mm,
                height_mm,
            } => {
                if current == target {
                    token.to_string()
                } else {
                    SheetSize::Custom {
                        width_mm: height_mm,
                        height_mm: width_mm,
                    }
                    .token(target)
                }
            }
            named => named.token(target),
        }
    }

    /// The token after the user picks `picked` (a bare picker token) on a
    /// sheet currently stored as `current`: a named size keeps the sheet's
    /// orientation, anything else is taken as written.
    pub fn picked_token(current: &str, picked: &str) -> String {
        let (_, orientation) = SheetSize::parse(current);
        match SheetSize::parse(picked) {
            (size, SheetOrientation::Landscape) if size.short_long_mm().is_some() => {
                size.token(orientation)
            }
            _ => picked.to_string(),
        }
    }

    /// The token after the user types a new custom `width_mm` and/or
    /// `height_mm` on a sheet currently stored as `current`. The other edge
    /// starts from the sheet's drawn size; both are clamped by
    /// [`SheetSize::custom`].
    pub fn resized_token(current: &str, width_mm: Option<f64>, height_mm: Option<f64>) -> String {
        let (size, orientation) = SheetSize::parse(current);
        let (w, h) = size.dimensions_mm(orientation);
        SheetSize::custom(width_mm.unwrap_or(w), height_mm.unwrap_or(h)).token(orientation)
    }

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
            // Short names the PDF page table read before this model (#1199).
            "A" => Some(SheetSize::AnsiA),
            "B" => Some(SheetSize::AnsiB),
            "C" => Some(SheetSize::AnsiC),
            "D" => Some(SheetSize::AnsiD),
            "E" => Some(SheetSize::AnsiE),
            "USLetter" => Some(SheetSize::Letter),
            "USLegal" => Some(SheetSize::Legal),
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
    let valid =
        |edge: f64| edge.is_finite() && (MIN_CUSTOM_EDGE_MM..=MAX_PAGE_EDGE_MM).contains(&edge);
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
            "Custom 9.9x100",
            "Custom 100x5080.1",
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
    fn custom_edges_clamp_to_what_the_pdf_modal_accepts() {
        assert_eq!(
            SheetSize::custom(1.0, 99999.0),
            SheetSize::Custom {
                width_mm: MIN_CUSTOM_EDGE_MM,
                height_mm: MAX_PAGE_EDGE_MM
            }
        );
        assert_eq!(
            SheetSize::custom(f64::NAN, 50.04).token(SheetOrientation::Landscape),
            "Custom 10x50"
        );
        let token = SheetSize::custom(210.123, 100.0).token(SheetOrientation::Landscape);
        assert_eq!(token, "Custom 210.1x100");
        assert!(SheetSize::parse(&token).0.is_recognized());
    }

    #[test]
    fn reorienting_named_custom_and_unknown_tokens() {
        use SheetOrientation::{Landscape, Portrait};
        assert_eq!(SheetSize::reoriented_token("A4", Portrait), "A4 portrait");
        assert_eq!(SheetSize::reoriented_token("A4 portrait", Landscape), "A4");
        assert_eq!(
            SheetSize::reoriented_token("Custom 300x200", Portrait),
            "Custom 200x300"
        );
        assert_eq!(
            SheetSize::reoriented_token("Custom 300x200", Landscape),
            "Custom 300x200"
        );
        assert_eq!(
            SheetSize::reoriented_token("Foolscap", Portrait),
            "Foolscap"
        );
    }

    #[test]
    fn picking_a_size_keeps_the_orientation() {
        assert_eq!(SheetSize::picked_token("A4 portrait", "A3"), "A3 portrait");
        assert_eq!(SheetSize::picked_token("A4", "A3"), "A3");
        assert_eq!(
            SheetSize::picked_token("Custom 200x300", "A3"),
            "A3 portrait"
        );
        assert_eq!(SheetSize::picked_token("A4", "A3 portrait"), "A3 portrait");
    }

    #[test]
    fn resizing_one_edge_keeps_the_other() {
        assert_eq!(
            SheetSize::resized_token("A4", Some(400.0), None),
            "Custom 400x210"
        );
        assert_eq!(
            SheetSize::resized_token("Custom 400x210", None, Some(2.0)),
            "Custom 400x10"
        );
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

    /// #1199 — the short names the PDF page table read before #1188 are the
    /// sizes they always meant, in either orientation, not A4.
    #[test]
    fn short_ansi_and_us_names_read_as_their_sizes() {
        let aliases = [
            ("A", SheetSize::AnsiA),
            ("B", SheetSize::AnsiB),
            ("C", SheetSize::AnsiC),
            ("D", SheetSize::AnsiD),
            ("E", SheetSize::AnsiE),
            ("USLetter", SheetSize::Letter),
            ("USLegal", SheetSize::Legal),
        ];
        for (token, size) in aliases {
            assert_eq!(
                SheetSize::parse(token),
                (size.clone(), SheetOrientation::Landscape),
                "{token}"
            );
            assert_eq!(
                SheetSize::parse(&format!("{token} portrait")),
                (size, SheetOrientation::Portrait),
                "{token} portrait"
            );
        }
        let (size, orientation) = SheetSize::parse("USLetter");
        assert_eq!(size.dimensions_mm(orientation), (279.4, 215.9));
    }
}
