//! Stroke / fill drawing of one footprint graphic (silk, fab, body line).

use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::{Color, Point, Radians, Size};
use signex_library::primitive::{FpGraphic, FpGraphicKind};

use super::layout::Fit;
use super::palette::fill_of;
use crate::styles::HAIRLINE;

pub(super) fn draw_graphic(frame: &mut Frame, g: &FpGraphic, color: Color, fit: &Fit) {
    let width = if g.stroke_width > 0.0 {
        fit.px(g.stroke_width).max(HAIRLINE)
    } else {
        HAIRLINE
    };
    let stroke = || Stroke::default().with_color(color).with_width(width);
    let to_px = |p: [f64; 2]| fit.to_px(p);
    // Closed shapes flagged `filled` get a faint body fill under the line.
    let outline = |frame: &mut Frame, path: &Path| {
        if g.filled {
            frame.fill(path, fill_of(color));
        }
        frame.stroke(path, stroke());
    };
    match &g.kind {
        FpGraphicKind::Line { from, to } => {
            frame.stroke(&Path::line(to_px(*from), to_px(*to)), stroke());
        }
        FpGraphicKind::Rectangle { from, to } => {
            let (a, b) = (to_px(*from), to_px(*to));
            let path = Path::rectangle(
                Point::new(a.x.min(b.x), a.y.min(b.y)),
                Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
            );
            outline(frame, &path);
        }
        FpGraphicKind::Circle { center, radius } => {
            outline(frame, &Path::circle(to_px(*center), fit.px(*radius)));
        }
        FpGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let sweep = FpGraphicKind::arc_sweep_deg(*start_deg, *end_deg);
            let path = Path::new(|b| {
                b.arc(canvas::path::Arc {
                    center: to_px(*center),
                    radius: fit.px(*radius),
                    start_angle: Radians(start_deg.to_radians() as f32),
                    end_angle: Radians((start_deg + sweep).to_radians() as f32),
                });
            });
            frame.stroke(&path, stroke());
        }
        FpGraphicKind::Polygon { vertices } if vertices.len() > 1 => {
            outline(frame, &closed_path(vertices, fit));
        }
        FpGraphicKind::Polygon { .. } | FpGraphicKind::Text { .. } => {}
    }
}

/// A closed polyline through `points`.
pub(super) fn closed_path(points: &[[f64; 2]], fit: &Fit) -> Path {
    Path::new(|b| {
        let Some((first, rest)) = points.split_first() else {
            return;
        };
        b.move_to(fit.to_px(*first));
        for p in rest {
            b.line_to(fit.to_px(*p));
        }
        b.close();
    })
}
