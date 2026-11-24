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

/// Readout text size, px; the margin and the rows derive from it.
const TEXT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.caption;
/// Share of the pane the scale bar can take (it aims for a fifth and rounds
/// to a 1/2/5 length).
const BAR_MAX_FRACTION: f32 = 0.35;
/// A readout character's width, in text sizes, for the row-fit estimate.
const CHAR_WIDTH_EM: f32 = 0.6;
/// Height of the scale bar and its label above the bottom edge, in text sizes.
const BAR_ROWS_EM: f32 = 3.0;
/// Height of the size label's own line when it stacks over the bar.
const STACKED_ROW_EM: f32 = 1.2;

fn label_width(label: &str) -> f32 {
    label.chars().count() as f32 * TEXT_SIZE * CHAR_WIDTH_EM
}

/// The size label does not fit beside the widest scale bar in `width`.
fn size_stacks_over_bar(width: f32, label: &str) -> bool {
    let bar = (width * BAR_MAX_FRACTION).max(TEXT_SIZE * 4.0);
    TEXT_SIZE * 3.0 + bar + label_width(label) > width
}

/// Height of the band the readouts take at the bottom of a `width` wide pane.
fn readout_band(width: f32, label: Option<&str>) -> f32 {
    let stacked = label.is_some_and(|l| size_stacks_over_bar(width, l));
    TEXT_SIZE * (BAR_ROWS_EM + if stacked { STACKED_ROW_EM } else { 0.0 })
}

/// The part of `viewport` a fit may fill: above the readout band. The fit
/// keeps its own pad from the bottom edge, so the band only adds the rest. A
/// box too short to spare it keeps at least half its height.
pub(super) fn drawing_area(
    viewport: Rectangle,
    bbox: (f64, f64, f64, f64),
    fit_pad: f32,
) -> Rectangle {
    let band = readout_band(viewport.width, Some(&size_label(bbox)));
    Rectangle {
        height: (viewport.height - (band - fit_pad).max(0.0)).max(viewport.height / 2.0),
        ..viewport
    }
}

/// Where the size label sits: bottom-right on the scale label's row, or on
/// its own line above the bar when the two would touch.
pub(super) fn size_label_bounds(area: Rectangle, label: &str) -> Rectangle {
    let width = label_width(label);
    let row_top = area.height - TEXT_SIZE * 2.25;
    let top = if size_stacks_over_bar(area.width, label) {
        area.height - TEXT_SIZE * (BAR_ROWS_EM + STACKED_ROW_EM)
    } else {
        row_top
    };
    Rectangle {
        x: area.width - TEXT_SIZE - width,
        y: top,
        width,
        height: TEXT_SIZE * STACKED_ROW_EM,
    }
}

/// Paint the size readout (bottom-right) and the scale bar (bottom-left).
pub(super) fn draw_readouts(
    frame: &mut canvas::Frame,
    area: Rectangle,
    scale: f32,
    bbox: Option<(f64, f64, f64, f64)>,
) {
    let text_size = TEXT_SIZE;
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
        let label = size_label(bbox);
        let top = size_label_bounds(area, &label).y;
        frame.fill_text(canvas::Text {
            align_x: iced::alignment::Horizontal::Right.into(),
            ..text(
                label,
                Point::new(area.width - margin, top),
                iced::alignment::Vertical::Top,
            )
        });
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
