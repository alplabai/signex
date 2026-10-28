//! Redundant-but-consistent constraint detection.
//!
//! [`super::dof`] compares the Jacobian's rank with the variable count, which
//! says THAT a sketch has more equations than it needs but not WHICH
//! constraints. This module names them: the Jacobian rows are taken in
//! constraint order and reduced by Gram-Schmidt against the rows kept so
//! far. A row that adds no new direction is a linear combination of earlier
//! ones, so the constraint owning it restates what the earlier constraints
//! already fix. The later constraint is the one flagged.
//!
//! The scan is per connected block of constraints (those sharing a state
//! column), so sketches with many independent pads cost the sum of small
//! blocks, not one large matrix.
//!
//! Conflicting constraints are not this module's business: a conflict is a
//! solve failure that already names its constraints
//! ([`crate::SolveError::OverConstrained`]), and a constraint still violated
//! at the solved state is left out here.

use std::collections::{HashMap, HashSet};

use crate::error::SketchError;
use crate::id::ConstraintId;
use crate::sketch::SketchData;
use crate::solver::derivatives::{SparseRow, arc_closure_rows, constraint_rows};
use crate::solver::dof::RANK_TOL;
use crate::solver::residual::ResolvedParams;
use crate::solver::state::EntityIndex;

/// A row is dependent when what survives projection onto the earlier rows
/// is under this fraction of its own length.
const DEPENDENCE_TOL: f64 = 1e-8;

/// Constraints with at least one Jacobian row that is a linear combination
/// of the rows before it, in constraint order, excluding any listed in
/// `violated`.
pub fn redundant_constraint_ids(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    params: &ResolvedParams,
    violated: &[ConstraintId],
) -> Result<Vec<ConstraintId>, SketchError> {
    // Arc closures are structural equations: they join the basis first, so
    // a constraint repeating one is the redundant side.
    let mut rows: Vec<(Option<ConstraintId>, SparseRow)> = arc_closure_rows(state, index, sketch)?
        .into_iter()
        .map(|row| (None, row))
        .collect();
    for c in &sketch.constraints {
        for row in constraint_rows(c, state, index, sketch, params)? {
            rows.push((Some(c.id), row));
        }
    }

    let mut redundant: Vec<ConstraintId> = Vec::new();
    for block in blocks(&rows, state.len()) {
        scan_block(&rows, &block, &mut redundant);
    }
    let skip: HashSet<ConstraintId> = violated.iter().copied().collect();
    let flagged: HashSet<ConstraintId> = redundant
        .into_iter()
        .filter(|id| !skip.contains(id))
        .collect();
    Ok(sketch
        .constraints
        .iter()
        .map(|c| c.id)
        .filter(|id| flagged.contains(id))
        .collect())
}

/// Row indices grouped by shared columns (union-find over the columns).
fn blocks(rows: &[(Option<ConstraintId>, SparseRow)], n: usize) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for (_, row) in rows {
        let Some(&(first, _)) = row.first() else {
            continue;
        };
        for &(col, _) in &row[1..] {
            let (a, b) = (find(&mut parent, first), find(&mut parent, col));
            parent[a] = b;
        }
    }
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, (_, row)) in rows.iter().enumerate() {
        if let Some(&(col, _)) = row.first() {
            by_root.entry(find(&mut parent, col)).or_default().push(i);
        }
    }
    let mut blocks: Vec<Vec<usize>> = by_root.into_values().collect();
    blocks.sort_by_key(|b| b[0]);
    blocks
}

/// Reduce one block's rows in order, recording the owner of each dependent row.
fn scan_block(
    rows: &[(Option<ConstraintId>, SparseRow)],
    block: &[usize],
    redundant: &mut Vec<ConstraintId>,
) {
    let mut local: HashMap<usize, usize> = HashMap::new();
    for &i in block {
        for &(col, _) in &rows[i].1 {
            let next = local.len();
            local.entry(col).or_insert(next);
        }
    }
    let width = local.len();
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for &i in block {
        let (owner, row) = &rows[i];
        let mut v = vec![0.0; width];
        for &(col, x) in row {
            v[local[&col]] += x;
        }
        let before = norm(&v);
        // A row with no gradient says nothing about dependence.
        if before <= RANK_TOL {
            continue;
        }
        // Two passes keep the projection orthogonal to rounding.
        for _ in 0..2 {
            for q in &basis {
                let dot: f64 = q.iter().zip(&v).map(|(a, b)| a * b).sum();
                v.iter_mut().zip(q).for_each(|(x, a)| *x -= dot * a);
            }
        }
        let after = norm(&v);
        if after <= DEPENDENCE_TOL * before {
            if let Some(id) = owner
                && !redundant.contains(id)
            {
                redundant.push(*id);
            }
        } else {
            v.iter_mut().for_each(|x| *x /= after);
            basis.push(v);
        }
    }
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
