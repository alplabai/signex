pub mod dof;
pub mod jacobian;
pub mod linalg;
pub mod lm;
pub mod math;
pub mod residual;
pub mod residuals;
pub mod state;
pub mod work;

#[cfg(test)]
mod shrink_tests;

use std::collections::HashMap;

use crate::error::SolveError;
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

use self::dof::{DofColor, entity_colours, over_constraint_ids};
use self::jacobian::numerical_jacobian;
use self::lm::{
    LmFailure, SolveResult, solve_lm_keeping_state, solve_lm_perturbed, undo_retry_offset,
};
use self::residual::{ResolvedParams, total_residual};
use self::work::WorkMeter;

type LmOutcome = Result<SolveResult, Box<LmFailure>>;

/// Longest named conflict that is narrowed one constraint at a time.
const SHRINK_LIMIT: usize = 8;

/// Top-level solver façade. Configure the work budget / iteration cap /
/// tolerance, then call [`Solver::solve`] to run an LM iteration
/// followed by DOF analysis in a single shot.
///
/// Public API patterned after the spec in
/// `docs/internal/SKETCH_MODE_v0.13_PLAN.md` Task 3.6.
#[derive(Clone, Debug)]
pub struct Solver {
    /// Work cap of one whole [`Solver::solve`], in work units (see
    /// [`lm::DEFAULT_WORK_BUDGET`] and [`work`]): the first run, the retry,
    /// the offset undo and conflict recovery share it. The DOF analysis of a
    /// converged sketch is outside it. Counted, not timed, so a given sketch
    /// solves or fails the same way on every run of the same build and
    /// platform (`hypot`, `atan2`, `sin` and `cos` come from the platform's
    /// maths library).
    pub work_budget: u64,
    /// Maximum LM iterations before declaring divergence. Default 100.
    pub max_iters: usize,
    /// `|r|² < tolerance²` declares convergence (the linear residual norm is
    /// compared against `tolerance`). Default 1e-12.
    pub tolerance: f64,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            work_budget: lm::DEFAULT_WORK_BUDGET,
            max_iters: 100,
            tolerance: 1e-12,
        }
    }
}

/// Bundled output of a full [`Solver::solve`] call: the LM result,
/// DOF colour per Point entity, the list of constraint IDs flagged
/// over-constrained, the Jacobian at the solved state (carried
/// for downstream UI/debugging — DOF rendering reuses it without
/// recomputing), and the resolved parameter map the solve ran with.
#[derive(Debug, Clone)]
pub struct FullSolveOutput {
    pub result: SolveResult,
    pub colours: HashMap<SketchEntityId, DofColor>,
    pub over_constraints: Vec<ConstraintId>,
    pub jacobian: Vec<Vec<f64>>,
    /// GH #599 — the resolved parameters this solve ran with.
    /// Consumers that re-evaluate residuals against the output (the
    /// over-constraint diagnostics panel) need the same map: an
    /// empty one makes every parameter-driven constraint fail to
    /// evaluate, which reads as a zero residual and hides the
    /// conflict. Carrying it here makes the two impossible to
    /// desynchronise.
    pub params: ResolvedParams,
}

impl Solver {
    fn run(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        meter: &mut WorkMeter,
        perturbed: bool,
    ) -> LmOutcome {
        let solve = if perturbed {
            solve_lm_perturbed
        } else {
            solve_lm_keeping_state
        };
        solve(sketch, params, meter, self.tolerance, self.max_iters)
    }

    /// A stalled first run is retried from a slightly moved start (a
    /// saddle start stalls on a solvable sketch). The sketch is reported
    /// over-constrained only if the retry stalls too, constraints are
    /// still violated there, and the sketch converges once exactly those
    /// constraints are removed. Anything else stays `DidNotConverge`.
    fn recover(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        meter: &mut WorkMeter,
        failure: LmFailure,
    ) -> Result<SolveResult, SolveError> {
        if !failure.stalled {
            return Err(failure.error);
        }
        let retry = match self.run(sketch, params, meter, true) {
            Ok(result) => {
                return undo_retry_offset(
                    sketch,
                    params,
                    meter,
                    self.tolerance,
                    self.max_iters,
                    result,
                );
            }
            Err(retry) if retry.stalled => *retry,
            Err(retry) => return Err(retry.error),
        };
        let SolveError::DidNotConverge {
            iters,
            final_residual_norm,
        } = retry.error
        else {
            return Err(retry.error);
        };
        let stalled = SolveResult {
            state: retry.state,
            index: retry.index,
            iterations: iters,
            final_residual_norm,
            elapsed_ms: 0,
        };
        // One residual pass over every constraint.
        meter.charge(stalled_rows(sketch, &stalled, params))?;
        let conflicting = over_constraint_ids(sketch, &stalled, params);
        let did_not_converge = SolveError::DidNotConverge {
            iters,
            final_residual_norm,
        };
        if conflicting.is_empty() {
            return Err(did_not_converge);
        }
        if !self.converges_without(sketch, params, meter, &conflicting)? {
            return Err(did_not_converge);
        }
        Err(SolveError::OverConstrained(self.shrink_conflict(
            sketch,
            params,
            meter,
            conflicting,
        )))
    }

    /// Whether the sketch converges with `removed` constraints taken out
    /// (a plain run, then a perturbed one). An exhausted work pool is an
    /// error, not a "no": nothing was learned about the sketch.
    fn converges_without(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        meter: &mut WorkMeter,
        removed: &[ConstraintId],
    ) -> Result<bool, SolveError> {
        let mut without = sketch.clone();
        without.constraints.retain(|c| !removed.contains(&c.id));
        for perturbed in [false, true] {
            match self.run(&without, params, meter, perturbed) {
                Ok(_) => return Ok(true),
                Err(failure) if is_pool_failure(&failure.error) => return Err(failure.error),
                Err(_) => {}
            }
        }
        Ok(false)
    }

    /// Keep only the named constraints whose removal alone resolves the
    /// conflict; fall back to the full set when none does, or when it is
    /// longer than [`SHRINK_LIMIT`], or when the work pool runs out (the
    /// conflict is already established; only its narrowing is cut short).
    /// Cost: at most `2 × SHRINK_LIMIT` extra LM runs, all drawn from the
    /// solve's one pool.
    fn shrink_conflict(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        meter: &mut WorkMeter,
        named: Vec<ConstraintId>,
    ) -> Vec<ConstraintId> {
        if named.len() < 2 || named.len() > SHRINK_LIMIT {
            return named;
        }
        let mut culprits = Vec::new();
        for id in &named {
            match self.converges_without(sketch, params, meter, &[*id]) {
                Ok(true) => culprits.push(*id),
                Ok(false) => {}
                // The pool ran out mid-narrowing: the rest were not tested,
                // so the full list is the only answer that cannot leave a
                // real culprit out (it may over-name, never under-name).
                Err(_) => return named,
            }
        }
        if culprits.is_empty() { named } else { culprits }
    }

    /// Run LM to convergence, then compute DOF analysis on the
    /// solved state. Errors propagate from [`lm::solve_lm`]; on success
    /// every output field is populated. The whole solve, retries and
    /// conflict recovery included, is bounded by [`Solver::work_budget`].
    pub fn solve(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
    ) -> Result<FullSolveOutput, SolveError> {
        let mut meter = WorkMeter::new(self.work_budget);
        self.solve_with_meter(sketch, params, &mut meter)
    }

    /// [`Solver::solve`] drawing on the caller's [`WorkMeter`], so the work a solve really cost can be
    /// read back from [`WorkMeter::spent`], also when it failed. The meter's
    /// own budget applies; [`Solver::work_budget`] is not consulted.
    pub fn solve_with_meter(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        meter: &mut WorkMeter,
    ) -> Result<FullSolveOutput, SolveError> {
        let result = match self.run(sketch, params, meter, false) {
            Ok(result) => result,
            Err(failure) => self.recover(sketch, params, meter, *failure)?,
        };

        // Compute the Jacobian once at the solved state and reuse
        // for DOF colour + over-constraint detection. A constraint-
        // free sketch has no Jacobian rows (m=0); we still build it
        // so the API surface is consistent. The analysis runs outside the
        // work pool and cannot fail a converged solve.
        let jacobian = numerical_jacobian(sketch, &result.state, &result.index, params)
            .map_err(|e| lm::residual_failure("post-solve jacobian", e))?;
        meter.record_analysis(result.state.len(), jacobian.len());

        // HI-14: thread the solved params through both analyses so
        // parametric constraints are evaluated rather than dropped.
        let colours = entity_colours(sketch, &result, &jacobian, &result.index, params);
        let over_constraints = over_constraint_ids(sketch, &result, params);

        Ok(FullSolveOutput {
            result,
            colours,
            over_constraints,
            jacobian,
            params: params.clone(),
        })
    }
}

/// The failures that say the work pool, not the sketch, ended the run.
fn is_pool_failure(error: &SolveError) -> bool {
    matches!(
        error,
        SolveError::BudgetExceeded { .. } | SolveError::TooLarge { .. }
    )
}

/// Residual rows of `sketch` at a stalled state (0 if it cannot be evaluated).
fn stalled_rows(sketch: &SketchData, at: &SolveResult, params: &ResolvedParams) -> u64 {
    total_residual(sketch, &at.state, &at.index, params).map_or(0, |r| r.len() as u64)
}
