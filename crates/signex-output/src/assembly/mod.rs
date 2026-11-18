//! Assembly drawings (#2486): one builder makes the top or the bottom
//! drawing of a board from that side's Fab, Assembly, Designator and
//! Fiducial layers, the board outline and the Dimensions layer. The board
//! PDF draws it as a page (`pdf::board`) and `svg::board` writes it as an
//! SVG file. Rules: `docs/BOARD_OUTPUT.md`, "Assembly drawings".

mod collect;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{BoardSide, PcbLayer, TechnicalLayer};
use signex_types::pcb_origin::EMPTY_BOARD_AREA;

/// What a mark of an assembly drawing shows; each kind has its own ink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AssemblyInk {
    /// The board outline and the Edge Cuts drawings.
    Outline,
    /// The Dimensions layer.
    Dimension,
    /// The side's Fab layer: the part bodies.
    Fab,
    /// The side's Assembly layer.
    Assembly,
    /// The side's Fiducial layer.
    Fiducial,
    /// The side's Designator layer and each part's designator.
    Designator,
}

impl AssemblyInk {
    /// The ink on white paper; black in monochrome.
    pub const fn rgb(self, monochrome: bool) -> (f32, f32, f32) {
        if monochrome {
            return (0.0, 0.0, 0.0);
        }
        match self {
            Self::Outline => (0.0, 0.0, 0.0),
            Self::Dimension => (0.50, 0.10, 0.55),
            Self::Fab => (0.35, 0.35, 0.35),
            Self::Assembly => (0.12, 0.27, 0.78),
            Self::Fiducial => (0.10, 0.52, 0.22),
            Self::Designator => (0.78, 0.12, 0.12),
        }
    }
}

/// One mark of a drawing, in board mm (y down).
#[derive(Debug, Clone, PartialEq)]
pub enum AssemblyMark {
    /// A line through `points`, closed back to the first if `closed`.
    Stroke {
        points: Vec<Point>,
        closed: bool,
        /// As saved; a renderer draws a hairline for 0.
        width_mm: f64,
        ink: AssemblyInk,
    },
    /// An even-odd area: an outer ring and its holes.
    Fill {
        rings: Vec<Vec<Point>>,
        ink: AssemblyInk,
    },
    /// A text centred on `at`, `size_mm` high, its baseline turned by
    /// `rotation_deg` in the sense a pad turns.
    Text {
        at: Point,
        rotation_deg: f64,
        size_mm: f64,
        text: String,
        ink: AssemblyInk,
    },
}

/// The top or bottom assembly drawing of a board.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyDrawing {
    pub side: BoardSide,
    /// The extent the drawing is framed on, board mm: the outline's, else
    /// the marks', else the default board area.
    pub min: Point,
    pub max: Point,
    /// In drawing order: dimensions, fab, assembly, fiducial, designators,
    /// then the outline on top.
    pub marks: Vec<AssemblyMark>,
}

/// The ink `layer` is drawn in on the `side` drawing; `None` when the
/// drawing leaves it out.
pub fn assembly_ink(side: BoardSide, layer: PcbLayer) -> Option<AssemblyInk> {
    let PcbLayer::Technical(layer) = layer else {
        return None;
    };
    use TechnicalLayer as T;
    let ink = match layer {
        T::EdgeCuts => return Some(AssemblyInk::Outline),
        T::Dimensions => return Some(AssemblyInk::Dimension),
        T::FabTop | T::FabBottom => AssemblyInk::Fab,
        T::AssemblyTop | T::AssemblyBottom => AssemblyInk::Assembly,
        T::FiducialTop | T::FiducialBottom => AssemblyInk::Fiducial,
        T::DesignatorTop | T::DesignatorBottom => AssemblyInk::Designator,
        _ => return None,
    };
    (layer.side() == Some(side)).then_some(ink)
}

/// The ink of the layer named `layer` on the `side` drawing.
fn ink_of(side: BoardSide, layer: &str) -> Option<AssemblyInk> {
    parse_pcb_layer(layer).and_then(|l| assembly_ink(side, l))
}

/// The `side` assembly drawing of `board`.
pub fn assembly_drawing(board: &PcbBoard, side: BoardSide) -> AssemblyDrawing {
    let mut marks = Vec::new();
    for ink in [
        AssemblyInk::Dimension,
        AssemblyInk::Fab,
        AssemblyInk::Assembly,
        AssemblyInk::Fiducial,
        AssemblyInk::Designator,
    ] {
        collect::layer_marks(board, side, ink, &mut marks);
    }
    collect::designators(board, side, &mut marks);
    collect::outline(board, side, &mut marks);
    let (min, max) = extent(&marks);
    AssemblyDrawing {
        side,
        min,
        max,
        marks,
    }
}

/// The outline marks' extent, else every mark's, else the default area.
fn extent(marks: &[AssemblyMark]) -> (Point, Point) {
    let outline = marks.iter().filter(|m| mark_ink(m) == AssemblyInk::Outline);
    bounds(outline.flat_map(mark_points))
        .or_else(|| bounds(marks.iter().flat_map(mark_points)))
        .unwrap_or(EMPTY_BOARD_AREA)
}

fn mark_ink(mark: &AssemblyMark) -> AssemblyInk {
    match mark {
        AssemblyMark::Stroke { ink, .. }
        | AssemblyMark::Fill { ink, .. }
        | AssemblyMark::Text { ink, .. } => *ink,
    }
}

fn mark_points(mark: &AssemblyMark) -> Vec<Point> {
    match mark {
        AssemblyMark::Stroke { points, .. } => points.clone(),
        AssemblyMark::Fill { rings, .. } => rings.iter().flatten().copied().collect(),
        AssemblyMark::Text { at, .. } => vec![*at],
    }
}

/// The corners of the box around `points`; `None` without a finite point
/// or with a box of no area.
fn bounds(points: impl Iterator<Item = Point>) -> Option<(Point, Point)> {
    let (min, max) = points.filter(|p| p.x.is_finite() && p.y.is_finite()).fold(
        None,
        |acc: Option<(Point, Point)>, p| {
            Some(match acc {
                None => (p, p),
                Some((min, max)) => (
                    Point::new(min.x.min(p.x), min.y.min(p.y)),
                    Point::new(max.x.max(p.x), max.y.max(p.y)),
                ),
            })
        },
    )?;
    (max.x > min.x && max.y > min.y).then_some((min, max))
}

#[cfg(test)]
pub(crate) mod tests;
