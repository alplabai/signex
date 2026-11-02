//! Small graphical footprint preview: pads, courtyard, silkscreen and fab
//! outlines of a `.snxfpt`, fitted to the box. View-only.

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Radians, Rectangle, Renderer, Size, Theme, Vector, mouse};
use signex_library::primitive::{Footprint, FpGraphic, FpGraphicKind, PadShape};

/// Share of the box the drawing fills, so pads keep a margin.
const FILL: f64 = 0.88;
/// Pads thinner than this many pixels get no number.
const MIN_LABELLED_PAD: f32 = 9.0;
/// Stroke width of outlines without their own width.
const HAIRLINE: f32 = 1.0;

pub struct FootprintPreview {
    footprint: Footprint,
    pad: Color,
    silk: Color,
    outline: Color,
    text: Color,
}

impl FootprintPreview {
    pub fn new(footprint: Footprint, pad: Color, silk: Color, outline: Color, text: Color) -> Self {
        Self {
            footprint,
            pad,
            silk,
            outline,
            text,
        }
    }

    /// Footprint-local bounding box `(min, max)` in mm of everything drawn.
    fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let fp = &self.footprint;
        let mut points: Vec<[f64; 2]> = Vec::new();
        for pad in &fp.pads {
            let r = pad.size[0].max(pad.size[1]) / 2.0;
            points.push([pad.position[0] - r, pad.position[1] - r]);
            points.push([pad.position[0] + r, pad.position[1] + r]);
        }
        points.extend(fp.courtyard.points.iter().copied());
        for g in fp.silk_f.iter().chain(&fp.fab_f) {
            graphic_points(g, &mut points);
        }
        let first = *points.first()?;
        Some(points.iter().fold((first, first), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            )
        }))
    }
}

fn graphic_points(g: &FpGraphic, out: &mut Vec<[f64; 2]>) {
    match &g.kind {
        FpGraphicKind::Line { from, to } | FpGraphicKind::Rectangle { from, to } => {
            out.extend([*from, *to]);
        }
        FpGraphicKind::Circle { center, radius } | FpGraphicKind::Arc { center, radius, .. } => {
            out.push([center[0] - radius, center[1] - radius]);
            out.push([center[0] + radius, center[1] + radius]);
        }
        FpGraphicKind::Polygon { vertices } => out.extend(vertices.iter().copied()),
        FpGraphicKind::Text { position, .. } => out.push(*position),
    }
}

impl<M> canvas::Program<M> for FootprintPreview {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let Some((lo, hi)) = self.bounds() else {
            return vec![frame.into_geometry()];
        };
        let span = [(hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6)];
        let scale =
            (f64::from(bounds.width) / span[0]).min(f64::from(bounds.height) / span[1]) * FILL;
        let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let centre = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        // Footprint space is y-down, like the canvas.
        let to_px = |p: [f64; 2]| {
            Point::new(
                centre.x + ((p[0] - mid[0]) * scale) as f32,
                centre.y + ((p[1] - mid[1]) * scale) as f32,
            )
        };
        let px = |mm: f64| (mm * scale) as f32;

        let courtyard = &self.footprint.courtyard.points;
        if courtyard.len() > 1 {
            let path = Path::new(|b| {
                b.move_to(to_px(courtyard[0]));
                for p in &courtyard[1..] {
                    b.line_to(to_px(*p));
                }
                b.close();
            });
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(self.outline)
                    .with_width(HAIRLINE),
            );
        }
        for (group, color) in [
            (&self.footprint.fab_f, self.outline),
            (&self.footprint.silk_f, self.silk),
        ] {
            for g in group {
                draw_graphic(&mut frame, g, color, &to_px, &px);
            }
        }
        for pad in &self.footprint.pads {
            let size = Size::new(px(pad.size[0]), px(pad.size[1]));
            let at = to_px(pad.position);
            let colour = self.pad;
            frame.with_save(|f| {
                f.translate(Vector::new(at.x, at.y));
                f.rotate(Radians(pad.rotation.to_radians() as f32));
                let top_left = Point::new(-size.width / 2.0, -size.height / 2.0);
                let short = size.width.min(size.height);
                let path = match &pad.shape {
                    PadShape::Round => Path::circle(Point::ORIGIN, short / 2.0),
                    PadShape::Oval => Path::rounded_rectangle(top_left, size, (short / 2.0).into()),
                    PadShape::RoundRect { radius_ratio } => Path::rounded_rectangle(
                        top_left,
                        size,
                        (short * *radius_ratio as f32).into(),
                    ),
                    _ => Path::rectangle(top_left, size),
                };
                f.fill(&path, colour);
            });
            if size.width.min(size.height) >= MIN_LABELLED_PAD {
                frame.fill_text(canvas::Text {
                    content: pad.number.clone(),
                    position: at,
                    color: self.text,
                    size: (size.height.min(size.width) * 0.6).into(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    ..canvas::Text::default()
                });
            }
        }
        vec![frame.into_geometry()]
    }
}

fn draw_graphic(
    frame: &mut Frame,
    g: &FpGraphic,
    color: Color,
    to_px: &dyn Fn([f64; 2]) -> Point,
    px: &dyn Fn(f64) -> f32,
) {
    let width = if g.stroke_width > 0.0 {
        px(g.stroke_width).max(HAIRLINE)
    } else {
        HAIRLINE
    };
    let stroke = || Stroke::default().with_color(color).with_width(width);
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
            frame.stroke(&path, stroke());
        }
        FpGraphicKind::Circle { center, radius } => {
            frame.stroke(&Path::circle(to_px(*center), px(*radius)), stroke());
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
                    radius: px(*radius),
                    start_angle: Radians(start_deg.to_radians() as f32),
                    end_angle: Radians((start_deg + sweep).to_radians() as f32),
                });
            });
            frame.stroke(&path, stroke());
        }
        FpGraphicKind::Polygon { vertices } if vertices.len() > 1 => {
            let path = Path::new(|b| {
                b.move_to(to_px(vertices[0]));
                for v in &vertices[1..] {
                    b.line_to(to_px(*v));
                }
                b.close();
            });
            frame.stroke(&path, stroke());
        }
        FpGraphicKind::Polygon { .. } | FpGraphicKind::Text { .. } => {}
    }
}
