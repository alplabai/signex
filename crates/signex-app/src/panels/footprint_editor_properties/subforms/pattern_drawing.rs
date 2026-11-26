//! The pattern illustration (mockup row 126): every instance with its number, the
//! dX / dY spacing, the source pad outlined and skipped instances dashed.

use iced::{Point, Rectangle};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Paint, Part, Primitive, Stroke, Style, TextRole,
    Tint, Weight,
};

use super::super::super::{ArrayKindSummary, ArraySummary};
use super::drawing::{bottom_note, canvas, centred, label};
use crate::styles::board_palette::BoardPalette;
use signex_bake::array::{ArrayPreview, InstancePreview as Instance};

pub(super) const COUNT: FieldId = FieldId::new("pattern.count");
pub(super) const NX: FieldId = FieldId::new("pattern.nx");
pub(super) const NY: FieldId = FieldId::new("pattern.ny");
pub(super) const DX: FieldId = FieldId::new("pattern.dx");
pub(super) const DY: FieldId = FieldId::new("pattern.dy");
pub(super) const SWEEP: FieldId = FieldId::new("pattern.sweep");
pub(super) const MASK: FieldId = FieldId::new("pattern.mask");
pub(super) const NUMBERING: FieldId = FieldId::new("pattern.numbering");

/// A pad of unknown size is drawn this big (mm).
const FALLBACK_PAD_MM: [f64; 2] = [1.0, 1.0];
/// The area the instances are fitted into, and the largest scale.
const FIT_W: f64 = 150.0;
const FIT_H: f64 = 88.0;
const MAX_SCALE: f64 = 32.0;
const CENTRE: Point = Point::new(146.0, 76.0);

/// Fits pattern millimetres into the view box (y up on the board, down on
/// screen).
struct Fit {
    scale: f64,
    mid: [f64; 2],
}

impl Fit {
    fn new(instances: &[Instance], pad: [f64; 2]) -> Self {
        let (mut lo, mut hi) = ([0.0_f64; 2], [0.0_f64; 2]);
        for c in instances {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(c.offset_mm[axis]);
                hi[axis] = hi[axis].max(c.offset_mm[axis]);
            }
        }
        let span = [hi[0] - lo[0] + pad[0], hi[1] - lo[1] + pad[1]];
        let scale = (FIT_W / span[0]).min(FIT_H / span[1]).min(MAX_SCALE);
        Self {
            scale,
            mid: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
        }
    }

    fn point(&self, mm: [f64; 2]) -> Point {
        Point::new(
            (f64::from(CENTRE.x) + (mm[0] - self.mid[0]) * self.scale) as f32,
            (f64::from(CENTRE.y) - (mm[1] - self.mid[1]) * self.scale) as f32,
        )
    }

    fn pad(&self, mm: [f64; 2], pad: [f64; 2]) -> Rectangle {
        centred(
            self.point(mm),
            (pad[0] * self.scale) as f32,
            (pad[1] * self.scale) as f32,
        )
    }
}

/// The fields a kept instance shows.
fn layout_fields(pattern: &ArraySummary) -> Vec<FieldId> {
    match pattern.kind {
        ArrayKindSummary::Linear { .. } => vec![COUNT, NUMBERING],
        ArrayKindSummary::Grid { .. } => vec![NX, NY, NUMBERING],
        ArrayKindSummary::Polar { .. } => vec![COUNT, SWEEP, NUMBERING],
    }
}

pub(super) fn drawing(pattern: &ArraySummary, preview: &ArrayPreview, i18n: &I18n) -> Illustration {
    let copper = Paint::Document(BoardPalette::new().layer(PcbLayer::Copper(CopperLayer::Top)));
    let pad = pattern.source_size_mm.unwrap_or(FALLBACK_PAD_MM);
    let source_only = [Instance {
        i: 0,
        j: 0,
        offset_mm: [0.0, 0.0],
        number: pattern.source_number.clone(),
        is_skipped: false,
    }];
    let (instances, note) = match preview {
        ArrayPreview::Instances(all) if !all.is_empty() => {
            (all.as_slice(), t!(i18n, ids::EDITORS_ROLE_PATTERN_LEGEND))
        }
        ArrayPreview::Instances(_) | ArrayPreview::Unresolved => (
            source_only.as_slice(),
            t!(i18n, ids::EDITORS_ROLE_PATTERN_UNRESOLVED),
        ),
        ArrayPreview::TooMany(n) => (
            source_only.as_slice(),
            t!(i18n, ids::EDITORS_ROLE_PATTERN_TOO_MANY, "count" => *n),
        ),
    };
    let fit = Fit::new(instances, pad);
    let kept_fields = layout_fields(pattern);
    let mut drawing = canvas();
    for (index, c) in instances.iter().enumerate() {
        let rect = fit.pad(c.offset_mm, pad);
        let shape = Primitive::RoundedRect {
            rect,
            radius: rect.height.min(rect.width) * 0.12,
        };
        let part = if c.is_skipped {
            let dashed = Style {
                stroke: Some(Stroke {
                    paint: copper,
                    weight: Weight::Thin,
                    is_dashed: true,
                }),
                fill: None,
            };
            Part::new(shape, dashed).linked(MASK)
        } else if index == 0 {
            Part::new(
                shape,
                Style::outline(Ink::Accent, Weight::Heavy).with_fill(copper, Tint::Strong),
            )
            .linked_all(kept_fields.clone())
        } else {
            Part::new(shape, Style::edge().with_fill(copper, Tint::Strong))
                .linked_all(kept_fields.clone())
        };
        drawing = drawing.part(part).part(label(
            fit.point(c.offset_mm),
            UserText::new(c.number.as_str()),
            Anchor::Middle,
            TextRole::Mono,
        ));
    }
    drawing = spacing_callouts(drawing, pattern, instances, &fit, pad, i18n);
    drawing.part(bottom_note(note))
}

/// dX over the first row and dY beside the first column of a linear or
/// grid pattern; the centre point of a polar one.
fn spacing_callouts(
    mut drawing: Illustration,
    pattern: &ArraySummary,
    instances: &[Instance],
    fit: &Fit,
    pad: [f64; 2],
    i18n: &I18n,
) -> Illustration {
    if let ArrayKindSummary::Polar {
        center_position_mm: Some(center),
        ..
    } = pattern.kind
        && let Some(source) = pattern.source_position_mm
    {
        let at = fit.point([center[0] - source[0], center[1] - source[1]]);
        return drawing.part(
            Part::new(
                Primitive::Circle {
                    center: at,
                    radius: 2.0,
                },
                Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Solid),
            )
            .linked(SWEEP),
        );
    }
    let Some(first) = instances.iter().find(|c| c.i == 0 && c.j == 0) else {
        return drawing;
    };
    let half_h = (pad[1] * fit.scale) as f32 / 2.0;
    let half_w = (pad[0] * fit.scale) as f32 / 2.0;
    let a = fit.point(first.offset_mm);
    if let Some(next) = instances.iter().find(|c| c.i == 1 && c.j == 0) {
        let b = fit.point(next.offset_mm);
        let y = a.y.min(b.y) - half_h - 10.0;
        let dx = next.offset_mm[0] - first.offset_mm[0];
        let text = t!(i18n, ids::EDITORS_ROLE_PATTERN_DX_CALLOUT, "value" => format!("{dx:.3}"));
        drawing = drawing.part(
            Part::callout(
                Callout::new(Point::new(a.x, y), Point::new(b.x, y))
                    .label(text)
                    .extension(
                        Point::new(a.x, a.y - half_h - 2.0),
                        Point::new(a.x, y - 3.0),
                    )
                    .extension(
                        Point::new(b.x, b.y - half_h - 2.0),
                        Point::new(b.x, y - 3.0),
                    ),
            )
            .linked(DX),
        );
    }
    if let Some(below) = instances.iter().find(|c| c.i == 0 && c.j == 1) {
        let b = fit.point(below.offset_mm);
        let x = a.x.min(b.x) - half_w - 10.0;
        let dy = below.offset_mm[1] - first.offset_mm[1];
        let text = t!(i18n, ids::EDITORS_ROLE_PATTERN_DY_CALLOUT, "value" => format!("{dy:.3}"));
        drawing = drawing.part(
            Part::callout(
                Callout::new(Point::new(x, a.y), Point::new(x, b.y))
                    .label(text)
                    .label_at(Point::new(x - 4.0, (a.y + b.y) / 2.0 + 3.0), Anchor::End)
                    .extension(
                        Point::new(a.x - half_w - 2.0, a.y),
                        Point::new(x - 3.0, a.y),
                    )
                    .extension(
                        Point::new(b.x - half_w - 2.0, b.y),
                        Point::new(x - 3.0, b.y),
                    ),
            )
            .linked(DY),
        );
    }
    drawing
}
