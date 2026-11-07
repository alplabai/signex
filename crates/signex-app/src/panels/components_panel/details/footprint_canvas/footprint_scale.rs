//! Size readout and scale bar drawn over the footprint preview. The
//! pure helpers (round length, labels) carry the logic; `draw_readouts`
//! only paints them.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle};

/// Share of the pane width the scale bar aims for.
const BAR_TARGET_FRACTION: f64 = 0.20;

/// Readout text colour: light grey, legible on the editor's fixed backdrop.
const READOUT_COLOR: Color = Color::from_rgb(0.80, 0.82, 0.85);

/// The 1/2/5 x 10^n mm length closest (in ratio) to `target_mm`; `None`
/// for a non-finite or non-positive target.
pub(super) fn round_length_mm(target_mm: f64) -> Option<f64> {
    if !target_mm.is_finite() || target_mm <= 0.0 {
        return None;
    }
    let decade = target_mm.log10().floor() as i32;
    let mut best: Option<(f64, f64)> = None;
    for d in [decade - 1, decade, decade + 1] {
        for m in [1.0, 2.0, 5.0] {
            let len = m * 10f64.powi(d);
            let miss = (len / target_mm).ln().abs();
            if best.is_none_or(|(_, b)| miss < b) {
                best = Some((len, miss));
            }
        }
    }
    best.map(|(len, _)| len)
}

/// Scale-bar length for a pane `width_px` wide at `scale` px per mm:
/// `(length_mm, length_px)`.
pub(super) fn scale_bar(scale: f32, width_px: f32) -> Option<(f64, f32)> {
    if !(scale.is_finite() && scale > 0.0 && width_px.is_finite() && width_px > 0.0) {
        return None;
    }
    let target_mm = f64::from(width_px) * BAR_TARGET_FRACTION / f64::from(scale);
    let len = round_length_mm(target_mm)?;
    Some((len, (len * f64::from(scale)) as f32))
}

/// A millimetre value with at most 3 decimals and no trailing zeros.
fn trim_mm(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `"W x H mm"` for a bbox `(min_x, min_y, max_x, max_y)`.
pub(super) fn size_label(bbox: (f64, f64, f64, f64)) -> String {
    format!(
        "{} x {} mm",
        trim_mm(bbox.2 - bbox.0),
        trim_mm(bbox.3 - bbox.1)
    )
}

/// `"0.5 mm"` for a scale-bar length.
pub(super) fn length_label(mm: f64) -> String {
    format!("{} mm", trim_mm(mm))
}

/// Paint the size readout (top-left) and the scale bar (bottom-left).
pub(super) fn draw_readouts(
    frame: &mut canvas::Frame,
    area: Rectangle,
    scale: f32,
    bbox: Option<(f64, f64, f64, f64)>,
) {
    let text_size = crate::styles::TypeScale::DEFAULT.caption;
    let margin = text_size;
    let text = |content: String, position: Point, align_y| canvas::Text {
        content,
        position,
        size: text_size.into(),
        color: READOUT_COLOR,
        align_y,
        ..canvas::Text::default()
    };
    if let Some(bbox) = bbox {
        frame.fill_text(text(
            size_label(bbox),
            Point::new(margin, margin),
            iced::alignment::Vertical::Top,
        ));
    }
    let Some((mm, px)) = scale_bar(scale, area.width) else {
        return;
    };
    let tick = text_size * 0.5;
    let y = area.height - margin - text_size * 1.5;
    let (x0, x1) = (margin, margin + px);
    let stroke = || Stroke::default().with_width(1.0).with_color(READOUT_COLOR);
    let bar = Path::new(|b| {
        b.move_to(Point::new(x0, y - tick));
        b.line_to(Point::new(x0, y));
        b.line_to(Point::new(x1, y));
        b.line_to(Point::new(x1, y - tick));
    });
    frame.stroke(&bar, stroke());
    frame.fill_text(text(
        length_label(mm),
        Point::new(x0, y + text_size * 0.25),
        iced::alignment::Vertical::Top,
    ));
}
