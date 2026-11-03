//! Pin number and pin name text of a placed symbol, lowered to world space.
//!
//! Separate from [`super::lower_symbol`] so the hit-test / culling box stays
//! the body's: labels never grow what a click selects.

use super::{
    HAlign, LibSymbol, PIN_LENGTH_MM, Point, SCHEMATIC_TEXT_MM, Symbol, SymbolTransform, VAlign,
    instance_pins, pin_direction,
};

/// Which of a pin's two texts a [`PinLabel`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLabelKind {
    Number,
    Name,
}

/// One pin text in world space, shared by the canvas and the SVG/PDF export.
#[derive(Debug, Clone, PartialEq)]
pub struct PinLabel {
    pub kind: PinLabelKind,
    /// The pin's number, for `=`-markup evaluation on the export side.
    pub pin_number: String,
    pub text: String,
    pub position: Point,
    /// Degrees, counter-clockwise on screen; 90 on vertical pins.
    pub rotation_deg: f64,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub size: f64,
}

/// Gap between a pin line and its number, mm.
const NUMBER_GAP_MM: f64 = 0.3;

/// Number and name text of `sym`'s visible pins, honouring the symbol's
/// `show_pin_numbers` / `show_pin_names` and each pin's own flags. The number
/// sits beside the pin line outside the body; the name sits inside the body,
/// `name_offset_mm` past the pin's body end.
pub fn lower_pin_labels(lib: &LibSymbol, sym: &Symbol) -> Vec<PinLabel> {
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
        let (wdx, wdy) = (body.x - tip.x, body.y - tip.y);
        let norm = wdx.hypot(wdy);
        if norm == 0.0 {
            continue;
        }
        let (ux, uy) = (wdx / norm, wdy / norm);
        let horizontal = ux.abs() >= uy.abs();

        if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
            let mid = Point::new((tip.x + body.x) * 0.5, (tip.y + body.y) * 0.5);
            let (position, rotation_deg, h_align, v_align) = if horizontal {
                (
                    Point::new(mid.x, mid.y - NUMBER_GAP_MM),
                    0.0,
                    HAlign::Center,
                    VAlign::Bottom,
                )
            } else {
                // Rotated so the text's top edge faces left of the line.
                (
                    Point::new(mid.x - NUMBER_GAP_MM, mid.y),
                    90.0,
                    HAlign::Center,
                    VAlign::Bottom,
                )
            };
            out.push(label(
                PinLabelKind::Number,
                pin,
                position,
                rotation_deg,
                h_align,
                v_align,
            ));
        }

        if lib.show_pin_names && pin.name_visible && !pin.name.is_empty() && pin.name != "~" {
            let offset = lib.name_offset_mm();
            let position = Point::new(body.x + ux * offset, body.y + uy * offset);
            // Vertical names read upward (90 deg): the anchor is the text's
            // start when the body lies above the pin, its end when below.
            let (rotation_deg, h_align, v_align) = if horizontal {
                (
                    0.0,
                    if ux > 0.0 {
                        HAlign::Left
                    } else {
                        HAlign::Right
                    },
                    VAlign::Center,
                )
            } else {
                (
                    90.0,
                    if uy < 0.0 {
                        HAlign::Left
                    } else {
                        HAlign::Right
                    },
                    VAlign::Center,
                )
            };
            out.push(label(
                PinLabelKind::Name,
                pin,
                position,
                rotation_deg,
                h_align,
                v_align,
            ));
        }
    }
    out
}

fn label(
    kind: PinLabelKind,
    pin: &super::Pin,
    position: Point,
    rotation_deg: f64,
    h_align: HAlign,
    v_align: VAlign,
) -> PinLabel {
    PinLabel {
        kind,
        pin_number: pin.number.clone(),
        text: match kind {
            PinLabelKind::Number => pin.number.clone(),
            PinLabelKind::Name => pin.name.clone(),
        },
        position,
        rotation_deg,
        h_align,
        v_align,
        size: SCHEMATIC_TEXT_MM,
    }
}

#[cfg(test)]
#[path = "pin_labels_tests.rs"]
mod tests;
