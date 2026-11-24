//! The free motions a solve leaves (mockup row 179): the null space of the
//! constraint Jacobian at the solved state, one basis motion per free state
//! variable, read as the points and radii each one moves. The Properties DOF
//! card counts them and lists each with a Show.
//!
//! The basis comes from the reduced row echelon form of the Jacobian with
//! partial pivoting: each free column gives the motion that moves that
//! variable by one and the pivot variables with it. The motions are
//! linearised at the solved state, as the canvas's DOF arrows are. Derived
//! from first principles; no third-party numerical source consulted.

use std::collections::HashSet;

use crate::id::SketchEntityId;
use crate::solver::dof::RANK_TOL;
use crate::solver::lm::SolveResult;

/// A component of a basis motion at or below this moves nothing.
const MOVE_EPS: f64 = 1e-9;

/// How a free motion moves what it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionKind {
    /// Every point it moves goes the same way, along the unit direction
    /// `(dx, dy)`, and no radius changes.
    Slide { dx: f64, dy: f64 },
    /// Its points move together in any direction of the plane: two free
    /// slides of the same points.
    AnyDirection,
    /// A turn, a stretch or a radius change.
    Reshape,
}

/// One free motion of a solved sketch.
#[derive(Debug, Clone, PartialEq)]
pub struct FreeMotion {
    /// The entities it moves: points, and circles or arcs whose radius
    /// changes, in state order.
    pub entities: Vec<SketchEntityId>,
    pub kind: MotionKind,
    /// The degrees of freedom it stands for: 2 for [`MotionKind::AnyDirection`],
    /// else 1.
    pub dof: usize,
}

/// The free degrees of freedom of a solve and the motions they are.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FreeMotions {
    /// The sum of the motions' `dof`.
    pub dof: usize,
    pub motions: Vec<FreeMotion>,
}

/// What one state variable is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Radius,
}

/// The free motions of `result`, from `jacobian` taken at its state. Points
/// no residual reads ([`crate::solver::state::EntityIndex::frozen`]) are
/// free in any direction and come last, in sketch order.
pub fn free_motions(result: &SolveResult, jacobian: &[Vec<f64>]) -> FreeMotions {
    let n = result.state.len();
    let owners = owners(result);
    let mut motions: Vec<FreeMotion> = null_space(jacobian, n)
        .iter()
        .filter_map(|v| motion(v, &owners))
        .collect();
    motions = merge_plane_slides(motions);
    let mut frozen: Vec<SketchEntityId> = result.index.frozen.iter().copied().collect();
    frozen.sort_by_key(|id| result.index.slots.get(id).copied().unwrap_or(usize::MAX));
    motions.extend(frozen.into_iter().map(|id| FreeMotion {
        entities: vec![id],
        kind: MotionKind::AnyDirection,
        dof: 2,
    }));
    FreeMotions {
        dof: motions.iter().map(|m| m.dof).sum(),
        motions,
    }
}

/// The entity and axis of each state variable.
fn owners(result: &SolveResult) -> Vec<Option<(SketchEntityId, Axis)>> {
    let mut owners = vec![None; result.state.len()];
    let mut set = |column: usize, owner| {
        if let Some(slot) = owners.get_mut(column) {
            *slot = Some(owner);
        }
    };
    for (&id, &(x, y)) in &result.index.points {
        set(x, (id, Axis::X));
        set(y, (id, Axis::Y));
    }
    for (&id, &column) in &result.index.radii {
        set(column, (id, Axis::Radius));
    }
    owners
}

/// One null-space vector of the `m × n` `jacobian` per free column, from
/// its reduced row echelon form. A Jacobian without rows leaves every
/// column free; a row of another width is left out.
fn null_space(jacobian: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
    let mut rows: Vec<Vec<f64>> = jacobian.iter().filter(|r| r.len() == n).cloned().collect();
    let scale = rows
        .iter()
        .flatten()
        .fold(1.0_f64, |max, v| max.max(v.abs()));
    let tol = RANK_TOL * scale;
    let mut pivots: Vec<usize> = Vec::new();
    for column in 0..n {
        let r = pivots.len();
        if r == rows.len() {
            break;
        }
        let best = (r..rows.len())
            .max_by(|&a, &b| rows[a][column].abs().total_cmp(&rows[b][column].abs()));
        let Some(best) = best.filter(|&p| rows[p][column].abs() > tol) else {
            continue;
        };
        rows.swap(r, best);
        let lead = rows[r][column];
        rows[r].iter_mut().for_each(|v| *v /= lead);
        let pivot = rows[r].clone();
        for (i, row) in rows.iter_mut().enumerate() {
            let factor = row[column];
            if i == r || factor == 0.0 {
                continue;
            }
            row.iter_mut()
                .zip(&pivot)
                .for_each(|(v, p)| *v -= factor * p);
        }
        pivots.push(column);
    }
    let pivot_set: HashSet<usize> = pivots.iter().copied().collect();
    (0..n)
        .filter(|column| !pivot_set.contains(column))
        .map(|free| {
            let mut v = vec![0.0; n];
            v[free] = 1.0;
            for (row, &column) in pivots.iter().enumerate() {
                v[column] = -rows[row][free];
            }
            v
        })
        .collect()
}

/// What the basis motion `v` moves and how; `None` when it moves no entity.
fn motion(v: &[f64], owners: &[Option<(SketchEntityId, Axis)>]) -> Option<FreeMotion> {
    let mut entities: Vec<SketchEntityId> = Vec::new();
    let mut seen: HashSet<SketchEntityId> = HashSet::new();
    // Each moved point's (dx, dy), in the order of `entities`.
    let mut moves: Vec<(SketchEntityId, f64, f64)> = Vec::new();
    let mut radius_changes = false;
    for (column, &d) in v.iter().enumerate() {
        if d.abs() <= MOVE_EPS {
            continue;
        }
        let Some((id, axis)) = owners.get(column).copied().flatten() else {
            continue;
        };
        if seen.insert(id) {
            entities.push(id);
        }
        if axis == Axis::Radius {
            radius_changes = true;
            continue;
        }
        let at = match moves.iter().position(|m| m.0 == id) {
            Some(at) => at,
            None => {
                moves.push((id, 0.0, 0.0));
                moves.len() - 1
            }
        };
        match axis {
            Axis::X => moves[at].1 = d,
            _ => moves[at].2 = d,
        }
    }
    if entities.is_empty() {
        return None;
    }
    let kind = match moves.first() {
        Some(&(_, dx, dy))
            if !radius_changes && moves.iter().all(|m| same(m.1, dx) && same(m.2, dy)) =>
        {
            let length = dx.hypot(dy);
            MotionKind::Slide {
                dx: dx / length,
                dy: dy / length,
            }
        }
        _ => MotionKind::Reshape,
    };
    Some(FreeMotion {
        entities,
        kind,
        dof: 1,
    })
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= MOVE_EPS * a.abs().max(b.abs()).max(1.0)
}

/// Two slides of the same points in different directions are one motion
/// in any direction, worth two degrees of freedom; it takes the first
/// slide's place.
fn merge_plane_slides(motions: Vec<FreeMotion>) -> Vec<FreeMotion> {
    let mut merged: Vec<FreeMotion> = Vec::with_capacity(motions.len());
    for motion in motions {
        let partner = merged.iter_mut().find(|m| {
            matches!(m.kind, MotionKind::Slide { .. })
                && matches!(motion.kind, MotionKind::Slide { .. })
                && m.entities == motion.entities
        });
        match partner {
            Some(partner) => {
                partner.kind = MotionKind::AnyDirection;
                partner.dof = 2;
            }
            None => merged.push(motion),
        }
    }
    merged
}
