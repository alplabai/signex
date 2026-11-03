//! Where a placed symbol's pin numbers and pin names sit, in world mm.
//!
//! The canvas, the SVG/PDF export and the symbol editor all place pin text
//! from [`pin_text_layout`], so they cannot disagree on side, alignment or
//! angle.

use super::{
    HAlign, LibSymbol, PIN_LENGTH_MM, Point, Symbol, SymbolTransform, VAlign, instance_pins,
    pin_direction,
};
use std::f64::consts::{FRAC_PI_2, PI};

/// Distance of a pin number from the pin line, mm.
pub const PIN_NUMBER_OFFSET_MM: f64 = 0.254;

/// How far along the pin, from its tip, a number is anchored (fraction of
/// the pin length).
pub const PIN_NUMBER_ALONG_RATIO: f64 = 0.18;

#[cfg(test)]
#[path = "pin_labels_tests.rs"]
mod tests;

/// Which text of a pin a [`PinLabel`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLabelKind {
    /// Outside the body, beside the pin line.
    Number,
    /// Inside the body, past the pin's body end.
    Name,
}

/// One pin text, drawn at `position` turned by `rotation_rad`.
#[derive(Debug, Clone, PartialEq)]
pub struct PinLabel {
    pub kind: PinLabelKind,
    /// The text as stored (before any expression evaluation).
    pub text: String,
    /// Number of the pin the text belongs to.
    pub pin_number: String,
    pub position: Point,
    /// Clockwise screen angle (Y-down), in `[-pi/2, pi/2)`: text is never
    /// upside-down, and every vertical pin reads bottom-to-top.
    pub rotation_rad: f64,
    /// Alignment in the text's own frame.
    pub h_align: HAlign,
    pub v_align: VAlign,
}

/// Placement of one pin's two texts, in a Y-down frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinTextLayout {
    pub rotation_rad: f64,
    pub number_pos: Point,
    pub name_pos: Point,
    /// Alignment of the name in the text's own frame: `Left` when the text
    /// runs along the pin direction, `Right` when the turn folded it.
    pub name_h_align: HAlign,
}

/// Lay out the texts of a pin whose connection point is `tip`, pointing
/// `dir` (unit, tip to body end) for `length`, in a Y-down frame.
///
/// Text turns with the pin and is folded into `[-pi/2, pi/2)`. Every
/// vertical pin, pointing up or down, reads bottom-to-top at `-pi/2`, so the
/// pins on one edge all read the same way and their numbers sit on the same
/// side. The name sits `name_offset` past the body end, running away from
/// the edge. The number sits near the tip, on the side the text's top faces.
pub fn pin_text_layout(
    tip: Point,
    dir: (f64, f64),
    length: f64,
    name_offset: f64,
) -> PinTextLayout {
    let (rotation, folded) = folded_angle(dir);
    // Use the snapped direction below too, so positions carry no noise.
    let dir = (rotation.cos(), rotation.sin());
    let dir = if folded { (-dir.0, -dir.1) } else { dir };
    // The text's top, as a screen direction.
    let up = (rotation.sin(), -rotation.cos());
    let along = length * PIN_NUMBER_ALONG_RATIO;
    PinTextLayout {
        rotation_rad: rotation,
        number_pos: Point::new(
            tip.x + dir.0 * along + up.0 * PIN_NUMBER_OFFSET_MM,
            tip.y + dir.1 * along + up.1 * PIN_NUMBER_OFFSET_MM,
        ),
        name_pos: Point::new(
            tip.x + dir.0 * (length + name_offset),
            tip.y + dir.1 * (length + name_offset),
        ),
        name_h_align: if folded { HAlign::Right } else { HAlign::Left },
    }
}

/// Angle tolerance of [`folded_angle`]. A 90 or 270 degree symbol turn
/// leaves about 1e-16 of cos/sin noise in a pin's world direction; without
/// a tolerance that noise decides which way a vertical pin's text reads.
const FOLD_EPS: f64 = 1e-9;

/// The text angle for a pin pointing `dir`, and whether the text was turned
/// half a circle to stay readable. Near-axis angles snap to the axis, and
/// both vertical directions land on `-pi/2`.
fn folded_angle(dir: (f64, f64)) -> (f64, bool) {
    let raw = dir.1.atan2(dir.0);
    let (mut rotation, folded) = if raw > FRAC_PI_2 - FOLD_EPS {
        (raw - PI, true)
    } else if raw < -FRAC_PI_2 - FOLD_EPS {
        (raw + PI, true)
    } else {
        (raw, false)
    };
    for axis in [-FRAC_PI_2, 0.0] {
        if (rotation - axis).abs() < FOLD_EPS {
            rotation = axis;
        }
    }
    (rotation, folded)
}

/// Pin numbers and names of `sym`'s visible pins, honouring the per-pin
/// `number_visible` / `name_visible` flags and the library's
/// `show_pin_numbers` / `show_pin_names`. A power port has none.
pub fn lower_pin_labels(lib: &LibSymbol, sym: &Symbol) -> Vec<PinLabel> {
    if sym.is_power {
        return Vec::new();
    }
    let t = SymbolTransform::from_symbol(sym);
    let mut out = Vec::new();
    for lp in instance_pins(lib, sym).filter(|lp| lp.pin.visible) {
        let pin = &lp.pin;
        let (dx, dy) = pin_direction(pin.rotation);
        let len = if pin.length > 0.0 {
            pin.length
        } else {
            PIN_LENGTH_MM
        };
        let tip = t.apply(pin.position);
        let body = t.apply(Point::new(
            pin.position.x + dx * len,
            pin.position.y + dy * len,
        ));
        // Unit direction tip -> body end, in world space.
        let (wx, wy) = (body.x - tip.x, body.y - tip.y);
        let wlen = wx.hypot(wy);
        let dir = if wlen > 0.0 {
            (wx / wlen, wy / wlen)
        } else {
            (1.0, 0.0)
        };
        let layout = pin_text_layout(tip, dir, wlen, lib.name_offset_mm());
        if lib.show_pin_names && pin.name_visible && !pin.name.is_empty() && pin.name != "~" {
            out.push(name_label(pin.name.clone(), &pin.number, &layout));
        }
        if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
            out.push(number_label(&pin.number, &layout));
        }
    }
    out
}

fn name_label(name: String, number: &str, layout: &PinTextLayout) -> PinLabel {
    PinLabel {
        kind: PinLabelKind::Name,
        text: name,
        pin_number: number.to_owned(),
        position: layout.name_pos,
        rotation_rad: layout.rotation_rad,
        h_align: layout.name_h_align,
        v_align: VAlign::Center,
    }
}

fn number_label(number: &str, layout: &PinTextLayout) -> PinLabel {
    PinLabel {
        kind: PinLabelKind::Number,
        text: number.to_owned(),
        pin_number: number.to_owned(),
        position: layout.number_pos,
        rotation_rad: layout.rotation_rad,
        h_align: HAlign::Center,
        v_align: VAlign::Bottom,
    }
}
