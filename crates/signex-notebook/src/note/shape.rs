//! A compiled note as vector paths in mm, x right and y down from the note's
//! top-left corner: what the canvas, the SVG and the PDF export draw.

/// One step of a path, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotePathOp {
    Move([f64; 2]),
    Line([f64; 2]),
    /// A quadratic Bézier: the control point, then the end.
    Quad([f64; 2], [f64; 2]),
    /// A cubic Bézier: two control points, then the end.
    Cubic([f64; 2], [f64; 2], [f64; 2]),
    Close,
}

/// The colour a shape is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePaint {
    /// The note's ink: what Typst draws in its default black, so a note
    /// reads on a dark and a light sheet alike and prints like the sheet.
    Ink,
    /// A colour the source chose, RGBA.
    Rgba([u8; 4]),
}

/// How a shape is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteShapeKind {
    /// Filled; `even_odd` picks the even-odd rule over non-zero.
    Fill { even_odd: bool },
    /// Outlined at a width in mm.
    Stroke { width_mm: f64 },
}

/// One filled or stroked path.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteShape {
    pub path: Vec<NotePathOp>,
    pub kind: NoteShapeKind,
    pub paint: NotePaint,
}

/// A link the source made with `#link`: where it sits and what it names.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteLink {
    /// `[min_x, min_y, max_x, max_y]` in mm.
    pub rect: [f64; 4],
    /// The URL or the project-relative path, as written.
    pub target: String,
}

impl NoteLink {
    /// Whether `(x, y)` in mm, from the note's top-left, is on the link.
    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let [x0, y0, x1, y1] = self.rect;
        x >= x0 && x <= x1 && y >= y0 && y <= y1
    }
}

/// A compiled note.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoteDrawing {
    /// The width and height the text takes, in mm.
    pub width_mm: f64,
    pub height_mm: f64,
    pub shapes: Vec<NoteShape>,
    pub links: Vec<NoteLink>,
}

/// Segments a quadratic or cubic curve is split into when flattened.
const CURVE_STEPS: usize = 8;

impl NoteShape {
    /// The path as closed polylines (fills) or open ones (strokes), curves
    /// split into short lines: what the canvas tessellates.
    #[must_use]
    pub fn contours(&self) -> Vec<Vec<[f64; 2]>> {
        let mut contours: Vec<Vec<[f64; 2]>> = Vec::new();
        let mut current: Vec<[f64; 2]> = Vec::new();
        let mut at = [0.0, 0.0];
        let mut start = at;
        for op in &self.path {
            // A segment after a close starts where the closed path did.
            if current.is_empty() && !matches!(op, NotePathOp::Move(_) | NotePathOp::Close) {
                current.push(at);
            }
            match *op {
                NotePathOp::Move(p) => {
                    if current.len() > 1 {
                        contours.push(std::mem::take(&mut current));
                    }
                    current.clear();
                    current.push(p);
                    at = p;
                    start = p;
                }
                NotePathOp::Line(p) => {
                    current.push(p);
                    at = p;
                }
                NotePathOp::Quad(c, p) => {
                    for step in 1..=CURVE_STEPS {
                        let t = step as f64 / CURVE_STEPS as f64;
                        current.push(quad(at, c, p, t));
                    }
                    at = p;
                }
                NotePathOp::Cubic(c1, c2, p) => {
                    for step in 1..=CURVE_STEPS {
                        let t = step as f64 / CURVE_STEPS as f64;
                        current.push(cubic(at, c1, c2, p, t));
                    }
                    at = p;
                }
                NotePathOp::Close => {
                    if current.len() > 1 {
                        contours.push(std::mem::take(&mut current));
                    }
                    current.clear();
                    at = start;
                }
            }
        }
        if current.len() > 1 {
            contours.push(current);
        }
        contours
    }

    /// The bounding box of the path's points, `[min_x, min_y, max_x, max_y]`.
    #[must_use]
    pub fn bounds(&self) -> Option<[f64; 4]> {
        let mut points = self.path.iter().flat_map(|op| match *op {
            NotePathOp::Move(p) | NotePathOp::Line(p) => vec![p],
            NotePathOp::Quad(c, p) => vec![c, p],
            NotePathOp::Cubic(c1, c2, p) => vec![c1, c2, p],
            NotePathOp::Close => Vec::new(),
        });
        let first = points.next()?;
        Some(
            points.fold([first[0], first[1], first[0], first[1]], |b, p| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            }),
        )
    }
}

fn quad(p0: [f64; 2], c: [f64; 2], p1: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    [
        u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p1[0],
        u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p1[1],
    ]
}

fn cubic(p0: [f64; 2], c1: [f64; 2], c2: [f64; 2], p1: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [
        a * p0[0] + b * c1[0] + c * c2[0] + d * p1[0],
        a * p0[1] + b * c1[1] + c * c2[1] + d * p1[1],
    ]
}
