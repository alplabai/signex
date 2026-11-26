//! The selection info rows of a curve drawing (#1267): its kind, its
//! geometry with Y read from the page origin, its border and fill.

use signex_types::schematic::{CurveShape, FillType, SchCurve};

fn fill_label(fill: FillType) -> &'static str {
    match fill {
        FillType::None => "None",
        FillType::Outline => "Outline",
        FillType::Background => "Background",
    }
}

/// `(key, value)` rows of `c`; `page_y` turns a sheet Y into the page's.
pub(crate) fn rows(c: &SchCurve, page_y: impl Fn(f64) -> f64) -> Vec<(String, String)> {
    let at = |x: f64, y: f64| format!("{x:.2}, {:.2}", page_y(y));
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |key: &str, value: String| out.push((key.to_string(), value));
    match c.shape {
        CurveShape::Ellipse {
            center,
            radius_x,
            radius_y,
        } => {
            push("Type", "Ellipse".into());
            push("Center", at(center.x, center.y));
            push("Radius X", format!("{radius_x:.3}"));
            push("Radius Y", format!("{radius_y:.3}"));
        }
        CurveShape::EllipticalArc {
            center,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        } => {
            push("Type", "Elliptical Arc".into());
            push("Center", at(center.x, center.y));
            push("Radius X", format!("{radius_x:.3}"));
            push("Radius Y", format!("{radius_y:.3}"));
            push("Start Angle", format!("{start_deg:.3}"));
            push("End Angle", format!("{end_deg:.3}"));
        }
        CurveShape::RoundRect {
            from,
            to,
            corner_radius,
        } => {
            push("Type", "Round Rectangle".into());
            push("Position", at(from.x, from.y));
            push("Width", format!("{:.2}", to.x - from.x));
            push("Height", format!("{:.2}", to.y - from.y));
            push("Corner Radius", format!("{corner_radius:.3}"));
        }
        CurveShape::Pie {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            push("Type", "Pie".into());
            push("Center", at(center.x, center.y));
            push("Radius", format!("{radius:.3}"));
            push("Start Angle", format!("{start_deg:.3}"));
            push("End Angle", format!("{end_deg:.3}"));
        }
    }
    push("Border", format!("{:.3}", c.width));
    if c.shape.is_closed() {
        push("Fill", fill_label(c.fill).into());
    }
    out
}
