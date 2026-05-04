pub mod state;
pub mod residual;
pub mod residuals;
pub mod jacobian;
pub mod lm;
pub mod linalg;
pub mod math;
pub mod dof;
pub mod timeout;

use std::collections::HashMap;

use crate::error::SolveError;
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

use self::dof::{entity_colours, over_constraint_ids, DofColor};
use self::jacobian::numerical_jacobian;
use self::lm::{solve_lm, SolveResult};
use self::residual::ResolvedParams;

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
/// over-constrained, and the Jacobian at the solved state (carried
/// for downstream UI/debugging — DOF rendering reuses it without
/// recomputing).
#[derive(Debug, Clone)]
pub struct FullSolveOutput {
    pub result: SolveResult,
    pub colours: HashMap<SketchEntityId, DofColor>,
    pub over_constraints: Vec<ConstraintId>,
    pub jacobian: Vec<Vec<f64>>,
}

impl Solver {
    /// Run LM to convergence, then compute DOF analysis on the
    /// solved state. Errors propagate from [`solve_lm`]; on success
    /// every output field is populated.
    pub fn solve(
        &self,
        sketch: &SketchData,
        params: &ResolvedParams,
    ) -> Result<FullSolveOutput, SolveError> {
        let result = solve_lm(
            sketch,
            params,
            self.timeout_ms,
            self.tolerance,
            self.max_iters,
        )?;

        // Compute the Jacobian once at the solved state and reuse
        // for DOF colour + over-constraint detection. A constraint-
        // free sketch has no Jacobian rows (m=0); we still build it
        // so the API surface is consistent.
        let jacobian = numerical_jacobian(sketch, &result.state, &result.index, params)
            .map_err(|_| SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())))?;

        let colours = entity_colours(sketch, &result, &jacobian, &result.index);
        let over_constraints = over_constraint_ids(sketch, &result, &jacobian);

        Ok(FullSolveOutput {
            result,
            colours,
            over_constraints,
            jacobian,
        })
    }
}
