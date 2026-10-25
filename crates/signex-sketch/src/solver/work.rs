//! Deterministic work accounting for the constraint solver.
//!
//! The solver is bounded by a count of *work units*, not by the clock, so a
//! given sketch solves or fails the same way on every machine and every run.
//! One [`WorkMeter`] is shared by every step of one [`crate::solver::Solver`]
//! solve (the first run, the perturbed retry, the offset undo, the
//! conflict-recovery runs and the DOF analysis), so the total is bounded, not
//! just each run.
//!
//! A work unit is one residual-row evaluation, or one multiply-add of the
//! dense linear algebra, whichever step is being counted. With `n` state
//! variables and `m` residual rows:
//!
//! | step | work units |
//! |---|---|
//! | central-difference Jacobian | `2·n·m` |
//! | one LM iteration | `(2n + 2)·m + m·n² + n³` |
//! | row-space projection (undo pass) | `m²·n + m³` |
//! | DOF analysis after the solve | `m·n²` (QR) `+ m` |

use crate::error::SolveError;

/// Work of one central-difference Jacobian: two residual evaluations per
/// state variable, each over `m` rows.
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

/// Work of the DOF analysis after the solve: one Jacobian, its QR (`m·n²`)
/// and one residual pass for the over-constraint check.
pub(crate) fn post_solve_cost(n: usize, m: usize) -> u64 {
    let (n64, m64) = (n as u64, m as u64);
    jacobian_cost(n, m)
        .saturating_add(m64.saturating_mul(n64).saturating_mul(n64))
        .saturating_add(m64)
}

/// The shared work pool of one solve.
///
/// Charging more than the pool holds fails and leaves the pool exhausted, so
/// every later charge fails too. A full solve also holds back the work of its
/// own DOF analysis ([`WorkMeter::for_full_solve`]), so a sketch that
/// converged is never left unanalysed.
#[derive(Debug, Clone)]
pub struct WorkMeter {
    budget: u64,
    spent: u64,
    reserve: u64,
    holds_back_analysis: bool,
}

impl WorkMeter {
    /// A pool of `budget` work units for plain LM runs.
    pub fn new(budget: u64) -> Self {
        Self {
            budget,
            spent: 0,
            reserve: 0,
            holds_back_analysis: false,
        }
    }

    /// A pool for [`crate::solver::Solver::solve`]: once the sketch's size is
    /// known, the work of the DOF analysis is held back from the LM runs.
    pub fn for_full_solve(budget: u64) -> Self {
        Self {
            holds_back_analysis: true,
            ..Self::new(budget)
        }
    }

    /// The whole pool, in work units.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Work charged so far. Never exceeds [`WorkMeter::budget`].
    pub fn spent(&self) -> u64 {
        self.spent
    }

    /// Work units still available to the LM runs and recovery steps.
    pub fn remaining(&self) -> u64 {
        self.budget
            .saturating_sub(self.reserve)
            .saturating_sub(self.spent)
    }

    /// Hold back the DOF analysis of a sketch with `n` variables and `m` rows.
    /// Only the first call of a full solve acts: that run sees the whole sketch.
    pub(crate) fn hold_back_analysis(&mut self, n: usize, m: usize) {
        if self.holds_back_analysis && self.reserve == 0 {
            self.reserve = post_solve_cost(n, m);
        }
    }

    /// Charge `cost` work units.
    ///
    /// [`SolveError::TooLarge`] when the step alone cannot fit the pool even
    /// when it is untouched; [`SolveError::BudgetExceeded`] when the pool has
    /// not enough left.
    pub(crate) fn charge(&mut self, cost: u64) -> Result<(), SolveError> {
        let ceiling = self.budget.saturating_sub(self.reserve);
        if cost > ceiling {
            return Err(SolveError::TooLarge {
                needed: cost.saturating_add(self.reserve),
                budget: self.budget,
            });
        }
        let total = self.spent.saturating_add(cost);
        if total > ceiling {
            let reported = total.saturating_add(self.reserve);
            self.spent = ceiling;
            return Err(SolveError::BudgetExceeded {
                spent: reported,
                budget: self.budget,
            });
        }
        self.spent = total;
        Ok(())
    }

    /// Charge the DOF analysis out of the held-back work. Cannot fail for a
    /// full solve; a pool that held nothing back charges what it can.
    pub(crate) fn charge_analysis(&mut self, n: usize, m: usize) {
        self.reserve = 0;
        let cost = post_solve_cost(n, m);
        self.spent = self.spent.saturating_add(cost).min(self.budget);
    }
}
