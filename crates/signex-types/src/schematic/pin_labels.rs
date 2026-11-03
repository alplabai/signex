//! Where a placed symbol's pin numbers and pin names sit, in world mm.
//!
//! The canvas and the SVG/PDF export both place pin text from this list, so
//! the two cannot disagree on side, alignment or visibility.

use super::{
    HAlign, LibSymbol, PIN_LENGTH_MM, Point, Symbol, SymbolTransform, VAlign, instance_pins,
    pin_direction,
};

#[cfg(test)]
#[path = "pin_labels_tests.rs"]
mod tests;

/// Gap between a pin line and its number, mm.
const PIN_NUMBER_GAP_MM: f64 = 0.4;

/// Which text of a pin a [`PinLabel`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinLabelKind {
    /// Outside the body, beside the pin line.
    Number,
    /// Inside the body, past the pin's body end.
    Name,
}

/// One pin text, ready to draw horizontally at `position`.
#[derive(Debug, Clone, PartialEq)]
pub struct PinLabel {
    pub kind: PinLabelKind,
    /// The text as stored (before any expression evaluation).
    pub text: String,
    /// Number of the pin the text belongs to.
    pub pin_number: String,
    pub position: Point,
    pub h_align: HAlign,
    pub v_align: VAlign,
}

/// Pin numbers and names of `sym`'s visible pins, honouring the per-pin
/// `number_visible` / `name_visible` flags and the library's
/// `show_pin_numbers` / `show_pin_names`.
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
        // Unit direction tip -> body end in world space.
        let (wx, wy) = (body.x - tip.x, body.y - tip.y);
        let wlen = wx.hypot(wy);
        let (wdx, wdy) = if wlen > 0.0 {
            (wx / wlen, wy / wlen)
        } else {
            (1.0, 0.0)
        };

        if lib.show_pin_names && pin.name_visible && !pin.name.is_empty() && pin.name != "~" {
            let offset = lib.name_offset_mm();
            let (h_align, v_align) = if wdx.abs() > wdy.abs() {
                (
                    if wdx > 0.0 {
                        HAlign::Left
                    } else {
                        HAlign::Right
                    },
                    VAlign::Center,
                )
            } else {
                (
                    HAlign::Center,
                    if wdy > 0.0 {
                        VAlign::Top
                    } else {
                        VAlign::Bottom
                    },
                )
            };
            out.push(PinLabel {
                kind: PinLabelKind::Name,
                text: pin.name.clone(),
                pin_number: pin.number.clone(),
                position: Point::new(body.x + wdx * offset, body.y + wdy * offset),
                h_align,
                v_align,
            });
        }

        if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
            let mid = Point::new((tip.x + body.x) * 0.5, (tip.y + body.y) * 0.5);
            // Above a horizontal pin line, left of a vertical one.
            let (position, h_align, v_align) = if wdx.abs() >= wdy.abs() {
                (
                    Point::new(mid.x, mid.y - PIN_NUMBER_GAP_MM),
                    HAlign::Center,
                    VAlign::Bottom,
                )
            } else {
                (
                    Point::new(mid.x - PIN_NUMBER_GAP_MM, mid.y),
                    HAlign::Right,
                    VAlign::Center,
                )
            };
            out.push(PinLabel {
                kind: PinLabelKind::Number,
                text: pin.number.clone(),
                pin_number: pin.number.clone(),
                position,
                h_align,
                v_align,
            });
        }
    }
    out
}
