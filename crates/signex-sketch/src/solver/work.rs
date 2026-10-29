//! Deterministic work accounting for the constraint solver.
//!
//! The solver is bounded by a count of *work units*, not by the clock, so a
//! given sketch solves or fails the same way on every run of the same build
//! and platform (`hypot`, `atan2`, `sin` and `cos` come from the platform's
//! maths library, so two platforms can differ in the last bit).
//! One [`WorkMeter`] is shared by every step of one [`crate::solver::Solver`]
//! solve (the first run, the perturbed retry, the offset undo, the
//! conflict-recovery runs), so the total is bounded, not just each run. The DOF
//! analysis of a converged sketch is outside the pool, see
//! [`WorkMeter::analysis_spent`].
//!
//! A work unit is one residual-row evaluation, or one multiply-add of the
//! dense linear algebra, whichever step is being counted. With `n` state
//! variables and `m` residual rows:
//!
//! | step | work units |
//! |---|---|
//! | Jacobian (accounted as central differences) | `2·n·m` |
//! | one LM iteration | `(2n + 2)·m + m·n² + n³` |
//! | row-space projection (undo pass) | `m²·n + m³` |
//! | redundancy pass, one block of `r` rows over `d` columns | `r·d + 4·d·Σᵢ min(i, d)` |

use crate::error::SolveError;

/// Work charged for one Jacobian: two residual evaluations per state
/// variable, each over `m` rows. The analytic Jacobian does less; the charge
/// is kept so a given budget solves or fails exactly as before.
pub(crate) fn jacobian_cost(n: usize, m: usize) -> u64 {
    2u64.saturating_mul(n as u64).saturating_mul(m as u64)
}

/// Work of one LM iteration: the Jacobian, the trial step and the baseline
/// (`(2n + 2)·m` residual-row evaluations), forming `JᵀJ` (`m·n²`) and the
/// dense LU of the damped normal matrix (`n³`).
pub(crate) fn iteration_cost(n: usize, m: usize) -> u64 {
    let (n, m) = (n as u64, m as u64);
    let evaluations = 2u64.saturating_mul(n).saturating_add(2).saturating_mul(m);
    let normal_matrix = m.saturating_mul(n).saturating_mul(n);
    let factorisation = n.saturating_mul(n).saturating_mul(n);
    evaluations
        .saturating_add(normal_matrix)
        .saturating_add(factorisation)
}

/// Work of projecting the retry offset onto the Jacobian's row space: the
/// `J·Jᵀ` product (`m²·n`) and its `m × m` solve (`m³`).
pub(crate) fn row_space_cost(n: usize, m: usize) -> u64 {
    let (n, m) = (n as u64, m as u64);
    m.saturating_mul(m)
        .saturating_mul(n)
        .saturating_add(m.saturating_mul(m).saturating_mul(m))
}

/// Work of the redundancy scan of one block of `rows` Jacobian rows over
/// `cols` distinct state columns: densifying the rows (`r·d`) and one
/// two-pass Gram-Schmidt projection per row against the basis kept so far,
/// `4·b·d` multiply-adds against a basis of `b <= min(i, d)` vectors. This is
/// the worst case (every row independent), so a block is charged, and refused,
/// before any of it runs.
pub(crate) fn redundancy_block_cost(rows: usize, cols: usize) -> u64 {
    let (r, d) = (rows as u64, cols as u64);
    // Σ_{i<r} min(i, d): the triangle up to d, then d per further row.
    let basis_sum = if r <= d {
        r.saturating_mul(r.saturating_sub(1)) / 2
    } else {
        (d.saturating_mul(d.saturating_sub(1)) / 2).saturating_add((r - d).saturating_mul(d))
    };
    r.saturating_mul(d)
        .saturating_add(4u64.saturating_mul(d).saturating_mul(basis_sum))
}

/// Work units of the DOF analysis after a solve: one Jacobian, its QR
/// (`m·n²`) and one residual pass for the over-constraint check. Recorded by
/// [`WorkMeter::record_analysis`], never charged to the pool (see there).
pub(crate) fn analysis_cost(n: usize, m: usize) -> u64 {
    let (n64, m64) = (n as u64, m as u64);
    jacobian_cost(n, m)
        .saturating_add(m64.saturating_mul(n64).saturating_mul(n64))
        .saturating_add(m64)
}

/// The shared work pool of one solve.
///
/// Charging more than the pool holds fails and leaves the pool exhausted, so
/// every later charge fails too.
#[derive(Debug, Clone)]
pub struct WorkMeter {
    budget: u64,
    spent: u64,
    analysis: u64,
}

impl WorkMeter {
    /// A pool of `budget` work units.
    pub fn new(budget: u64) -> Self {
        Self {
            budget,
            spent: 0,
            analysis: 0,
        }
    }

    /// The whole pool, in work units.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Work charged to the pool so far. Never exceeds [`WorkMeter::budget`].
    pub fn spent(&self) -> u64 {
        self.spent
    }

    /// Work units of the DOF analysis of a converged sketch, in the same
    /// units but outside the pool. The analysis only runs on a sketch that
    /// already converged, and failing a converged sketch would drop its
    /// geometry, so it can neither fail nor be refused. It is the one part of
    /// a solve the budget does not bound: it grows as `m·n²` and measures
    /// 0.7 ns per unit on an under-constrained sketch (`m` well below `n`)
    /// and up to 2.7 ns per unit on a fully constrained one (`m` close to
    /// `n`, the QR's worst case), so up to three times the LM runs' rate.
    pub fn analysis_spent(&self) -> u64 {
        self.analysis
    }

    /// Work units still available to the LM runs and recovery steps.
    pub fn remaining(&self) -> u64 {
        self.budget.saturating_sub(self.spent)
    }

    /// Charge `cost` work units.
    ///
    /// [`SolveError::TooLarge`] when the step alone cannot fit the whole
    /// pool; [`SolveError::BudgetExceeded`] when the pool has not enough left.
    pub(crate) fn charge(&mut self, cost: u64) -> Result<(), SolveError> {
        if cost > self.budget {
            return Err(SolveError::TooLarge {
                needed: cost,
                budget: self.budget,
            });
        }
        let total = self.spent.saturating_add(cost);
        if total > self.budget {
            self.spent = self.budget;
            return Err(SolveError::BudgetExceeded {
                spent: total,
                budget: self.budget,
            });
        }
        self.spent = total;
        Ok(())
    }

    /// Charge `cost` work units only if they fit what is left. A refusal
    /// leaves the pool untouched, unlike [`WorkMeter::charge`]: it is for
    /// optional, advisory work that may be skipped.
    pub(crate) fn try_charge(&mut self, cost: u64) -> bool {
        if cost > self.remaining() {
            return false;
        }
        self.spent += cost;
        true
    }

    /// Record the DOF analysis of a sketch with `n` variables and `m` rows.
    pub(crate) fn record_analysis(&mut self, n: usize, m: usize) {
        self.analysis = self.analysis.saturating_add(analysis_cost(n, m));
    }
}
