//! Analytic partial derivatives of every constraint residual.
//!
//! Each residual row depends on a handful of point coordinates (and, for a
//! circle, its radius), so its gradient has a handful of non-zero entries.
//! This module derives them by hand, kind by kind, in the same order as
//! [`crate::solver::residual::residual`]; a row is a list of `(column,
//! value)` pairs over only the state variables the constraint touches. A
//! point that is fixed or frozen has no column and contributes nothing.
//! Repeated columns (a point named twice by one constraint) are legal and
//! add up when the rows are assembled.
//!
//! The formulas are the plain chain rule on the residuals in
//! `solver::residuals`; `derivatives_tests` checks each against central
//! differences.

use crate::constraint::{Constraint, ConstraintKind};
use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::math::{Vec2, add, cross, dot, norm, scale, sub};
use crate::solver::residual::{ResolvedParams, resolve_dim_cached};
use crate::solver::state::{EntityIndex, arc_refs, find_entity, line_endpoints, point_xy};

/// One Jacobian row: the non-zero `(state column, d row / d column)` entries.
pub type SparseRow = Vec<(usize, f64)>;

/// Same cut-off as the `point_on` residuals: a shorter line has no direction.
const DEGENERATE_LEN_EPS: f64 = 1e-12;

/// A point's position and, if the solve can move it, its state columns.
struct Pt {
    at: Vec2,
    cols: Option<(usize, usize)>,
}

/// Add `g` (d row / d point) to `row`.
fn put(row: &mut SparseRow, p: &Pt, g: Vec2) {
    if let Some((xi, yi)) = p.cols {
        row.push((xi, g.0));
        row.push((yi, g.1));
    }
}

/// Gradient of `|a - b|` with respect to `a` (that of `b` is the negation).
/// Zero at `a == b`, where the norm has no gradient.
fn dist_grad(a: Vec2, b: Vec2) -> Vec2 {
    let d = sub(a, b);
    let n = norm(d);
    if n == 0.0 {
        (0.0, 0.0)
    } else {
        (d.0 / n, d.1 / n)
    }
}

/// `-1`, `0` or `1`; zero at zero, as a central difference of `|x|` reads.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `f = cross(p - a, b - a) / |b - a|`: its value and gradients with respect
/// to `p`, `a`, `b`. `None` when `|b - a|` is zero or under `min_len`.
fn perp(p: Vec2, a: Vec2, b: Vec2, min_len: f64) -> Option<(f64, Vec2, Vec2, Vec2)> {
    let d = sub(b, a);
    let len = norm(d);
    if len < min_len || len == 0.0 {
        return None;
    }
    let w = sub(p, a);
    let f = cross(w, d) / len;
    let gp = (d.1 / len, -d.0 / len);
    let gb = (
        -w.1 / len - f * d.0 / (len * len),
        w.0 / len - f * d.1 / (len * len),
    );
    let ga = (-gp.0 - gb.0, -gp.1 - gb.1);
    Some((f, gp, ga, gb))
}

/// A circle (explicit radius column) or an arc (radius `|start - centre|`).
enum Round {
    Circle {
        center: Pt,
        radius_col: usize,
        r: f64,
    },
    Arc {
        center: Pt,
        start: Pt,
    },
}

impl Round {
    fn center(&self) -> &Pt {
        match self {
            Round::Circle { center, .. } | Round::Arc { center, .. } => center,
        }
    }

    fn radius(&self) -> f64 {
        match self {
            Round::Circle { r, .. } => *r,
            Round::Arc { center, start } => norm(sub(start.at, center.at)),
        }
    }

    /// Add `k * d radius` to `row`.
    fn put_radius(&self, row: &mut SparseRow, k: f64) {
        match self {
            Round::Circle { radius_col, .. } => row.push((*radius_col, k)),
            Round::Arc { center, start } => {
                let g = dist_grad(start.at, center.at);
                put(row, start, scale(k, g));
                put(row, center, scale(-k, g));
            }
        }
    }
}

struct Cx<'a> {
    state: &'a [f64],
    index: &'a EntityIndex,
    sketch: &'a SketchData,
}

impl Cx<'_> {
    fn pt(&self, id: SketchEntityId) -> Result<Pt, SketchError> {
        let at = point_xy(id, self.state, self.index, self.sketch)
            .ok_or(SketchError::EntityNotFound(id))?;
        let cols = if self.index.fixed.contains(&id) || self.index.frozen.contains(&id) {
            None
        } else {
            self.index.points.get(&id).copied()
        };
        Ok(Pt { at, cols })
    }

    fn line(&self, id: SketchEntityId) -> Result<(Pt, Pt), SketchError> {
        let (s, e) =
            line_endpoints(id, self.index, self.sketch).ok_or(SketchError::EntityNotFound(id))?;
        Ok((self.pt(s)?, self.pt(e)?))
    }

    fn round(&self, id: SketchEntityId) -> Result<Round, SketchError> {
        let entity =
            find_entity(id, self.index, self.sketch).ok_or(SketchError::EntityNotFound(id))?;
        match entity.kind {
            EntityKind::Circle { center, .. } => {
                let radius_col = *self
                    .index
                    .radii
                    .get(&id)
                    .ok_or(SketchError::EntityNotFound(id))?;
                Ok(Round::Circle {
                    center: self.pt(center)?,
                    radius_col,
                    r: self.state[radius_col],
                })
            }
            EntityKind::Arc { center, start, .. } => Ok(Round::Arc {
                center: self.pt(center)?,
                start: self.pt(start)?,
            }),
            _ => Err(SketchError::EntityNotFound(id)),
        }
    }
}

/// Jacobian rows of one constraint, in the order and count of its residual.
pub fn constraint_rows(
    c: &Constraint,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
    params: &ResolvedParams,
) -> Result<Vec<SparseRow>, SketchError> {
    use ConstraintKind::*;
    let cx = Cx {
        state,
        index,
        sketch,
    };
    // A target that cannot be resolved fails the row, as it does the residual.
    if let Some(target) = c.kind.dim_target() {
        resolve_dim_cached(target, params, index)?;
    }
    Ok(match &c.kind {
        Coincident { p1, p2 } => {
            let (a, b) = (cx.pt(*p1)?, cx.pt(*p2)?);
            let (mut rx, mut ry) = (vec![], vec![]);
            put(&mut rx, &a, (-1.0, 0.0));
            put(&mut rx, &b, (1.0, 0.0));
            put(&mut ry, &a, (0.0, -1.0));
            put(&mut ry, &b, (0.0, 1.0));
            vec![rx, ry]
        }
        DistancePtPt { p1, p2, .. } => {
            let (a, b) = (cx.pt(*p1)?, cx.pt(*p2)?);
            let g = dist_grad(b.at, a.at);
            let mut r = vec![];
            put(&mut r, &b, g);
            put(&mut r, &a, scale(-1.0, g));
            vec![r]
        }
        Horizontal { line } | Vertical { line } => {
            let (s, e) = cx.line(*line)?;
            let g = if matches!(c.kind, Horizontal { .. }) {
                (0.0, 1.0)
            } else {
                (1.0, 0.0)
            };
            let mut r = vec![];
            put(&mut r, &e, g);
            put(&mut r, &s, scale(-1.0, g));
            vec![r]
        }
        Fixed { .. } => vec![],
        Parallel { l1, l2 } | Perpendicular { l1, l2 } | Angle { l1, l2, .. } => {
            let (a, b) = (cx.line(*l1)?, cx.line(*l2)?);
            vec![direction_pair(&c.kind, &a, &b)]
        }
        PointOnLine { point, line } | DistancePtLine { point, line, .. } => {
            let p = cx.pt(*point)?;
            let (a, b) = cx.line(*line)?;
            let (_, gp, ga, gb) = perp(p.at, a.at, b.at, DEGENERATE_LEN_EPS)
                .ok_or(SketchError::DegenerateLine(*line))?;
            let mut r = vec![];
            put(&mut r, &p, gp);
            put(&mut r, &a, ga);
            put(&mut r, &b, gb);
            vec![r]
        }
        PointOnArc { point, arc } => {
            let (center, start, _, _) =
                arc_refs(*arc, index, sketch).ok_or(SketchError::EntityNotFound(*arc))?;
            let (p, c0, s) = (cx.pt(*point)?, cx.pt(center)?, cx.pt(start)?);
            let (gp, gs) = (dist_grad(p.at, c0.at), dist_grad(s.at, c0.at));
            let mut r = vec![];
            put(&mut r, &p, gp);
            put(&mut r, &s, scale(-1.0, gs));
            put(&mut r, &c0, sub(gs, gp));
            vec![r]
        }
        DistancePtCircle { point, circle, .. } => {
            let p = cx.pt(*point)?;
            let round = cx.round(*circle)?;
            let g = dist_grad(p.at, round.center().at);
            let mut r = vec![];
            put(&mut r, &p, g);
            put(&mut r, round.center(), scale(-1.0, g));
            round.put_radius(&mut r, -1.0);
            vec![r]
        }
        EqualLength { l1, l2 } => {
            let mut r = vec![];
            for (line, k) in [(*l1, -1.0), (*l2, 1.0)] {
                let (s, e) = cx.line(line)?;
                let g = scale(k, dist_grad(e.at, s.at));
                put(&mut r, &e, g);
                put(&mut r, &s, scale(-1.0, g));
            }
            vec![r]
        }
        EqualRadius { e1, e2 } => {
            let mut r = vec![];
            cx.round(*e1)?.put_radius(&mut r, -1.0);
            cx.round(*e2)?.put_radius(&mut r, 1.0);
            vec![r]
        }
        TangentLineArc { line, arc } => {
            let (s, e) = cx.line(*line)?;
            let round = cx.round(*arc)?;
            let mut r = vec![];
            // A line shorter than the cut-off has signed distance 0, a constant.
            if let Some((f, gc, gs, ge)) = perp(round.center().at, s.at, e.at, DEGENERATE_LEN_EPS) {
                let k = sign(f);
                put(&mut r, round.center(), scale(k, gc));
                put(&mut r, &s, scale(k, gs));
                put(&mut r, &e, scale(k, ge));
            }
            round.put_radius(&mut r, -1.0);
            vec![r]
        }
        TangentArcArc { a1, a2, internal } => {
            let (r1, r2) = (cx.round(*a1)?, cx.round(*a2)?);
            let g = dist_grad(r2.center().at, r1.center().at);
            let mut r = vec![];
            put(&mut r, r2.center(), g);
            put(&mut r, r1.center(), scale(-1.0, g));
            if *internal {
                // minus |r1 - r2|
                let k = sign(r1.radius() - r2.radius());
                r1.put_radius(&mut r, -k);
                r2.put_radius(&mut r, k);
            } else {
                // minus (r1 + r2)
                r1.put_radius(&mut r, -1.0);
                r2.put_radius(&mut r, -1.0);
            }
            vec![r]
        }
        SymmetricAboutLine { p1, p2, line } => {
            let (u, v) = (cx.pt(*p1)?, cx.pt(*p2)?);
            let (a, b) = cx.line(*line)?;
            let m = scale(0.5, add(u.at, v.at));
            // Zero-length line: the raw cross, as the residual does.
            let (gm, ga, gb) = match perp(m, a.at, b.at, 0.0) {
                Some((_, gm, ga, gb)) => (gm, ga, gb),
                None => {
                    let (d, w) = (sub(b.at, a.at), sub(m, a.at));
                    let (gm, gb) = ((d.1, -d.0), (-w.1, w.0));
                    (gm, (-gm.0 - gb.0, -gm.1 - gb.1), gb)
                }
            };
            let (mut mid, mut perp_row) = (vec![], vec![]);
            put(&mut mid, &u, scale(0.5, gm));
            put(&mut mid, &v, scale(0.5, gm));
            put(&mut mid, &a, ga);
            put(&mut mid, &b, gb);
            // dot(p2 - p1, b - a)
            let (d, q) = (sub(b.at, a.at), sub(v.at, u.at));
            put(&mut perp_row, &v, d);
            put(&mut perp_row, &u, scale(-1.0, d));
            put(&mut perp_row, &b, q);
            put(&mut perp_row, &a, scale(-1.0, q));
            vec![mid, perp_row]
        }
        SymmetricAboutPoint { p1, p2, center } => {
            let (u, v, o) = (cx.pt(*p1)?, cx.pt(*p2)?, cx.pt(*center)?);
            axis_rows(|r, g| {
                put(r, &u, scale(0.5, g));
                put(r, &v, scale(0.5, g));
                put(r, &o, scale(-1.0, g));
            })
        }
        Midpoint { point, line } => {
            let p = cx.pt(*point)?;
            let (a, b) = cx.line(*line)?;
            axis_rows(|r, g| {
                put(r, &p, g);
                put(r, &a, scale(-0.5, g));
                put(r, &b, scale(-0.5, g));
            })
        }
    })
}

/// The x row and y row of a residual that is linear per axis: `fill` gets a
/// unit vector along the axis and adds that row's point gradients.
fn axis_rows(fill: impl Fn(&mut SparseRow, Vec2)) -> Vec<SparseRow> {
    let (mut rx, mut ry) = (vec![], vec![]);
    fill(&mut rx, (1.0, 0.0));
    fill(&mut ry, (0.0, 1.0));
    vec![rx, ry]
}

/// Row of Parallel (`d1 x d2`), Perpendicular (`d1 . d2`) or Angle
/// (`atan2(d1 x d2, d1 . d2)`; the wrap to `(-pi, pi]` has slope 1) over the
/// directions `d = end - start` of two lines.
fn direction_pair(kind: &ConstraintKind, l1: &(Pt, Pt), l2: &(Pt, Pt)) -> SparseRow {
    let d1 = sub(l1.1.at, l1.0.at);
    let d2 = sub(l2.1.at, l2.0.at);
    let (g1, g2) = match kind {
        ConstraintKind::Parallel { .. } => ((d2.1, -d2.0), (-d1.1, d1.0)),
        ConstraintKind::Perpendicular { .. } => (d2, d1),
        _ => {
            let (cr, dt) = (cross(d1, d2), dot(d1, d2));
            let q = cr * cr + dt * dt;
            if q == 0.0 {
                ((0.0, 0.0), (0.0, 0.0))
            } else {
                let g = |dcr: Vec2, ddt: Vec2| {
                    ((dt * dcr.0 - cr * ddt.0) / q, (dt * dcr.1 - cr * ddt.1) / q)
                };
                (g((d2.1, -d2.0), d2), g((-d1.1, d1.0), d1))
            }
        }
    };
    let mut r = vec![];
    put(&mut r, &l1.1, g1);
    put(&mut r, &l1.0, scale(-1.0, g1));
    put(&mut r, &l2.1, g2);
    put(&mut r, &l2.0, scale(-1.0, g2));
    r
}

/// Jacobian rows of every arc's implicit closure `|end - c| - |start - c|`,
/// in entity order. A frozen arc has an empty row, as its residual is 0.
pub fn arc_closure_rows(
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<SparseRow>, SketchError> {
    let cx = Cx {
        state,
        index,
        sketch,
    };
    let mut rows = Vec::new();
    for entity in &sketch.entities {
        if !matches!(entity.kind, EntityKind::Arc { .. }) {
            continue;
        }
        let (center, start, end, _) =
            arc_refs(entity.id, index, sketch).ok_or(SketchError::EntityNotFound(entity.id))?;
        let mut row = vec![];
        if ![center, start, end]
            .iter()
            .all(|p| index.frozen.contains(p))
        {
            let (c, s, e) = (cx.pt(center)?, cx.pt(start)?, cx.pt(end)?);
            let (ge, gs) = (dist_grad(e.at, c.at), dist_grad(s.at, c.at));
            put(&mut row, &e, ge);
            put(&mut row, &s, scale(-1.0, gs));
            put(&mut row, &c, sub(gs, ge));
        }
        rows.push(row);
    }
    Ok(rows)
}
