//! The parameters of a trapezoid or a custom pad (#2242) and the rules they
//! keep. The file reader and writer, the board engine and the pad geometry
//! (`pcb_geom::pad_shape`) all ask [`pad_shape_fault`], so a pad one of
//! them accepts, every one of them accepts.
//!
//! Rules are judged on the whole nm a file stores, so a pad that passes
//! here re-opens after a save.

use crate::format::mm_as_stored_nm;
use crate::pcb::{Pad, PadShape, Point};

/// Why a pad's shape parameters cannot make copper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadShapeFault {
    /// A trapezoid delta that is not a finite length.
    DeltaNotStorable,
    /// No delta on either axis: such a pad is a rectangle, and a
    /// `trapezoid` row without one (as files before 1.3 wrote it) must not
    /// load as one silently.
    DeltaMissing,
    /// Both deltas set: a trapezoid narrows along one axis only.
    DeltaOnBothAxes,
    /// A delta that would shrink the narrow edge to nothing or past it.
    DeltaTooWide,
    /// A custom corner that is not a finite length.
    OutlineNotStorable,
    /// A custom outline of fewer than 3 corners.
    OutlineTooFewCorners,
    /// A custom outline whose edges cross or touch away from a shared corner.
    OutlineCrossesItself,
    /// A custom outline with no area (every corner on one line).
    OutlineHasNoArea,
}

impl PadShapeFault {
    /// The pad field the fault is in, as the file names it.
    pub fn field(self) -> &'static str {
        match self {
            PadShapeFault::DeltaNotStorable
            | PadShapeFault::DeltaMissing
            | PadShapeFault::DeltaOnBothAxes
            | PadShapeFault::DeltaTooWide => "trapezoid_delta",
            PadShapeFault::OutlineNotStorable
            | PadShapeFault::OutlineTooFewCorners
            | PadShapeFault::OutlineCrossesItself
            | PadShapeFault::OutlineHasNoArea => "custom_outline",
        }
    }
}

impl std::fmt::Display for PadShapeFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PadShapeFault::DeltaNotStorable => "a trapezoid delta must be a finite length",
            PadShapeFault::DeltaMissing => {
                "a trapezoid needs a delta on one axis; with none it is a rect"
            }
            PadShapeFault::DeltaOnBothAxes => {
                "a trapezoid narrows along one axis: delta x or delta y must be 0"
            }
            PadShapeFault::DeltaTooWide => {
                "a trapezoid delta must be less than twice the side it narrows"
            }
            PadShapeFault::OutlineNotStorable => {
                "every custom pad corner must be a finite position"
            }
            PadShapeFault::OutlineTooFewCorners => "a custom pad outline needs at least 3 corners",
            PadShapeFault::OutlineCrossesItself => "a custom pad outline must not cross itself",
            PadShapeFault::OutlineHasNoArea => "a custom pad outline must enclose an area",
        })
    }
}

impl std::error::Error for PadShapeFault {}

/// What is wrong with the parameters of `pad`'s own shape; `None` when they
/// make copper. Only the shape in use is judged: a delta kept on a pad that
/// is now a rectangle is not. A trapezoid's delta must fit its own size and
/// every per-layer size, since each layer's copper is built from its own.
pub fn pad_shape_fault(pad: &Pad) -> Option<PadShapeFault> {
    match pad.shape {
        PadShape::Trapezoid => std::iter::once(pad.size)
            .chain(pad.layer_sizes.iter().flat_map(|sizes| sizes.sizes()))
            .find_map(|size| trapezoid_corners(size, pad.trapezoid_delta).err()),
        PadShape::Custom => outline_fault(&pad.custom_outline),
        PadShape::Circle | PadShape::Rect | PadShape::Oval | PadShape::RoundRect => None,
    }
}

/// The corners of a trapezoid pad in its own frame, mm, counter-clockwise
/// from the −x −y corner. `size` is the pad's box measured through its
/// middle: with `delta.x` the +y edge is `size.x + delta.x / 2` long and the
/// −y edge `size.x − delta.x / 2`; `delta.y` does the same for the +x and −x
/// edges along y.
pub fn trapezoid_corners(size: Point, delta: Point) -> Result<[Point; 4], PadShapeFault> {
    let (Some(dx), Some(dy)) = (mm_as_stored_nm(delta.x), mm_as_stored_nm(delta.y)) else {
        return Err(PadShapeFault::DeltaNotStorable);
    };
    if dx == 0 && dy == 0 {
        return Err(PadShapeFault::DeltaMissing);
    }
    if dx != 0 && dy != 0 {
        return Err(PadShapeFault::DeltaOnBothAxes);
    }
    let fits = |d: i64, side: f64| {
        d == 0
            || mm_as_stored_nm(side).is_some_and(|s| i128::from(d).abs() < 2 * i128::from(s).abs())
    };
    if !fits(dx, size.x) || !fits(dy, size.y) {
        return Err(PadShapeFault::DeltaTooWide);
    }
    let (hw, hh) = (size.x.abs() / 2.0, size.y.abs() / 2.0);
    let (qx, qy) = (delta.x / 4.0, delta.y / 4.0);
    Ok([
        Point::new(-(hw - qx), -(hh - qy)),
        Point::new(hw - qx, -(hh + qy)),
        Point::new(hw + qx, hh + qy),
        Point::new(-(hw + qx), hh - qy),
    ])
}

/// What is wrong with a custom outline; `None` for a simple polygon with
/// area, of either winding.
pub fn outline_fault(outline: &[Point]) -> Option<PadShapeFault> {
    let mut corners = Vec::with_capacity(outline.len());
    for p in outline {
        match (mm_as_stored_nm(p.x), mm_as_stored_nm(p.y)) {
            (Some(x), Some(y)) => corners.push((i128::from(x), i128::from(y))),
            _ => return Some(PadShapeFault::OutlineNotStorable),
        }
    }
    let n = corners.len();
    if n < 3 {
        return Some(PadShapeFault::OutlineTooFewCorners);
    }
    let twice_area: i128 = (0..n)
        .map(|i| cross(corners[i], corners[(i + 1) % n]))
        .sum();
    if twice_area == 0 {
        return Some(PadShapeFault::OutlineHasNoArea);
    }
    let edge = |i: usize| (corners[i], corners[(i + 1) % n]);
    for i in 0..n {
        let (a, b) = edge(i);
        if a == b {
            return Some(PadShapeFault::OutlineCrossesItself);
        }
        for j in i + 1..n {
            let (c, d) = edge(j);
            let neighbours = j == i + 1 || (i == 0 && j == n - 1);
            let meets = if neighbours {
                // Neighbours share one corner; they may not fold back
                // over each other.
                if j == i + 1 {
                    folds_back(b, a, d)
                } else {
                    folds_back(a, b, c)
                }
            } else {
                segments_meet(a, b, c, d)
            };
            if meets {
                return Some(PadShapeFault::OutlineCrossesItself);
            }
        }
    }
    None
}

type Nm = (i128, i128);

fn cross(a: Nm, b: Nm) -> i128 {
    a.0 * b.1 - a.1 * b.0
}

fn turn(a: Nm, b: Nm, c: Nm) -> i128 {
    cross((b.0 - a.0, b.1 - a.1), (c.0 - a.0, c.1 - a.1)).signum()
}

/// Whether `p` lies in the box of `a`–`b` (on the segment, when collinear).
fn in_box(a: Nm, b: Nm, p: Nm) -> bool {
    a.0.min(b.0) <= p.0 && p.0 <= a.0.max(b.0) && a.1.min(b.1) <= p.1 && p.1 <= a.1.max(b.1)
}

/// Whether closed segments `a`–`b` and `c`–`d` share a point.
fn segments_meet(a: Nm, b: Nm, c: Nm, d: Nm) -> bool {
    let (d1, d2) = (turn(c, d, a), turn(c, d, b));
    let (d3, d4) = (turn(a, b, c), turn(a, b, d));
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && in_box(c, d, a))
        || (d2 == 0 && in_box(c, d, b))
        || (d3 == 0 && in_box(a, b, c))
        || (d4 == 0 && in_box(a, b, d))
}

/// Whether two edges leaving `shared` towards `a` and `b` overlap: they lie
/// on one line and point the same way.
fn folds_back(shared: Nm, a: Nm, b: Nm) -> bool {
    let (u, v) = (
        (a.0 - shared.0, a.1 - shared.1),
        (b.0 - shared.0, b.1 - shared.1),
    );
    cross(u, v) == 0 && u.0 * v.0 + u.1 * v.1 > 0
}

#[cfg(test)]
#[path = "pcb_pad_shape_tests.rs"]
mod tests;
