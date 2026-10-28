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
//! A constraint is flagged only when EVERY one of its rows is implied by the
//! constraints before it: a multi-row constraint with one new direction still
//! removes a degree of freedom, so deleting it would not be free.
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

/// Constraints all of whose Jacobian rows are linear combinations of the
/// rows of the constraints before them (and the arc closures), in
/// constraint order. Constraints in `violated` are neither flagged nor
/// allowed to imply others.
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
    for c in sketch
        .constraints
        .iter()
        .filter(|c| !violated.contains(&c.id))
    {
        for row in constraint_rows(c, state, index, sketch, params)? {
            rows.push((Some(c.id), row));
        }
    }

    let mut redundant: HashSet<ConstraintId> = HashSet::new();
    for block in blocks(&rows, state.len()) {
        scan_block(&rows, &block, &mut redundant);
    }
    Ok(sketch
        .constraints
        .iter()
        .map(|c| c.id)
        .filter(|id| redundant.contains(id))
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

/// Reduce one block in order, a constraint at a time: its rows are tested
/// against the basis of everything before it, then added to that basis.
fn scan_block(
    rows: &[(Option<ConstraintId>, SparseRow)],
    block: &[usize],
    redundant: &mut HashSet<ConstraintId>,
) {
    let mut local: HashMap<usize, usize> = HashMap::new();
    for &i in block {
        for &(col, _) in &rows[i].1 {
            let next = local.len();
            local.entry(col).or_insert(next);
        }
    }
    let dense = |i: usize| {
        let mut v = vec![0.0; local.len()];
        for &(col, x) in &rows[i].1 {
            v[local[&col]] += x;
        }
        v
    };
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut start = 0;
    while start < block.len() {
        let owner = rows[block[start]].0;
        let mut end = start + 1;
        while owner.is_some() && end < block.len() && rows[block[end]].0 == owner {
            end += 1;
        }
        let vectors: Vec<Vec<f64>> = block[start..end].iter().map(|&i| dense(i)).collect();
        if let Some(id) = owner
            && implied_by(&vectors, &basis)
        {
            redundant.insert(id);
        }
        for v in vectors {
            let before = norm(&v);
            let v = project_out(v, &basis);
            let after = norm(&v);
            if before > RANK_TOL && after > DEPENDENCE_TOL * before {
                basis.push(v.iter().map(|x| x / after).collect());
            }
        }
        start = end;
    }
}

/// Whether every row with a gradient lies in the span of `basis`, and at
/// least one has a gradient (a constraint that is flat at this state says
/// nothing).
fn implied_by(rows: &[Vec<f64>], basis: &[Vec<f64>]) -> bool {
    let mut any = false;
    for v in rows {
        let before = norm(v);
        if before <= RANK_TOL {
            continue;
        }
        any = true;
        if norm(&project_out(v.clone(), basis)) > DEPENDENCE_TOL * before {
            return false;
        }
    }
    any
}

/// `v` minus its projection onto the orthonormal `basis`. Two passes keep
/// the result orthogonal to rounding.
fn project_out(mut v: Vec<f64>, basis: &[Vec<f64>]) -> Vec<f64> {
    for _ in 0..2 {
        for q in basis {
            let dot: f64 = q.iter().zip(&v).map(|(a, b)| a * b).sum();
            v.iter_mut().zip(q).for_each(|(x, a)| *x -= dot * a);
        }
    }
    v
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
