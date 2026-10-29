//! How a wire is styled: the per-net colour override and the net class.
//!
//! Precedence, per property:
//!
//! - **colour** - the per-net colour override (the Active Bar net-colour
//!   palette) wins over the class colour, which wins over the theme's wire
//!   colour;
//! - **width** - the class width, else the wire's own `stroke_width`;
//! - **line style** - the class line style, else solid.
//!
//! The class styles arrive resolved, keyed by element uuid, from the net
//! join (`signex_net::element_class_styles`); nothing here reads the project.

use std::collections::HashMap;

use signex_types::net_class::NetClassStyle;
use signex_types::schematic::{SCHEMATIC_RENDER_MIN_STROKE_MM, Wire};
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

/// The colour a wire is drawn with, before focus dimming: the override, else
/// the class colour, else `None` for the caller's theme default.
#[must_use]
pub fn wire_color(styling: Option<WireStyling<'_>>, wire: Uuid) -> Option<ThemeColor> {
    let styling = styling?;
    styling
        .color_overrides
        .get(&wire)
        .copied()
        .or_else(|| styling.class_styles.get(&wire).and_then(|s| s.color))
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

/// The width in mm a wire is drawn at, and so the width it is hit at.
#[must_use]
pub fn drawn_wire_width_mm(wire: &Wire, class: Option<&NetClassStyle>) -> f64 {
    class
        .and_then(|style| style.wire_width_mm)
        .unwrap_or(wire.stroke_width)
        .max(SCHEMATIC_RENDER_MIN_STROKE_MM)
}

#[cfg(test)]
mod tests {
    use signex_types::net_class::NetLineStyle;

    use super::*;

    fn style(color: Option<ThemeColor>, width: Option<f64>) -> NetClassStyle {
        NetClassStyle {
            color,
            wire_width_mm: width,
            line: NetLineStyle::Dashed,
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
        assert_eq!(wire_color(Some(only_class), id), Some(blue));
        assert_eq!(wire_color(None, id), None);

        overrides.insert(id, red);
        let both = WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
            class_revision: 0,
        };
        assert_eq!(wire_color(Some(both), id), Some(red));
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
        assert_eq!(wire_color(Some(styling), id), None);
    }

    #[test]
    fn class_width_replaces_the_wire_width() {
        let wire = Wire {
            stroke_width: 0.15,
            ..Default::default()
        };
        assert_eq!(drawn_wire_width_mm(&wire, None), 0.15);
        let wide = style(None, Some(0.6));
        assert_eq!(drawn_wire_width_mm(&wire, Some(&wide)), 0.6);
        let no_width = style(None, None);
        assert_eq!(drawn_wire_width_mm(&wire, Some(&no_width)), 0.15);
    }
}
