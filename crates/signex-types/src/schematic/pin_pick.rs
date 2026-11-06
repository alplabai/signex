//! Which pin of a placed symbol a point picks: the one rule behind the hover
//! card, click-select and the net highlight. (The right-click menu picks the
//! part only and does not use it.)
//!
//! A pin wins only within the pick tolerance of its line (tip to body end).
//! The inside of the body belongs to the part, filled or not, so a point
//! inside the body outline never picks a pin. Pins the symbol hides are not
//! drawn and cannot be picked.

use uuid::Uuid;

use super::{
    PIN_LENGTH_MM, Point, SchematicSheet, SymbolPrimitive, SymbolTransform, instance_pins,
    lower_symbol, pin_direction, primitives_bounds,
};
use crate::segment::point_to_segment_distance_sq;

/// Least distance (mm) from a pin's line at which a point still picks it
/// when zoomed in; zoomed out, the px floor below is larger.
pub const SCHEMATIC_PIN_PICK_TOLERANCE_MM: f64 = 0.64;

/// The pick radius in screen px: about 1.3 mm at 100 % zoom, and it keeps a
/// pin pickable when the sheet is zoomed far out.
pub const SCHEMATIC_PIN_PICK_TOLERANCE_PX: f64 = 4.0;

/// The pick tolerance in mm at `scale_px_per_mm`: the larger of the mm and
/// the px floor.
#[must_use]
pub fn pin_pick_tolerance_mm(scale_px_per_mm: f64) -> f64 {
    if scale_px_per_mm.is_finite() && scale_px_per_mm > 0.0 {
        SCHEMATIC_PIN_PICK_TOLERANCE_MM.max(SCHEMATIC_PIN_PICK_TOLERANCE_PX / scale_px_per_mm)
    } else {
        SCHEMATIC_PIN_PICK_TOLERANCE_MM
    }
}

/// A pin of a placed symbol, found by [`symbol_pin_at`].
#[derive(Debug, Clone, PartialEq)]
pub struct PickedPin {
    /// Pin number, else its name: the key the netlist knows the pin by.
    pub key: String,
}

/// The pin of `symbol` nearest `at` within `tolerance_mm` of its line, if
/// `at` is not inside the symbol's body.
#[must_use]
pub fn symbol_pin_at(
    document: &SchematicSheet,
    symbol: Uuid,
    at: Point,
    tolerance_mm: f64,
) -> Option<PickedPin> {
    let sym = document.symbols.iter().find(|s| s.uuid == symbol)?;
    let lib = document.resolve_lib_symbol(sym)?;
    if inside_body(
        &lower_symbol(&lib, sym, &super::SchematicDefaults::BUILT_IN),
        at,
    ) {
        return None;
    }
    pin_lines(&lib, sym)
        .filter_map(|line| {
            let distance_sq = point_to_segment_distance_sq(
                (at.x, at.y),
                (line.tip.x, line.tip.y),
                (line.inner.x, line.inner.y),
            );
            (distance_sq <= tolerance_mm * tolerance_mm)
                .then_some((distance_sq, PickedPin { key: line.key }))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, pin)| pin)
}

/// The drawn line of one visible pin of a placed symbol, in sheet mm.
#[derive(Debug, Clone, PartialEq)]
pub struct PinLine {
    /// Pin number, else its name (as [`PickedPin::key`]).
    pub key: String,
    /// The connecting end.
    pub tip: Point,
    /// The end at the body.
    pub inner: Point,
}

fn pin_lines<'a>(
    lib: &'a super::LibSymbol,
    sym: &'a super::Symbol,
) -> impl Iterator<Item = PinLine> + 'a {
    let xform = SymbolTransform::from_symbol(sym);
    instance_pins(lib, sym)
        .filter(|lp| lp.pin.visible)
        .map(move |lp| {
            let pin = &lp.pin;
            let (dx, dy) = pin_direction(pin.rotation);
            let len = if pin.length > 0.0 {
                pin.length
            } else {
                PIN_LENGTH_MM
            };
            PinLine {
                key: if pin.number.is_empty() {
                    pin.name.clone()
                } else {
                    pin.number.clone()
                },
                tip: xform.apply(pin.position),
                inner: xform.apply(Point::new(
                    pin.position.x + dx * len,
                    pin.position.y + dy * len,
                )),
            }
        })
}

/// The line of the first visible pin of `symbol` whose key is `key`: what a
/// pin click highlights.
#[must_use]
pub fn symbol_pin_line(document: &SchematicSheet, symbol: Uuid, key: &str) -> Option<PinLine> {
    let sym = document.symbols.iter().find(|s| s.uuid == symbol)?;
    let lib = document.resolve_lib_symbol(sym)?;
    pin_lines(&lib, sym).find(|line| line.key == key)
}

/// Whether `at` is inside the drawn body: the box through the middle of its
/// outline strokes (the outer bounds pulled in by half the stroke), so a pin that
/// meets the outline still picks at its end. Everything inside the outline
/// belongs to the part, at any zoom.
fn inside_body(primitives: &[SymbolPrimitive], at: Point) -> bool {
    let body: Vec<SymbolPrimitive> = primitives
        .iter()
        .filter(|p| {
            matches!(
                p,
                SymbolPrimitive::Path { .. }
                    | SymbolPrimitive::Circle { .. }
                    | SymbolPrimitive::Arc { .. }
            )
        })
        .cloned()
        .collect();
    let Some(bounds) = primitives_bounds(&body) else {
        return false;
    };
    let edge = body
        .iter()
        .filter_map(SymbolPrimitive::stroke_width)
        .fold(0.0, f64::max)
        * 0.5;
    at.x > bounds.min_x + edge
        && at.x < bounds.max_x - edge
        && at.y > bounds.min_y + edge
        && at.y < bounds.max_y - edge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_px_radius_wins_unless_zoomed_far_in() {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(pin_pick_tolerance_mm(3.0), 4.0 / 3.0));
        assert!(near(pin_pick_tolerance_mm(0.3), 4.0 / 0.3));
        assert_eq!(pin_pick_tolerance_mm(48.0), SCHEMATIC_PIN_PICK_TOLERANCE_MM);
        assert_eq!(
            pin_pick_tolerance_mm(f64::NAN),
            SCHEMATIC_PIN_PICK_TOLERANCE_MM
        );
    }
}
