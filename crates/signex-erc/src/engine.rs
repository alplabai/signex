//! Rule engine: runs all registered built-in rules against an [`ErcContext`]
//! and returns a flat list of [`Diagnostic`]s in rule order.
//!
//! DSL-compiled rules plug in via [`run_all_with_dsl`] using the [`EvalFn`]
//! type alias so the DSL crate never needs to depend back on the engine.

use std::sync::Arc;

use crate::context::ErcContext;
use crate::diagnostic::{Diagnostic, LocationSpace};
use crate::rules;

/// Evaluation function produced by the DSL compiler. Takes a read-only
/// [`ErcContext`] and returns the diagnostics it found.
pub type EvalFn = Arc<dyn Fn(&ErcContext) -> Vec<Diagnostic> + Send + Sync>;

/// Run every built-in rule against `ctx` and return all diagnostics.
pub fn run_all(ctx: &ErcContext) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    rules::unused_pin(ctx, &mut out);
    rules::duplicate_ref_designator(ctx, &mut out);
    rules::hier_port_disconnected(ctx, &mut out);
    rules::dangling_wire(ctx, &mut out);
    rules::net_label_conflict(ctx, &mut out);
    rules::orphan_label(ctx, &mut out);
    rules::bus_bit_width_mismatch(ctx, &mut out);
    rules::bad_hier_sheet_pin(ctx, &mut out);
    rules::missing_power_flag(ctx, &mut out);
    rules::power_port_short(ctx, &mut out);
    rules::symbol_outside_sheet(ctx, &mut out);
    rules::ambiguous_label_anchor(ctx, &mut out);
    drop_suppressed(ctx, &mut out);
    out
}

/// Drop every diagnostic located on a No-ERC directive. Positional only: the
/// directive silences all rules at its point. Diagnostics whose location is a
/// placeholder or lives in another sheet's coordinates are never matched.
fn drop_suppressed(ctx: &ErcContext, diagnostics: &mut Vec<Diagnostic>) {
    if ctx.no_erc_directives.is_empty() {
        return;
    }
    diagnostics.retain(|d| {
        d.location_space != LocationSpace::ThisSheet || !ctx.has_no_erc_directive_at(&d.location)
    });
}

/// Run built-in rules **and** any DSL-compiled rules in a single pass.
pub fn run_all_with_dsl(ctx: &ErcContext, dsl_rules: &[EvalFn]) -> Vec<Diagnostic> {
    let mut out = run_all(ctx);
    let mut dsl = Vec::new();
    for rule in dsl_rules {
        dsl.extend(rule(ctx));
    }
    drop_suppressed(ctx, &mut dsl);
    out.extend(dsl);
    out
}
