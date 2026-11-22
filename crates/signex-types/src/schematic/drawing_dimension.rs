//! A dimension annotation on a schematic sheet (#1267): a measured span
//! between two points, drawn as extension lines, a dimension line with
//! arrowheads and the measured text.
//!
//! Annotation only, and free once placed: it never follows the objects it
//! measured. Geometry lives here so the canvas, hit test, SVG and PDF draw
//! the same lines; see docs/RENDERING_RULES.md, "Dimensions".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::drawing_bezier::{LINE_END_ARROW_HALF_WIDTH_MM, LINE_END_ARROW_LENGTH_MM};
use super::{Point, StrokeColor};

/// Text height of a dimension's label (mm).
pub const DIMENSION_TEXT_MM: f64 = 1.27;

/// Gap between a measured point and its extension line (mm).
pub const DIMENSION_EXTENSION_GAP_MM: f64 = 0.5;

/// How far an extension line runs past the dimension line (mm).
pub const DIMENSION_EXTENSION_OVERSHOOT_MM: f64 = 1.0;

/// Gap between the dimension line and its text (mm).
pub const DIMENSION_TEXT_GAP_MM: f64 = 0.5;

/// Which span a dimension measures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionDirection {
    /// The X distance; the dimension line is horizontal.
    #[default]
    Horizontal,
    /// The Y distance; the dimension line is vertical.
    Vertical,
    /// The straight distance; the line runs parallel to the two points.
    Aligned,
}

impl DimensionDirection {
    pub const ALL: [Self; 3] = [Self::Horizontal, Self::Vertical, Self::Aligned];
}

/// The unit a dimension's text is written in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionUnits {
    #[default]
    Mm,
    Mil,
    In,
}

impl DimensionUnits {
    pub const ALL: [Self; 3] = [Self::Mm, Self::Mil, Self::In];

    /// The unit symbol written after the value.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Mil => "mil",
            Self::In => "in",
        }
    }

    /// `mm` expressed in this unit.
    pub fn from_mm(self, mm: f64) -> f64 {
        match self {
            Self::Mm => mm,
            Self::Mil => mm / 0.0254,
            Self::In => mm / 25.4,
        }
    }
}

/// The arrowheads at the ends of the dimension line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionArrows {
    /// Filled heads inside the extension lines, tips on them.
    #[default]
    FilledInside,
    /// Filled heads outside the extension lines, pointing in.
    FilledOutside,
    /// Open (two-stroke) heads inside the extension lines.
    Open,
}

impl DimensionArrows {
    pub const ALL: [Self; 3] = [Self::FilledInside, Self::FilledOutside, Self::Open];

    pub fn is_filled(self) -> bool {
        !matches!(self, Self::Open)
    }
}

/// Highest number of decimals a dimension writes.
pub const DIMENSION_MAX_PRECISION: u8 = 3;

fn two() -> u8 {
    2
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A dimension measuring `from` to `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchDimension {
    pub uuid: Uuid,
    pub from: Point,
    pub to: Point,
    /// Signed distance of the dimension line from the measured points (mm),
    /// along the dimension's normal (see [`SchDimension::normal`]).
    pub offset: f64,
    pub direction: DimensionDirection,
    #[serde(default)]
    pub units: DimensionUnits,
    /// Decimals written, `0..=3`.
    #[serde(default = "two")]
    pub precision: u8,
    #[serde(default)]
    pub arrows: DimensionArrows,
    /// Text written instead of the measured value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_text: Option<String>,
    /// Stroke width in mm; `0` draws the sheet's default drawing width.
    #[serde(default)]
    pub width: f64,
    /// `None` draws in the sheet's drawing colour.
    #[serde(default)]
    pub stroke_color: Option<StrokeColor>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// Where a dimension's lines and text go, in sheet mm.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionGeometry {
    /// The dimension line, from the `from` side to the `to` side.
    pub line: [Point; 2],
    /// Extension line at `from`, then at `to`.
    pub extensions: [[Point; 2]; 2],
    /// The two arrowheads, each `[tip, wing, wing]`.
    pub arrows: [[Point; 3]; 2],
    /// Line pieces past the extension lines (outside arrows only).
    pub outside_tails: Vec<[Point; 2]>,
    /// Horizontal-centre anchor of the text, on the outer side of the line.
    pub text_at: Point,
    /// Text rotation, clockwise-positive degrees, kept readable
    /// (`-90 <= angle < 90`).
    pub text_angle_deg: f64,
    /// `true` when [`Self::text_at`] is the text's top edge (the text hangs
    /// away from the line), `false` when it is the bottom edge.
    pub text_hangs: bool,
}

impl SchDimension {
    /// A dimension with the default text (mm, two decimals, filled inside
    /// arrows, measured value) in the sheet's drawing colour.
    pub fn new(
        uuid: Uuid,
        from: Point,
        to: Point,
        offset: f64,
        direction: DimensionDirection,
    ) -> Self {
        Self {
            uuid,
            from,
            to,
            offset,
            direction,
            units: DimensionUnits::Mm,
            precision: two(),
            arrows: DimensionArrows::FilledInside,
            override_text: None,
            width: 0.0,
            stroke_color: None,
            locked: false,
        }
    }

    /// The direction a linear dimension from `from` to `to` takes: the
    /// axis the two points are further apart along (horizontal on a tie).
    pub fn linear_direction(from: Point, to: Point) -> DimensionDirection {
        if (to.x - from.x).abs() >= (to.y - from.y).abs() {
            DimensionDirection::Horizontal
        } else {
            DimensionDirection::Vertical
        }
    }

    /// The offset that puts the dimension line through `p`.
    pub fn offset_to(&self, p: Point) -> f64 {
        let (nx, ny) = self.normal();
        match self.direction {
            DimensionDirection::Horizontal => p.y - self.from.y,
            DimensionDirection::Vertical => p.x - self.from.x,
            DimensionDirection::Aligned => (p.x - self.from.x) * nx + (p.y - self.from.y) * ny,
        }
    }

    /// The offset for a line through `p`, or through `p` mirrored across the
    /// middle of the measured span when `flipped`, which puts a line outside
    /// the two points on their other side even when they differ in height.
    pub fn offset_on_side(&self, p: Point, flipped: bool) -> f64 {
        if !flipped {
            return self.offset_to(p);
        }
        self.offset_to(Point::new(
            self.from.x + self.to.x - p.x,
            self.from.y + self.to.y - p.y,
        ))
    }

    /// The measured length in mm.
    pub fn measured_mm(&self) -> f64 {
        let (dx, dy) = (self.to.x - self.from.x, self.to.y - self.from.y);
        match self.direction {
            DimensionDirection::Horizontal => dx.abs(),
            DimensionDirection::Vertical => dy.abs(),
            DimensionDirection::Aligned => dx.hypot(dy),
        }
    }

    /// The measured value in the dimension's unit and precision with the
    /// unit symbol (`12.70 mm`), ignoring any override.
    pub fn measured_text(&self) -> String {
        let decimals = usize::from(self.precision.min(DIMENSION_MAX_PRECISION));
        let value = self.units.from_mm(self.measured_mm());
        format!("{value:.decimals$} {}", self.units.suffix())
    }

    /// The text drawn on the sheet: the override when one is set, else
    /// [`Self::measured_text`].
    pub fn label(&self) -> String {
        match &self.override_text {
            Some(text) => text.clone(),
            None => self.measured_text(),
        }
    }

    /// Unit vector along the measured span.
    fn axis(&self) -> (f64, f64) {
        match self.direction {
            DimensionDirection::Horizontal => (1.0, 0.0),
            DimensionDirection::Vertical => (0.0, 1.0),
            DimensionDirection::Aligned => {
                let (dx, dy) = (self.to.x - self.from.x, self.to.y - self.from.y);
                let len = dx.hypot(dy);
                if len <= 1e-12 {
                    (1.0, 0.0)
                } else {
                    (dx / len, dy / len)
                }
            }
        }
    }

    /// The unit normal [`Self::offset`] is measured along: the axis turned
    /// a quarter (`(0, 1)` for a horizontal dimension, `(1, 0)` for a
    /// vertical one).
    pub fn normal(&self) -> (f64, f64) {
        match self.direction {
            DimensionDirection::Horizontal => (0.0, 1.0),
            DimensionDirection::Vertical => (1.0, 0.0),
            DimensionDirection::Aligned => {
                let (ux, uy) = self.axis();
                (-uy, ux)
            }
        }
    }

    /// The dimension line's two ends.
    pub fn line(&self) -> [Point; 2] {
        let (nx, ny) = self.normal();
        match self.direction {
            DimensionDirection::Horizontal => {
                let y = self.from.y + self.offset;
                [Point::new(self.from.x, y), Point::new(self.to.x, y)]
            }
            DimensionDirection::Vertical => {
                let x = self.from.x + self.offset;
                [Point::new(x, self.from.y), Point::new(x, self.to.y)]
            }
            DimensionDirection::Aligned => [
                Point::new(
                    self.from.x + nx * self.offset,
                    self.from.y + ny * self.offset,
                ),
                Point::new(self.to.x + nx * self.offset, self.to.y + ny * self.offset),
            ],
        }
    }

    /// Every line, head and the text anchor.
    pub fn geometry(&self) -> DimensionGeometry {
        let line = self.line();
        let extensions = [extension(self.from, line[0]), extension(self.to, line[1])];
        let (dx, dy) = (line[1].x - line[0].x, line[1].y - line[0].y);
        let len = dx.hypot(dy);
        let (ux, uy) = if len <= 1e-12 {
            self.axis()
        } else {
            (dx / len, dy / len)
        };
        let back = |p: Point, k: f64| Point::new(p.x - ux * k, p.y - uy * k);
        let (arrows, outside_tails) = match self.arrows {
            DimensionArrows::FilledInside | DimensionArrows::Open => (
                [
                    head(Point::new(line[0].x + ux, line[0].y + uy), line[0]),
                    head(back(line[1], 1.0), line[1]),
                ],
                Vec::new(),
            ),
            DimensionArrows::FilledOutside => {
                let tail = 2.0 * LINE_END_ARROW_LENGTH_MM;
                let out0 = back(line[0], tail);
                let out1 = back(line[1], -tail);
                (
                    [head(out0, line[0]), head(out1, line[1])],
                    vec![[out0, line[0]], [line[1], out1]],
                )
            }
        };
        // The outer side: away from the measured points; up for a zero offset.
        let (nx, ny) = self.normal();
        let side = if self.offset > 0.0 { 1.0 } else { -1.0 };
        let mid = Point::new((line[0].x + line[1].x) / 2.0, (line[0].y + line[1].y) / 2.0);
        let text_at = Point::new(
            mid.x + nx * side * DIMENSION_TEXT_GAP_MM,
            mid.y + ny * side * DIMENSION_TEXT_GAP_MM,
        );
        let text_angle_deg = readable_angle(uy.atan2(ux).to_degrees());
        // The text's up direction after its turn; it must face the outer side.
        let (sin, cos) = text_angle_deg.to_radians().sin_cos();
        let text_hangs = sin * nx * side - cos * ny * side < 0.0;
        DimensionGeometry {
            line,
            extensions,
            arrows,
            outside_tails,
            text_at,
            text_angle_deg,
            text_hangs,
        }
    }

    /// Every point the dimension's drawing reaches, for bounds.
    pub fn outline_points(&self) -> Vec<Point> {
        let g = self.geometry();
        let mut out = vec![self.from, self.to, g.line[0], g.line[1], g.text_at];
        out.extend(g.extensions.iter().flatten().copied());
        out.extend(g.arrows.iter().flatten().copied());
        out.extend(g.outside_tails.iter().flatten().copied());
        let half = self.label().chars().count() as f64 * DIMENSION_TEXT_MM * 0.3;
        let (ax, ay) = (
            g.text_angle_deg.to_radians().cos(),
            g.text_angle_deg.to_radians().sin(),
        );
        out.push(Point::new(g.text_at.x - ax * half, g.text_at.y - ay * half));
        out.push(Point::new(g.text_at.x + ax * half, g.text_at.y + ay * half));
        out
    }

    /// Map the dimension through `f` (move, rotate, mirror). The two points
    /// map directly; a horizontal dimension turned a quarter becomes a
    /// vertical one, and the offset follows the dimension line.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        let line_anchor = self.line()[0];
        let (ax, ay) = self.axis();
        let axis_probe = Point::new(self.from.x + ax, self.from.y + ay);
        let from = f(self.from);
        let to = f(self.to);
        let anchor = f(line_anchor);
        let probe = f(axis_probe);
        self.from = from;
        self.to = to;
        if self.direction != DimensionDirection::Aligned {
            let (px, py) = (probe.x - from.x, probe.y - from.y);
            self.direction = if px.abs() >= py.abs() {
                DimensionDirection::Horizontal
            } else {
                DimensionDirection::Vertical
            };
        }
        let (nx, ny) = self.normal();
        let (ox, oy) = match self.direction {
            DimensionDirection::Horizontal => (0.0, anchor.y - from.y),
            DimensionDirection::Vertical => (anchor.x - from.x, 0.0),
            DimensionDirection::Aligned => (anchor.x - from.x, anchor.y - from.y),
        };
        self.offset = ox * nx + oy * ny;
    }

    /// Distance from `p` to the drawn lines.
    pub fn distance_to(&self, p: Point) -> f64 {
        let g = self.geometry();
        let mut segments = vec![g.line];
        segments.extend(g.extensions);
        segments.extend(g.outside_tails.iter().copied());
        let line = segments
            .iter()
            .map(|s| super::drawing_bezier::segment_distance(p, s[0], s[1]))
            .fold(f64::INFINITY, f64::min);
        let text = (p.x - g.text_at.x).hypot(p.y - g.text_at.y) - DIMENSION_TEXT_MM;
        line.min(text.max(0.0))
    }
}

/// An extension line from near `point` to just past `on_line`.
fn extension(point: Point, on_line: Point) -> [Point; 2] {
    let (dx, dy) = (on_line.x - point.x, on_line.y - point.y);
    let len = dx.hypot(dy);
    if len <= DIMENSION_EXTENSION_GAP_MM {
        return [on_line, on_line];
    }
    let (ux, uy) = (dx / len, dy / len);
    [
        Point::new(
            point.x + ux * DIMENSION_EXTENSION_GAP_MM,
            point.y + uy * DIMENSION_EXTENSION_GAP_MM,
        ),
        Point::new(
            on_line.x + ux * DIMENSION_EXTENSION_OVERSHOOT_MM,
            on_line.y + uy * DIMENSION_EXTENSION_OVERSHOOT_MM,
        ),
    ]
}

/// A dimension arrowhead with its tip at `tip`, pointing away from `from`.
fn head(from: Point, tip: Point) -> [Point; 3] {
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let len = dx.hypot(dy).max(1e-12);
    let (ux, uy) = (dx / len, dy / len);
    let base = Point::new(
        tip.x - ux * LINE_END_ARROW_LENGTH_MM,
        tip.y - uy * LINE_END_ARROW_LENGTH_MM,
    );
    let (nx, ny) = (
        -uy * LINE_END_ARROW_HALF_WIDTH_MM,
        ux * LINE_END_ARROW_HALF_WIDTH_MM,
    );
    [
        tip,
        Point::new(base.x + nx, base.y + ny),
        Point::new(base.x - nx, base.y - ny),
    ]
}

/// `degrees` turned by half a turn when needed so text never reads upside
/// down: the result is in `(-90, 90]`.
fn readable_angle(degrees: f64) -> f64 {
    let mut a = degrees.rem_euclid(360.0);
    if a > 180.0 {
        a -= 360.0;
    }
    if a >= 90.0 {
        a -= 180.0;
    } else if a < -90.0 {
        a += 180.0;
    }
    a
}
