//! A compiled note's frames lowered to [`NoteDrawing`]: glyph outlines and
//! shapes as paths in mm, links as rectangles. Every page is drawn, one under
//! the other. Images and clipping inside the source are not drawn (no file
//! is read, and the note box clips a Fixed box note as a whole).

use typst::layout::{Frame, FrameItem, Transform};
use typst::model::Destination;
use typst::text::TextItem;
use typst::visualize::{CurveItem, FillRule, Geometry, Paint, Shape};
use typst_layout::PagedDocument;

use super::shape::{NoteDrawing, NoteLink, NotePaint, NotePathOp, NoteShape, NoteShapeKind};
use super::{MAX_NOTE_SHAPES, PT_PER_MM};

/// A 2-D affine map: `x' = a x + c y + e`, `y' = b x + d y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    const fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    const fn translate(x: f64, y: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: x,
            f: y,
        }
    }

    fn of(t: &Transform) -> Self {
        Self {
            a: t.sx.get(),
            b: t.ky.get(),
            c: t.kx.get(),
            d: t.sy.get(),
            e: t.tx.to_pt(),
            f: t.ty.to_pt(),
        }
    }

    /// `self` after `inner`: `inner` applies first.
    fn then(self, inner: Self) -> Self {
        Self {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        [
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        ]
    }

    /// How much a length grows under the map (the square root of the area
    /// scale), for stroke widths.
    fn length_scale(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }
}

/// What the walk collects.
#[derive(Default)]
struct Sink {
    shapes: Vec<NoteShape>,
    links: Vec<NoteLink>,
}

impl Sink {
    fn push(&mut self, shape: NoteShape) {
        if shape.path.len() > 1 {
            self.shapes.push(shape);
        }
    }
}

/// Lower every page of `document`, one under the other, in mm.
pub(super) fn lower(document: &PagedDocument) -> Result<NoteDrawing, String> {
    let mm_per_pt = 1.0 / PT_PER_MM;
    let mut sink = Sink::default();
    let (mut width_mm, mut height_mm) = (0.0_f64, 0.0_f64);
    for page in document.pages() {
        let top = Affine::translate(0.0, height_mm).then(Affine::scale(mm_per_pt, mm_per_pt));
        walk(&page.frame, top, &mut sink);
        width_mm = width_mm.max(page.frame.width().to_pt() * mm_per_pt);
        height_mm += page.frame.height().to_pt() * mm_per_pt;
        if sink.shapes.len() > MAX_NOTE_SHAPES {
            return Err(format!(
                "the note draws more than {MAX_NOTE_SHAPES} shapes; split it into smaller notes"
            ));
        }
    }
    Ok(NoteDrawing {
        width_mm,
        height_mm,
        shapes: sink.shapes,
        links: sink.links,
    })
}

fn walk(frame: &Frame, ts: Affine, sink: &mut Sink) {
    for (pos, item) in frame.items() {
        let at = ts.then(Affine::translate(pos.x.to_pt(), pos.y.to_pt()));
        match item {
            FrameItem::Group(group) => {
                walk(&group.frame, at.then(Affine::of(&group.transform)), sink)
            }
            FrameItem::Text(text) => glyphs(text, at, sink),
            FrameItem::Shape(shape, _) => shape_paths(shape, at, sink),
            FrameItem::Link(Destination::Url(url), size) => {
                let [x0, y0] = at.apply(0.0, 0.0);
                let [x1, y1] = at.apply(size.x.to_pt(), size.y.to_pt());
                sink.links.push(NoteLink {
                    rect: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
                    target: url.as_str().to_string(),
                });
            }
            FrameItem::Link(..) | FrameItem::Image(..) | FrameItem::Tag(_) => {}
        }
    }
}

/// Each glyph of `text` as its outline, filled in the text's paint.
fn glyphs(text: &TextItem, at: Affine, sink: &mut Sink) {
    let upem = text.font.units_per_em();
    if !(upem.is_finite() && upem > 0.0) {
        return;
    }
    let scale = text.size.to_pt() / upem;
    let paint = paint_of(&text.fill);
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    for glyph in &text.glyphs {
        let dx = x + glyph.x_offset.at(text.size).to_pt();
        let dy = y + glyph.y_offset.at(text.size).to_pt();
        // Font units are y-up.
        let ts = at
            .then(Affine::translate(dx, -dy))
            .then(Affine::scale(scale, -scale));
        let mut outline = Outline {
            ts,
            path: Vec::new(),
        };
        if text
            .font
            .ttf()
            .outline_glyph(ttf_parser::GlyphId(glyph.id), &mut outline)
            .is_some()
        {
            sink.push(NoteShape {
                path: outline.path,
                kind: NoteShapeKind::Fill { even_odd: false },
                paint,
            });
        }
        x += glyph.x_advance.at(text.size).to_pt();
        y += glyph.y_advance.at(text.size).to_pt();
    }
}

/// A shape's fill and its outline.
fn shape_paths(shape: &Shape, at: Affine, sink: &mut Sink) {
    let path = match &shape.geometry {
        Geometry::Line(to) => vec![
            NotePathOp::Move(at.apply(0.0, 0.0)),
            NotePathOp::Line(at.apply(to.x.to_pt(), to.y.to_pt())),
        ],
        Geometry::Rect(size) => {
            let (w, h) = (size.x.to_pt(), size.y.to_pt());
            vec![
                NotePathOp::Move(at.apply(0.0, 0.0)),
                NotePathOp::Line(at.apply(w, 0.0)),
                NotePathOp::Line(at.apply(w, h)),
                NotePathOp::Line(at.apply(0.0, h)),
                NotePathOp::Close,
            ]
        }
        Geometry::Curve(curve) => curve
            .0
            .iter()
            .map(|item| match item {
                CurveItem::Move(p) => NotePathOp::Move(at.apply(p.x.to_pt(), p.y.to_pt())),
                CurveItem::Line(p) => NotePathOp::Line(at.apply(p.x.to_pt(), p.y.to_pt())),
                CurveItem::Cubic(c1, c2, p) => NotePathOp::Cubic(
                    at.apply(c1.x.to_pt(), c1.y.to_pt()),
                    at.apply(c2.x.to_pt(), c2.y.to_pt()),
                    at.apply(p.x.to_pt(), p.y.to_pt()),
                ),
                CurveItem::Close => NotePathOp::Close,
            })
            .collect(),
    };
    let is_line = matches!(shape.geometry, Geometry::Line(_));
    if let Some(fill) = shape.fill.as_ref().filter(|_| !is_line) {
        sink.push(NoteShape {
            path: path.clone(),
            kind: NoteShapeKind::Fill {
                even_odd: shape.fill_rule == FillRule::EvenOdd,
            },
            paint: paint_of(fill),
        });
    }
    if let Some(stroke) = &shape.stroke {
        let width_mm = stroke.thickness.to_pt() * at.length_scale();
        if width_mm > 0.0 && width_mm.is_finite() {
            sink.push(NoteShape {
                path,
                kind: NoteShapeKind::Stroke { width_mm },
                paint: paint_of(&stroke.paint),
            });
        }
    }
}

/// Typst's default black is the note's ink; any other solid colour is kept.
/// A gradient or tiling draws in the ink.
fn paint_of(paint: &Paint) -> NotePaint {
    match paint {
        Paint::Solid(color) => match color.to_vec4_u8() {
            [0, 0, 0, 255] => NotePaint::Ink,
            rgba => NotePaint::Rgba(rgba),
        },
        Paint::Gradient(_) | Paint::Tiling(_) => NotePaint::Ink,
    }
}

/// A glyph outline, mapped to mm as it is read.
struct Outline {
    ts: Affine,
    path: Vec<NotePathOp>,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path
            .push(NotePathOp::Move(self.ts.apply(f64::from(x), f64::from(y))));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path
            .push(NotePathOp::Line(self.ts.apply(f64::from(x), f64::from(y))));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.push(NotePathOp::Quad(
            self.ts.apply(f64::from(x1), f64::from(y1)),
            self.ts.apply(f64::from(x), f64::from(y)),
        ));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.push(NotePathOp::Cubic(
            self.ts.apply(f64::from(x1), f64::from(y1)),
            self.ts.apply(f64::from(x2), f64::from(y2)),
            self.ts.apply(f64::from(x), f64::from(y)),
        ));
    }

    fn close(&mut self) {
        self.path.push(NotePathOp::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::Affine;

    #[test]
    fn a_map_after_a_map_applies_the_inner_first() {
        let scale = Affine::scale(2.0, 3.0);
        let shift = Affine::translate(1.0, 1.0);
        assert_eq!(scale.then(shift).apply(1.0, 1.0), [4.0, 6.0]);
        assert_eq!(shift.then(scale).apply(1.0, 1.0), [3.0, 4.0]);
        assert!((Affine::scale(2.0, 8.0).length_scale() - 4.0).abs() < 1e-12);
    }
}
