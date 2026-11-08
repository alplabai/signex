//! How a wire is styled: the per-net colour override and the net class.
//!
//! Precedence, per property:
//!
//! - **colour** - the wire's own colour wins over the per-net colour override
//!   (the Active Bar net-colour palette), which wins over the class colour,
//!   which wins over the theme's wire colour;
//! - **width** - the wire's own `stroke_width`, else the class width, else
//!   the project default, else the built-in (#2026);
//! - **line style** - the class line style, else the project default, else
//!   solid.
//!
//! The class styles arrive resolved, keyed by element uuid, from the net
//! join (`signex_net::element_class_styles`); nothing here reads the project.

use std::collections::HashMap;

use signex_types::net_class::NetClassStyle;
use signex_types::schematic::{SchematicDefaults, Wire};
use signex_types::theme::Color as ThemeColor;
use uuid::Uuid;

/// Element uuid to the style of the net class that element's net belongs to.
pub type WireClassStyles = HashMap<Uuid, NetClassStyle>;

/// What the renderer needs to style wires, borrowed from app state.
#[derive(Clone, Copy)]
pub struct WireStyling<'a> {
    pub color_overrides: &'a HashMap<Uuid, ThemeColor>,
    pub class_styles: &'a WireClassStyles,
    /// Moves whenever `class_styles` changes (the scene cache compares this
    /// instead of the whole map every frame).
    pub class_revision: u64,
}

/// An owned copy, kept by the scene cache to tell when a rebuild is due.
#[derive(Clone)]
pub(super) struct OwnedWireStyling {
    color_overrides: HashMap<Uuid, ThemeColor>,
    class_revision: u64,
    /// Whether the class map had no entry, for the comparison with "no
    /// styling at all", which draws the same.
    class_empty: bool,
}

impl WireStyling<'_> {
    pub(super) fn to_owned_styling(self) -> OwnedWireStyling {
        OwnedWireStyling {
            color_overrides: self.color_overrides.clone(),
            class_revision: self.class_revision,
            class_empty: self.class_styles.is_empty(),
        }
    }
}

impl OwnedWireStyling {
    pub(super) fn matches(&self, other: Option<WireStyling<'_>>) -> bool {
        match other {
            Some(styling) => {
                self.color_overrides == *styling.color_overrides
                    && self.class_revision == styling.class_revision
            }
            None => self.color_overrides.is_empty() && self.class_empty,
        }
    }
}

/// The colour a wire is drawn with, before focus dimming: its own colour,
/// else the net override, else the class colour, else `None` for the
/// caller's theme default.
#[must_use]
pub fn wire_color(styling: Option<WireStyling<'_>>, wire: &Wire) -> Option<ThemeColor> {
    wire.color.or_else(|| {
        let styling = styling?;
        styling
            .color_overrides
            .get(&wire.uuid)
            .copied()
            .or_else(|| styling.class_styles.get(&wire.uuid).and_then(|s| s.color))
    })
}

/// The colour a junction dot is drawn with, before focus dimming, or `None`
/// for the theme's junction colour.
///
/// A junction is tinted only by its net's class colour; a per-net colour
/// override then takes precedence over that, as it does for a wire. An
/// override alone leaves the dot at the theme colour, so a sheet with no net
/// classes draws its junctions exactly as it did before classes existed. The
/// PDF export applies the same rule (`signex-output`'s `junction_rgb`).
#[must_use]
pub fn junction_color(styling: Option<WireStyling<'_>>, junction: Uuid) -> Option<ThemeColor> {
    let styling = styling?;
    let class_color = styling.class_styles.get(&junction)?.color?;
    Some(
        styling
            .color_overrides
            .get(&junction)
            .copied()
            .unwrap_or(class_color),
    )
}

/// What a wire's drawn width depends on besides the wire: its net class and
/// the project defaults (#2026). Hit test and item bounds take it, so a wire
/// is grabbed and boxed at the width it is drawn at.
#[derive(Clone, Copy, Default)]
pub struct WireWidths<'a> {
    pub class_styles: Option<&'a WireClassStyles>,
    pub defaults: SchematicDefaults,
}

impl WireWidths<'_> {
    /// The width `wire` is drawn at.
    #[must_use]
    pub fn wire_mm(&self, wire: &Wire) -> f64 {
        let class = self.class_styles.and_then(|styles| styles.get(&wire.uuid));
        drawn_wire_width_mm(wire, class, &self.defaults)
    }

    /// The width a bus is drawn at.
    #[must_use]
    pub fn bus_mm(&self) -> f64 {
        self.defaults.bus_mm()
    }
}

/// The width in mm a wire is drawn at, and so the width it is hit at.
#[must_use]
pub fn drawn_wire_width_mm(
    wire: &Wire,
    class: Option<&NetClassStyle>,
    defaults: &SchematicDefaults,
) -> f64 {
    defaults.wire_mm(
        wire.stroke_width,
        class.and_then(|style| style.wire_width_mm),
    )
}

#[cfg(test)]
mod tests {
    use signex_types::net_class::NetLineStyle;

    use super::*;

    fn wire_of(uuid: Uuid) -> Wire {
        Wire {
            uuid,
            ..Default::default()
        }
    }

    /// The width every stock symbol's graphics are stored at, which its pins
    /// draw at (`symbol_stroke_mm`).
    fn stock_pin_width_mm() -> f64 {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/stock-library/symbols");
        let mut widths = Vec::new();
        for entry in std::fs::read_dir(dir).expect("stock symbols") {
            let text = std::fs::read_to_string(entry.expect("entry").path()).expect("symbol");
            for line in text.lines() {
                if let Some(v) = line.trim().strip_prefix("stroke_width = ") {
                    widths.push(v.parse::<f64>().expect("width"));
                }
            }
        }
        assert!(!widths.is_empty(), "no stock graphic widths found");
        SchematicDefaults::BUILT_IN.symbol_stroke_mm(widths)
    }

    const B: &SchematicDefaults = &SchematicDefaults::BUILT_IN;

    #[test]
    fn a_default_wire_draws_as_wide_as_a_stock_pin() {
        let wire = Wire::default();
        assert_eq!(wire.stroke_width, 0.0);
        assert_eq!(drawn_wire_width_mm(&wire, None, B), stock_pin_width_mm());
        // An explicit width keeps its value, over a class width too (#2026).
        let wide = Wire {
            stroke_width: 0.5,
            ..Wire::default()
        };
        assert_eq!(drawn_wire_width_mm(&wide, None, B), 0.5);
        assert_eq!(
            drawn_wire_width_mm(&wide, Some(&style(None, Some(0.4))), B),
            0.5
        );
    }

    fn style(color: Option<ThemeColor>, width: Option<f64>) -> NetClassStyle {
        NetClassStyle {
            color,
            wire_width_mm: width,
            line: Some(NetLineStyle::Dashed),
        }
    }

    #[test]
    fn override_beats_class_colour_beats_theme() {
        let id = Uuid::from_u128(1);
        let (red, blue) = (
            ThemeColor::new(255, 0, 0, 255),
            ThemeColor::new(0, 0, 255, 255),
        );
        let mut overrides = HashMap::new();
        let mut classes = WireClassStyles::new();
        classes.insert(id, style(Some(blue), None));

        let only_class = WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
            class_revision: 0,
        };
        assert_eq!(wire_color(Some(only_class), &wire_of(id)), Some(blue));
        assert_eq!(wire_color(None, &wire_of(id)), None);

        overrides.insert(id, red);
        let both = WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
            class_revision: 0,
        };
        assert_eq!(wire_color(Some(both), &wire_of(id)), Some(red));

        let own = Wire {
            color: Some(ThemeColor::new(0, 255, 0, 255)),
            ..wire_of(id)
        };
        assert_eq!(
            wire_color(Some(both), &own),
            own.color,
            "a wire's own colour beats the net override and the class"
        );
    }

    #[test]
    fn class_without_colour_leaves_the_theme_colour() {
        let id = Uuid::from_u128(1);
        let overrides = HashMap::new();
        let mut classes = WireClassStyles::new();
        classes.insert(id, style(None, Some(0.5)));
        let styling = WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
            class_revision: 0,
        };
        assert_eq!(wire_color(Some(styling), &wire_of(id)), None);
    }

    /// #2026: explicit > net class > project > built-in.
    #[test]
    fn class_width_fills_an_unset_wire_width() {
        let wire = Wire {
            stroke_width: 0.15,
            ..Default::default()
        };
        let unset = Wire::default();
        let wide = style(None, Some(0.6));
        let project = SchematicDefaults {
            wire_width_mm: 0.3,
            ..SchematicDefaults::BUILT_IN
        };
        assert_eq!(drawn_wire_width_mm(&wire, None, B), 0.15);
        assert_eq!(drawn_wire_width_mm(&wire, Some(&wide), B), 0.15);
        assert_eq!(drawn_wire_width_mm(&unset, Some(&wide), &project), 0.6);
        assert_eq!(drawn_wire_width_mm(&unset, None, &project), 0.3);
        let no_width = style(None, None);
        assert_eq!(drawn_wire_width_mm(&unset, Some(&no_width), B), 0.254);
    }
}
