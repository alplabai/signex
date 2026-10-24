pub mod dof;
pub mod jacobian;
pub mod linalg;
pub mod lm;
pub mod math;
pub mod residual;
pub mod residuals;
pub mod state;

use std::collections::HashMap;

use crate::error::SolveError;
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

use self::dof::{DofColor, entity_colours, over_constraint_ids};
use self::jacobian::numerical_jacobian;
use self::lm::{
    LmFailure, SolveResult, solve_lm_keeping_state, solve_lm_perturbed, undo_retry_offset,
};
use self::residual::ResolvedParams;

type LmOutcome = Result<SolveResult, Box<LmFailure>>;

/// Longest named conflict that is narrowed one constraint at a time.
const SHRINK_LIMIT: usize = 8;

/// Top-level solver façade. Configure timeouts / iteration cap /
/// tolerance, then call [`Solver::solve`] to run an LM iteration
/// followed by DOF analysis in a single shot.
///
/// Public API patterned after the spec in
/// `docs/internal/SKETCH_MODE_v0.13_PLAN.md` Task 3.6.
#[derive(Clone, Debug)]
pub struct Solver {
    /// Wall-clock timeout per `solve()` call. Default 50 ms.
    pub timeout_ms: u64,
    /// Maximum LM iterations before declaring divergence. Default 100.
    pub max_iters: usize,
    /// `|r|² < tolerance` declares convergence. Default 1e-12.
    pub tolerance: f64,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            timeout_ms: 50,
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
    fn run(&self, sketch: &SketchData, params: &ResolvedParams, perturbed: bool) -> LmOutcome {
        let solve = if perturbed {
            solve_lm_perturbed
        } else {
            solve_lm_keeping_state
        };
        solve(
            sketch,
            params,
            self.timeout_ms,
            self.tolerance,
            self.max_iters,
        )
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
        failure: LmFailure,
    ) -> Result<SolveResult, SolveError> {
        if !failure.stalled {
            return Err(failure.error);
        }
        let retry = match self.run(sketch, params, true) {
            Ok(result) => {
                return Ok(undo_retry_offset(
                    sketch,
                    params,
                    self.timeout_ms,
                    self.tolerance,
                    self.max_iters,
                    result,
                ));
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
        let conflicting = over_constraint_ids(sketch, &stalled, &[], params);
        let did_not_converge = SolveError::DidNotConverge {
            iters,
            final_residual_norm,
        };
        if conflicting.is_empty() {
            return Err(did_not_converge);
        }
        if !self.converges_without(sketch, params, &conflicting) {
            return Err(did_not_converge);
        }
        Err(SolveError::OverConstrained(self.shrink_conflict(
            sketch,
            params,
            conflicting,
        )))
    }

    /// Whether the sketch converges with `removed` constraints taken out
    /// (a plain run, then a perturbed one).
    fn converges_without(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        removed: &[ConstraintId],
    ) -> bool {
        let mut without = sketch.clone();
        without.constraints.retain(|c| !removed.contains(&c.id));
        self.run(&without, params, false).is_ok() || self.run(&without, params, true).is_ok()
    }

    /// Keep only the named constraints whose removal alone resolves the
    /// conflict; fall back to the full set when none does, or when it is
    /// longer than [`SHRINK_LIMIT`]. Cost: at most `2 × SHRINK_LIMIT` extra
    /// LM runs, each bounded by `timeout_ms`.
    fn shrink_conflict(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
        named: Vec<ConstraintId>,
    ) -> Vec<ConstraintId> {
        if named.len() < 2 || named.len() > SHRINK_LIMIT {
            return named;
        }
        let culprits: Vec<ConstraintId> = named
            .iter()
            .copied()
            .filter(|id| self.converges_without(sketch, params, &[*id]))
            .collect();
        if culprits.is_empty() { named } else { culprits }
    }

    /// Run LM to convergence, then compute DOF analysis on the
    /// solved state. Errors propagate from [`solve_lm`]; on success
    /// every output field is populated.
    pub fn solve(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
    ) -> Result<FullSolveOutput, SolveError> {
        let result = match self.run(sketch, params, false) {
            Ok(result) => result,
            Err(failure) => self.recover(sketch, params, *failure)?,
        };

        // Compute the Jacobian once at the solved state and reuse
        // for DOF colour + over-constraint detection. A constraint-
        // free sketch has no Jacobian rows (m=0); we still build it
        // so the API surface is consistent.
        let jacobian = numerical_jacobian(sketch, &result.state, &result.index, params)
            .map_err(|e| SolveError::Internal(format!("post-solve jacobian: {e}")))?;

        // HI-14: thread the solved params through both analyses so
        // parametric constraints are evaluated rather than dropped.
        let colours = entity_colours(sketch, &result, &jacobian, &result.index, params);
        let over_constraints = over_constraint_ids(sketch, &result, &jacobian, params);

        Ok(FullSolveOutput {
            result,
            colours,
            over_constraints,
            jacobian,
            params: params.clone(),
        })
    }
}
