//! The drawing behind the Import DXF card (mockup row 179c): the pieces of
//! the current choices fitted left of the card, the file name and the layer
//! mapping under them. While checking, each gap and overlap is ringed and
//! named; once imported, what landed draws in the fully constrained green.

use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_sketch::import::{ImportPiece, ImportShape};

use super::{SP, TS, target_label};
use crate::app::Message;
use crate::app::state::{SketchDxfDialog, SketchDxfStep};
use crate::library::editor::footprint::SketchInk;
use crate::library::editor::footprint::format_check_mm as mm;

/// A ringed place on the drawing and what it names.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Mark {
    pub at: (f64, f64),
    pub label: String,
}

/// The bounds of the drawing, in sketch mm: `(min x, min y, max x, max y)`.
pub(super) type Extent = (f64, f64, f64, f64);

/// Where the drawing lands on the canvas: a sketch point `p` draws at
/// `origin + (p - (min x, min y)) * scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fit {
    pub scale: f32,
    pub origin: Point,
    pub min: (f64, f64),
}

impl Fit {
    fn at(&self, (x, y): (f64, f64)) -> Point {
        Point::new(
            self.origin.x + (x - self.min.0) as f32 * self.scale,
            self.origin.y + (y - self.min.1) as f32 * self.scale,
        )
    }
}

/// The bounds of `pieces`; `None` for nothing.
pub(super) fn extent(pieces: &[ImportPiece]) -> Option<Extent> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for piece in pieces {
        match piece.shape {
            ImportShape::Line { a, b } => points.extend([a, b]),
            ImportShape::Arc { center, start, .. } => {
                let r = (start.0 - center.0).hypot(start.1 - center.1);
                points.extend([(center.0 - r, center.1 - r), (center.0 + r, center.1 + r)]);
            }
            ImportShape::Circle { center, radius } => points.extend([
                (center.0 - radius, center.1 - radius),
                (center.0 + radius, center.1 + radius),
            ]),
        }
    }
    let first = *points.first()?;
    Some(points.iter().fold(
        (first.0, first.1, first.0, first.1),
        |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
    ))
}

/// Fit `extent` into `area` at `top_left`, keeping its proportions.
pub(super) fn fit(extent: Extent, top_left: Point, area: Size) -> Fit {
    let width = (extent.2 - extent.0).max(f64::EPSILON) as f32;
    let height = (extent.3 - extent.1).max(f64::EPSILON) as f32;
    let scale = (area.width / width).min(area.height / height).max(0.0);
    Fit {
        scale,
        origin: top_left,
        min: (extent.0, extent.1),
    }
}

/// The marks of the check: each gap and each overlap.
pub(super) fn marks(dialog: &SketchDxfDialog, i18n: &I18n) -> Vec<Mark> {
    let report = &dialog.report;
    let gaps = report.gaps.iter().map(|gap| Mark {
        at: gap.at,
        label: t!(i18n, ids::BOARD_SKETCH_DXF_MARK_GAP, "gap" => mm(gap.gap_mm, 2)).into_string(),
    });
    let overlaps = report.overlaps.iter().map(|overlap| Mark {
        at: overlap.at,
        label: t!(i18n, ids::BOARD_SKETCH_DXF_MARK_OVERLAP, "count" => overlap.segments)
            .into_string(),
    });
    gaps.chain(overlaps).collect()
}

/// The layer mapping: each layer and its target, a skipped layer ignored.
pub(super) fn layer_caption(dialog: &SketchDxfDialog, i18n: &I18n) -> String {
    let entries: Vec<String> = dialog
        .rows
        .iter()
        .zip(&dialog.targets)
        .map(|(row, target)| match target {
            None => t!(i18n, ids::BOARD_SKETCH_DXF_LAYER_SKIPPED, "layer" => row.name.clone())
                .into_string(),
            Some(_) => t!(i18n, ids::BOARD_SKETCH_DXF_LAYER_TARGET,
                "layer" => row.name.clone(),
                "target" => target_label(*target, i18n))
            .into_string(),
        })
        .collect();
    t!(i18n, ids::BOARD_SKETCH_DXF_LAYERS, "list" => entries.join(" · ")).into_string()
}

/// The canvas behind the card.
pub(super) fn preview<'a>(
    dialog: &'a SketchDxfDialog,
    card_room: f32,
    i18n: &I18n,
) -> Element<'a, Message> {
    let ink = SketchInk::board();
    let imported = matches!(dialog.step, SketchDxfStep::Imported(_));
    let marks = if dialog.step == SketchDxfStep::Checking {
        marks(dialog, i18n)
    } else {
        Vec::new()
    };
    let drawing = DxfPreview {
        pieces: &dialog.pieces,
        marks,
        captions: [dialog.file.clone(), layer_caption(dialog, i18n)],
        card_room,
        line: if imported { ink.full } else { ink.under },
        problem: ink.over,
        caption: ink.label,
    };
    let canvas: canvas::Canvas<DxfPreview<'a>, Message> = canvas::Canvas::new(drawing);
    canvas.width(Length::Fill).height(Length::Fill).boxed()
}

struct DxfPreview<'a> {
    pieces: &'a [ImportPiece],
    marks: Vec<Mark>,
    /// The file name, then the layer mapping.
    captions: [String; 2],
    /// The width the card takes at the right.
    card_room: f32,
    line: Color,
    problem: Color,
    caption: Color,
}

impl DxfPreview<'_> {
    /// The fit of the drawing left of the card, above the captions.
    fn fit(&self, bounds: Size) -> Option<Fit> {
        let extent = extent(self.pieces)?;
        let captions = self.captions.len() as f32 * TS.caption * 1.5;
        let ring = TS.body * 2.0;
        let width = (bounds.width - self.card_room).max(bounds.width / 2.0);
        let area = Size::new(
            (width - 2.0 * SP.xl - ring).max(0.0),
            (bounds.height - 2.0 * SP.xl - captions - SP.l - ring).max(0.0),
        );
        let top_left = Point::new(SP.xl + ring / 2.0, SP.xl + ring / 2.0);
        Some(fit(extent, top_left, area))
    }

    fn stroke(&self, color: Color) -> canvas::Stroke<'static> {
        canvas::Stroke::default()
            .with_color(color)
            .with_width(TS.body / 8.0)
    }

    fn label(&self, frame: &mut canvas::Frame, content: String, at: Point, color: Color) {
        frame.fill_text(canvas::Text {
            content,
            position: at,
            color,
            size: iced::Pixels(TS.caption),
            font: iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            },
            ..canvas::Text::default()
        });
    }

    fn draw_piece(&self, frame: &mut canvas::Frame, fit: &Fit, piece: &ImportPiece) {
        let path = match piece.shape {
            ImportShape::Line { a, b } => canvas::Path::line(fit.at(a), fit.at(b)),
            ImportShape::Circle { center, radius } => {
                canvas::Path::circle(fit.at(center), radius as f32 * fit.scale)
            }
            ImportShape::Arc { center, start, end } => {
                let angle = |p: (f64, f64)| (p.1 - center.1).atan2(p.0 - center.0) as f32;
                let from = angle(start);
                let mut to = angle(end);
                if to <= from {
                    to += std::f32::consts::TAU;
                }
                let radius = (start.0 - center.0).hypot(start.1 - center.1) as f32 * fit.scale;
                canvas::Path::new(|b| {
                    b.arc(canvas::path::Arc {
                        center: fit.at(center),
                        radius,
                        start_angle: iced::Radians(from),
                        end_angle: iced::Radians(to),
                    });
                })
            }
        };
        frame.stroke(&path, self.stroke(self.line));
    }
}

impl<M> canvas::Program<M> for DxfPreview<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let Some(fit) = self.fit(bounds.size()) else {
            return vec![frame.into_geometry()];
        };
        for piece in self.pieces {
            self.draw_piece(&mut frame, &fit, piece);
        }
        let ring = TS.body;
        for mark in &self.marks {
            let at = fit.at(mark.at);
            frame.stroke(&canvas::Path::circle(at, ring), self.stroke(self.problem));
            let text_at = Point::new(at.x + ring, at.y - ring - TS.caption);
            self.label(&mut frame, mark.label.clone(), text_at, self.problem);
        }
        let bottom = extent(self.pieces).map_or(fit.origin.y, |e| fit.at((e.0, e.3)).y);
        for (i, caption) in self.captions.iter().enumerate() {
            let y = bottom + SP.l + i as f32 * TS.caption * 1.5;
            let at = Point::new(fit.origin.x, y);
            self.label(&mut frame, caption.clone(), at, self.caption);
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> ImportPiece {
        ImportPiece {
            shape: ImportShape::Line { a, b },
            tag: 0,
        }
    }

    #[test]
    fn the_extent_covers_lines_and_circles() {
        let circle = ImportPiece {
            shape: ImportShape::Circle {
                center: (10.0, 0.0),
                radius: 2.0,
            },
            tag: 1,
        };
        let e = extent(&[line((0.0, 0.0), (5.0, 3.0)), circle]).expect("something");
        assert_eq!(e, (0.0, -2.0, 12.0, 3.0));
        assert_eq!(extent(&[]), None);
    }

    #[test]
    fn the_fit_keeps_the_drawing_inside_its_area_and_proportional() {
        let area = Size::new(300.0, 100.0);
        let top_left = Point::new(10.0, 20.0);
        let f = fit((0.0, 0.0, 60.0, 40.0), top_left, area);
        assert!(
            (f.scale - 2.5).abs() < 1e-6,
            "the height limits: {}",
            f.scale
        );
        assert_eq!(f.at((0.0, 0.0)), top_left);
        let far = f.at((60.0, 40.0));
        assert!(far.x <= top_left.x + area.width && far.y <= top_left.y + area.height);
    }
}
