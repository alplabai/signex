//! The footprint thumbnail of a placed part's Footprint card (#42), as
//! transparent line art: theme-ink pad outlines with a low-alpha fill, a
//! dashed courtyard and a pin-1 dot. No board ground and no layer colours.

use iced::{Point, Rectangle};
use signex_library::primitive::{Footprint, Pad, PadKind, PadShape};
use signex_widgets::property_illustration::{
    Illustration, Ink, Paint, Part, Primitive, Style, Tint, Weight,
};

/// View box of the thumbnail.
const VIEW_W: f32 = 116.0;
const VIEW_H: f32 = 88.0;
/// Margin inside the view box, in view units.
const MARGIN: f32 = 10.0;
/// Courtyard drawn around the pads when the footprint stores none, mm.
const COURTYARD_GROW_MM: f64 = 0.25;
/// Height of the thumbnail, in caption sizes.
pub(in crate::panels) const THUMB_HEIGHT_EM: f32 = 8.0;
/// Width of the thumbnail, in caption sizes.
pub(in crate::panels) const THUMB_WIDTH_EM: f32 = THUMB_HEIGHT_EM * VIEW_W / VIEW_H;
/// Radius of the pin-1 dot, in view units.
const PIN_ONE_DOT: f32 = 1.8;

/// How a pad's outline is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ThumbPadShape {
    Rect,
    /// Corner radius as a share of the shorter side.
    Rounded(f64),
    /// A drilled hole with no copper.
    Hole,
}

/// One pad, axis-aligned, in footprint mm (y down).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ThumbPad {
    center: (f64, f64),
    size: (f64, f64),
    shape: ThumbPadShape,
}

/// What the thumbnail draws of a footprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FootprintThumb {
    pads: Vec<ThumbPad>,
    /// The courtyard outline, mm; empty when there are no pads.
    courtyard: Vec<(f64, f64)>,
    /// The pad that carries pin 1.
    pin_one: Option<usize>,
}

fn thumb_pad(pad: &Pad) -> ThumbPad {
    let quarter = (pad.rotation.rem_euclid(180.0) - 90.0).abs() < 45.0;
    let (w, h) = (pad.size[0].abs(), pad.size[1].abs());
    let shape = match pad.shape {
        _ if pad.kind == PadKind::NptHole => ThumbPadShape::Hole,
        PadShape::Round | PadShape::Oval => ThumbPadShape::Rounded(0.5),
        PadShape::RoundRect { radius_ratio } => {
            ThumbPadShape::Rounded(radius_ratio.clamp(0.0, 0.5))
        }
        _ => ThumbPadShape::Rect,
    };
    ThumbPad {
        center: (pad.position[0], pad.position[1]),
        size: if quarter { (h, w) } else { (w, h) },
        shape,
    }
}

impl FootprintThumb {
    pub fn of(footprint: &Footprint) -> Self {
        let pads: Vec<ThumbPad> = footprint
            .pads
            .iter()
            .filter(|p| p.position.iter().chain(&p.size).all(|v| v.is_finite()))
            .map(thumb_pad)
            .collect();
        let saved: Vec<(f64, f64)> = footprint
            .courtyard
            .points
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .map(|p| (p[0], p[1]))
            .collect();
        let courtyard = if saved.len() >= 3 {
            saved
        } else {
            pad_bounds(&pads)
                .map(|(x0, y0, x1, y1)| {
                    let g = COURTYARD_GROW_MM;
                    vec![
                        (x0 - g, y0 - g),
                        (x1 + g, y0 - g),
                        (x1 + g, y1 + g),
                        (x0 - g, y1 + g),
                    ]
                })
                .unwrap_or_default()
        };
        let pin_one = footprint
            .pads
            .iter()
            .position(|p| p.number == "1")
            .or((!pads.is_empty()).then_some(0))
            .filter(|&i| i < pads.len());
        Self {
            pads,
            courtyard,
            pin_one,
        }
    }

    /// The thumbnail drawing; `None` for a footprint with nothing to draw.
    pub fn illustration(&self) -> Option<Illustration> {
        let mut bounds = pad_bounds(&self.pads)?;
        for &(x, y) in &self.courtyard {
            bounds = grow(bounds, x, y);
        }
        let (x0, y0, x1, y1) = bounds;
        let (bw, bh) = ((x1 - x0).max(1e-3) as f32, (y1 - y0).max(1e-3) as f32);
        let scale = ((VIEW_W - 2.0 * MARGIN) / bw).min((VIEW_H - 2.0 * MARGIN) / bh);
        let (ox, oy) = (
            (VIEW_W - bw * scale) / 2.0 - x0 as f32 * scale,
            (VIEW_H - bh * scale) / 2.0 - y0 as f32 * scale,
        );
        let map = |x: f64, y: f64| Point::new(ox + x as f32 * scale, oy + y as f32 * scale);
        let mut ill = Illustration::new(VIEW_W, VIEW_H).height_em(THUMB_HEIGHT_EM);
        if self.courtyard.len() >= 3 {
            ill = ill.part(Part::new(
                Primitive::Polyline {
                    points: self.courtyard.iter().map(|&(x, y)| map(x, y)).collect(),
                    is_closed: true,
                },
                Style::dashed(),
            ));
        }
        for pad in &self.pads {
            ill = ill.part(pad_part(pad, &map, scale));
        }
        if let Some(pad) = self.pin_one.and_then(|i| self.pads.get(i)) {
            let (cx, cy) = pad.center;
            let corner = map(cx - pad.size.0 / 2.0, cy - pad.size.1 / 2.0);
            ill = ill.part(Part::new(
                Primitive::Circle {
                    center: Point::new(corner.x - 2.0 * PIN_ONE_DOT, corner.y - 2.0 * PIN_ONE_DOT),
                    radius: PIN_ONE_DOT,
                },
                Style::filled(Paint::Ink(Ink::Secondary), Tint::Solid),
            ));
        }
        Some(ill)
    }
}

fn pad_part(pad: &ThumbPad, map: &impl Fn(f64, f64) -> Point, scale: f32) -> Part {
    let (cx, cy) = pad.center;
    let (w, h) = pad.size;
    let top_left = map(cx - w / 2.0, cy - h / 2.0);
    let rect = Rectangle {
        x: top_left.x,
        y: top_left.y,
        width: w as f32 * scale,
        height: h as f32 * scale,
    };
    let outline = Style::outline(Ink::Secondary, Weight::Thin);
    let filled = outline.with_fill(Paint::Ink(Ink::Secondary), Tint::Light);
    let primitive = match pad.shape {
        ThumbPadShape::Rect => Primitive::Rect { rect },
        ThumbPadShape::Rounded(ratio) => Primitive::RoundedRect {
            rect,
            radius: rect.width.min(rect.height) * ratio as f32,
        },
        ThumbPadShape::Hole => {
            return Part::new(
                Primitive::Circle {
                    center: map(cx, cy),
                    radius: rect.width.min(rect.height) / 2.0,
                },
                outline,
            );
        }
    };
    Part::new(primitive, filled)
}

fn grow(b: (f64, f64, f64, f64), x: f64, y: f64) -> (f64, f64, f64, f64) {
    (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y))
}

/// The box around every pad, mm.
fn pad_bounds(pads: &[ThumbPad]) -> Option<(f64, f64, f64, f64)> {
    pads.iter().fold(None, |acc, pad| {
        let (cx, cy) = pad.center;
        let (hw, hh) = (pad.size.0 / 2.0, pad.size.1 / 2.0);
        let b = acc.unwrap_or((cx - hw, cy - hh, cx + hw, cy + hh));
        Some(grow(grow(b, cx - hw, cy - hh), cx + hw, cy + hh))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_pads() -> Footprint {
        let mut fp = Footprint::empty("R_0402");
        fp.pads = [(-0.5, 0.0), (0.5, 0.0)]
            .iter()
            .enumerate()
            .map(|(i, (x, y))| Pad {
                number: (i + 1).to_string(),
                kind: PadKind::Smd,
                shape: PadShape::Rect,
                position: [*x, *y],
                size: [0.6, 0.5],
                ..Pad::default()
            })
            .collect();
        fp
    }

    #[test]
    fn the_thumbnail_is_line_art_with_a_dashed_courtyard_and_a_pin_one_dot() {
        let thumb = FootprintThumb::of(&two_pads());
        assert_eq!(thumb.pin_one, Some(0));
        assert_eq!(thumb.courtyard.len(), 4, "a courtyard around the pads");
        let ill = thumb.illustration().expect("a drawing");
        let parts: Vec<_> = ill.visible_parts().collect();
        // Courtyard, two pads, pin-1 dot.
        assert_eq!(parts.len(), 4);
        assert!(parts[0].style.stroke.is_some_and(|s| s.is_dashed));
        for pad in &parts[1..3] {
            let fill = pad.style.fill.expect("a pad fill");
            assert_eq!(
                fill.paint,
                Paint::Ink(Ink::Secondary),
                "theme ink, no layer colour"
            );
            assert!(fill.tint.alpha() < 0.5, "a low-alpha fill");
        }
        assert!(matches!(parts[3].primitive, Primitive::Circle { .. }));
    }

    #[test]
    fn a_footprint_without_pads_has_no_thumbnail() {
        assert!(
            FootprintThumb::of(&Footprint::empty("x"))
                .illustration()
                .is_none()
        );
    }

    #[test]
    fn a_quarter_turned_pad_swaps_its_sides() {
        let mut fp = two_pads();
        fp.pads[0].rotation = 90.0;
        let thumb = FootprintThumb::of(&fp);
        assert_eq!(thumb.pads[0].size, (0.5, 0.6));
        assert_eq!(thumb.pads[1].size, (0.6, 0.5));
    }
}
