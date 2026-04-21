//! Rule engine: runs all registered built-in rules against an [`ErcContext`]
//! and returns a flat list of [`Diagnostic`]s in rule order.
//!
//! Phase 2 will extend this with DSL-compiled rules that slot into the same
//! pipeline after the built-in pass.

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::rules;

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
    out
}
