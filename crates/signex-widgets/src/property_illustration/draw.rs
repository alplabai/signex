//! Paints parts onto a canvas frame.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::{self, Frame, LineCap, LineDash, LineJoin, Path};
use iced::{Color, Font, Pixels, Point, font};

use super::geometry::{Baseline, Fit, Head, LabelPlacement, callout_geometry, leader_label};
use super::illustration::Part;
use super::metrics::Metrics;
use super::palette::Palette;
use super::primitive::{Anchor, Cap, Primitive, TextLabel, TextRole};
use super::style::{Ink, Paint, Stroke, Weight};

/// How much wider than the body a band's edge is, in base strokes.
const BAND_EDGE_STROKES: f32 = 1.6;

/// Fit, metrics and palette for one paint pass.
pub(super) struct Painter<'a> {
    pub fit: Fit,
    pub metrics: Metrics,
    pub palette: &'a Palette,
}

/// Where a free label is drawn: centred vertically on its point.
pub(super) fn label_placement(label: &TextLabel, fit: &Fit) -> LabelPlacement {
    LabelPlacement {
        at: fit.point(label.at),
        anchor: label.anchor,
        baseline: Baseline::Centre,
    }
}

impl Painter<'_> {
    pub fn part(&self, frame: &mut Frame, part: &Part, is_highlighted: bool) {
        match &part.primitive {
            Primitive::Band { points, width, cap } => {
                self.band(frame, part, points, *width, *cap, is_highlighted);
            }
            Primitive::Callout(callout) => {
                let color = self.annotation_color(part, is_highlighted);
                let geometry = callout_geometry(callout, &self.fit, &self.metrics);
                let dash = self.metrics.dash();
                for (from, to) in &callout.extensions {
                    let path = Path::line(self.fit.point(*from), self.fit.point(*to));
                    frame.stroke(&path, self.line(color, Weight::Thin, Some(&dash)));
                }
                let (a, b) = geometry.line;
                frame.stroke(&Path::line(a, b), self.line(color, Weight::Thin, None));
                for head in &geometry.heads {
                    frame.fill(&head_path(head), color);
                }
                if let (Some(text), Some(placement)) = (&callout.label, geometry.label) {
                    self.text(frame, text.as_str(), placement, TextRole::Caption, color);
                }
            }
            Primitive::Leader(leader) => {
                let color = self.annotation_color(part, is_highlighted);
                let (at, end) = (self.fit.point(leader.at), self.fit.point(leader.label_at));
                frame.fill(&Path::circle(at, self.metrics.leader_dot()), color);
                frame.stroke(&Path::line(at, end), self.line(color, Weight::Thin, None));
                let placement = leader_label(end, leader.anchor, &self.metrics);
                self.text(
                    frame,
                    leader.label.as_str(),
                    placement,
                    TextRole::Caption,
                    color,
                );
            }
            Primitive::Label(label) => {
                let color = self.annotation_color(part, is_highlighted);
                let placement = label_placement(label, &self.fit);
                self.text(frame, label.text.as_str(), placement, label.role, color);
            }
            shape => {
                if let Some(path) = self.shape_path(shape) {
                    self.fill_and_stroke(frame, &path, part, is_highlighted);
                }
            }
        }
    }

    fn shape_path(&self, primitive: &Primitive) -> Option<Path> {
        let fit = &self.fit;
        Some(match primitive {
            Primitive::Line { from, to } => Path::line(fit.point(*from), fit.point(*to)),
            Primitive::Rect { rect } => {
                let r = fit.rect(*rect);
                Path::rectangle(r.position(), r.size())
            }
            Primitive::RoundedRect { rect, radius } => {
                let r = fit.rect(*rect);
                Path::rounded_rectangle(r.position(), r.size(), fit.length(*radius).into())
            }
            Primitive::Circle { center, radius } => {
                Path::circle(fit.point(*center), fit.length(*radius))
            }
            Primitive::Arc {
                center,
                radius,
                start,
                end,
            } => Path::new(|b| {
                b.arc(canvas::path::Arc {
                    center: fit.point(*center),
                    radius: fit.length(*radius),
                    start_angle: *start,
                    end_angle: *end,
                });
            }),
            Primitive::Polyline { points, is_closed } => {
                polyline_path(points.iter().map(|p| fit.point(*p)), *is_closed)?
            }
            Primitive::Band { .. }
            | Primitive::Callout(_)
            | Primitive::Leader(_)
            | Primitive::Label(_)
            | Primitive::HoverArea { .. } => return None,
        })
    }

    fn fill_and_stroke(&self, frame: &mut Frame, path: &Path, part: &Part, is_highlighted: bool) {
        if let Some(fill) = part.style.fill {
            frame.fill(path, self.palette.tinted(fill.paint, fill.tint));
        }
        if let Some(stroke) = part.style.stroke {
            let color = self.stroke_color(stroke, is_highlighted);
            let dash = self.metrics.dash();
            let dash = stroke.is_dashed.then_some(&dash);
            frame.stroke(path, self.line(color, stroke.weight, dash));
        }
    }

    /// A band: the edge, the body knocked out to the panel, then the tint.
    fn band(
        &self,
        frame: &mut Frame,
        part: &Part,
        points: &[Point],
        width: f32,
        cap: Cap,
        is_highlighted: bool,
    ) {
        let Some(path) = polyline_path(points.iter().map(|p| self.fit.point(*p)), false) else {
            return;
        };
        let body = self.fit.length(width);
        let edge_color = match part.style.stroke {
            Some(stroke) => self.stroke_color(stroke, is_highlighted),
            None if is_highlighted => self.palette.ink(Ink::Accent),
            None => self.palette.ink(Ink::Secondary),
        };
        let edge = body + BAND_EDGE_STROKES * self.metrics.stroke();
        frame.stroke(&path, band_stroke(edge_color, edge, cap));
        frame.stroke(&path, band_stroke(self.palette.ink(Ink::Panel), body, cap));
        if let Some(fill) = part.style.fill {
            let tint = self.palette.tinted(fill.paint, fill.tint);
            frame.stroke(&path, band_stroke(tint, body, cap));
        }
    }

    fn stroke_color(&self, stroke: Stroke, is_highlighted: bool) -> Color {
        if is_highlighted {
            self.palette.ink(Ink::Accent)
        } else {
            self.palette.paint(stroke.paint)
        }
    }

    fn annotation_color(&self, part: &Part, is_highlighted: bool) -> Color {
        if is_highlighted {
            return self.palette.ink(Ink::Accent);
        }
        let paint = part
            .style
            .stroke
            .map_or(Paint::Ink(Ink::Faint), |s| s.paint);
        self.palette.paint(paint)
    }

    fn line<'d>(
        &self,
        color: Color,
        weight: Weight,
        dash: Option<&'d [f32; 2]>,
    ) -> canvas::Stroke<'d> {
        let mut stroke = canvas::Stroke::default()
            .with_color(color)
            .with_width(self.metrics.stroke() * weight.factor())
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        if let Some(segments) = dash {
            stroke.line_cap = LineCap::Butt;
            stroke.line_dash = LineDash {
                segments: segments.as_slice(),
                offset: 0,
            };
        }
        stroke
    }

    fn text(
        &self,
        frame: &mut Frame,
        content: &str,
        placement: LabelPlacement,
        role: TextRole,
        color: Color,
    ) {
        let align_x = match placement.anchor {
            Anchor::Start => Horizontal::Left,
            Anchor::Middle => Horizontal::Center,
            Anchor::End => Horizontal::Right,
        };
        let align_y = match placement.baseline {
            Baseline::Above => Vertical::Bottom,
            Baseline::Centre => Vertical::Center,
        };
        frame.fill_text(canvas::Text {
            content: content.to_owned(),
            position: placement.at,
            color,
            size: Pixels(self.metrics.text(role)),
            font: font_of(role),
            align_x: align_x.into(),
            align_y,
            ..canvas::Text::default()
        });
    }
}

fn font_of(role: TextRole) -> Font {
    match role {
        TextRole::Caption | TextRole::Note => Font::DEFAULT,
        TextRole::Mono => Font {
            weight: font::Weight::Bold,
            ..Font::MONOSPACE
        },
    }
}

fn band_stroke(color: Color, width: f32, cap: Cap) -> canvas::Stroke<'static> {
    canvas::Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_join(LineJoin::Round)
        .with_line_cap(match cap {
            Cap::Round => LineCap::Round,
            Cap::Square => LineCap::Square,
            Cap::Flat => LineCap::Butt,
        })
}

fn head_path(head: &Head) -> Path {
    Path::new(|b| {
        b.move_to(head[0]);
        b.line_to(head[1]);
        b.line_to(head[2]);
        b.close();
    })
}

fn polyline_path(mut points: impl Iterator<Item = Point>, is_closed: bool) -> Option<Path> {
    let first = points.next()?;
    Some(Path::new(|b| {
        b.move_to(first);
        for p in points.by_ref() {
            b.line_to(p);
        }
        if is_closed {
            b.close();
        }
    }))
}
