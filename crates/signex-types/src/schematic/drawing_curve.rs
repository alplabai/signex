//! An ellipse, elliptical arc or round rectangle drawn on a schematic sheet
//! (#1267). Annotation only: it never joins a net and ERC never reads it.
//! The geometry is the shared [`CurveShape`], the one the symbol body uses.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CurveShape, FillType, Point, StrokeColor};

/// One curve or shape drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchCurve {
    pub uuid: Uuid,
    pub shape: CurveShape,
    /// Stroke width in mm; `0` draws the sheet's default drawing width.
    #[serde(default)]
    pub width: f64,
    /// Ignored for an open elliptical arc.
    #[serde(default)]
    pub fill: FillType,
    /// `None` draws in the sheet's drawing colour.
    #[serde(default)]
    pub stroke_color: Option<StrokeColor>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl SchCurve {
    /// An unfilled shape in the sheet's drawing colour at `width` mm.
    pub fn new(uuid: Uuid, shape: CurveShape, width: f64) -> Self {
        Self {
            uuid,
            shape,
            width,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        }
    }

    /// The fill the shape draws with: none for an open arc.
    pub fn drawn_fill(&self) -> FillType {
        if self.shape.is_closed() {
            self.fill
        } else {
            FillType::None
        }
    }

    /// Map the shape through `f` (move, rotate, mirror).
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        self.shape.map_points(f);
    }

    /// The outline; see [`CurveShape::flatten`].
    pub fn flatten(&self) -> Vec<Point> {
        self.shape.flatten()
    }

    /// The smallest box holding the outline.
    pub fn bounds(&self) -> (Point, Point) {
        self.shape.bounds()
    }

    /// The outline's segments, the closing one included for a closed
    /// shape.
    pub fn segments(&self) -> Vec<[Point; 2]> {
        let ring = self.flatten();
        let mut out: Vec<[Point; 2]> = ring.windows(2).map(|w| [w[0], w[1]]).collect();
        if self.shape.is_closed()
            && let (Some(first), Some(last)) = (ring.first(), ring.last())
            && ring.len() > 2
        {
            out.push([*last, *first]);
        }
        out
    }
}
