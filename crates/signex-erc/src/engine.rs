//! Rule engine: runs all registered built-in rules against an [`ErcContext`]
//! and returns a flat list of [`Diagnostic`]s in rule order.
//!
//! DSL-compiled rules plug in via [`run_all_with_dsl`] using the [`EvalFn`]
//! type alias so the DSL crate never needs to depend back on the engine.

use std::sync::Arc;

use crate::context::ErcContext;
use crate::diagnostic::{Diagnostic, LocationSpace};
use crate::pin_matrix::ErcConfig;
use crate::rules;

/// Evaluation function produced by the DSL compiler. Takes a read-only
/// [`ErcContext`] and returns the diagnostics it found.
pub type EvalFn = Arc<dyn Fn(&ErcContext) -> Vec<Diagnostic> + Send + Sync>;

/// Run every built-in rule against `ctx` with the baseline pin matrix.
pub fn run_all(ctx: &ErcContext) -> Vec<Diagnostic> {
    run_all_with_config(ctx, &ErcConfig::default())
}

/// Run every built-in rule against `ctx` and return all diagnostics.
pub fn run_all_with_config(ctx: &ErcContext, config: &ErcConfig) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    rules::unused_pin(ctx, &mut out);
    rules::duplicate_ref_designator(ctx, &mut out);
    rules::hier_port_disconnected(ctx, &mut out);
    rules::dangling_wire(ctx, &mut out);
    rules::net_label_conflict(ctx, &mut out);
    rules::orphan_label(ctx, &mut out);
    let bus = rules::bus_analysis(ctx);
    rules::bus_bit_width_mismatch(ctx, &bus, &mut out);
    rules::bus_name_conflict(ctx, &bus, &mut out);
    rules::bus_runs_width_mismatch(ctx, &bus, &mut out);
    rules::bus_port_width_mismatch(ctx, &mut out);
    rules::global_bus_width_mismatch(ctx, &mut out);
    rules::bad_hier_sheet_pin(ctx, &mut out);
    rules::missing_power_flag(ctx, &mut out);
    rules::power_port_short(ctx, &mut out);
    rules::symbol_outside_sheet(ctx, &mut out);
    rules::ambiguous_label_anchor(ctx, &mut out);
    rules::pin_type_conflict(ctx, &config.pin_matrix, &mut out);
    rules::pins_touch_not_connected(ctx, &mut out);
    rules::bus_entry_unlabeled(ctx, &bus, &mut out);
    rules::bus_member_not_in_bus(ctx, &bus, &mut out);
    rules::bus_unnamed(ctx, &bus, &mut out);
    rules::bus_label_on_wire(ctx, &bus, &mut out);
    rules::net_multiple_names(ctx, &mut out);
    rules::local_label_name_on_other_sheets(ctx, &mut out);
    rules::part_without_footprint(ctx, &mut out);
    rules::part_without_value(ctx, &mut out);
    rules::part_without_mpn(ctx, &mut out);
    rules::part_without_library_body(ctx, &mut out);
    rules::unannotated_designator(ctx, &mut out);
    rules::net_completeness(ctx, &mut out);
    rules::units_not_placed(ctx, &mut out);
    rules::off_grid(ctx, &mut out);
    rules::sheet_pin_unconnected(ctx, &mut out);
    rules::sheet_pin_direction_conflict(ctx, &mut out);
    rules::repeat_entry_needs_bus(ctx, &mut out);
    rules::harness_entry_mismatch(ctx, &config.harness_definitions, &mut out);
    rules::harness_type_conflict(ctx, &mut out);
    crate::images::image_file_missing(ctx, &config.missing_images, &mut out);
    rules::net_class_conflict(ctx, &mut out);
    rules::diff_pair_partner_missing(ctx, &mut out);
    drop_suppressed(ctx, &mut out);
    out
}

/// Drop every diagnostic located on a No-ERC directive that silences its rule
/// (every rule when the directive names none, #1699). Diagnostics whose
/// location is a placeholder or lives in another sheet's coordinates are
/// never matched.
/// Then drop what [`directive_suppressed`] drops (#2420).
fn drop_suppressed(ctx: &ErcContext, diagnostics: &mut Vec<Diagnostic>) {
    if !ctx.no_erc_directives.is_empty() {
        diagnostics.retain(|d| {
            d.location_space != LocationSpace::ThisSheet
                || !ctx.has_no_erc_directive_at(&d.location, d.rule_id.as_str())
        });
    }
    if !ctx.directives.is_empty() {
        diagnostics.retain(|d| !directive_suppressed(ctx, d));
    }
}

/// Whether a finding located on this sheet (a point, or a net's anchor pin)
/// sits inside a Compile Mask, or inside a Blanket that excludes its rule.
fn directive_suppressed(ctx: &ErcContext, d: &Diagnostic) -> bool {
    if !matches!(
        d.location_space,
        LocationSpace::ThisSheet | LocationSpace::Net
    ) {
        return false;
    }
    directives_suppress(&ctx.directives, d.location, d.rule_id.as_str())
}

/// Whether one of `directives` covering `at` silences `rule_id`: a Compile
/// Mask silences every rule, a Blanket the rules it excludes. A point
/// directive covers nothing.
pub(crate) fn directives_suppress(
    directives: &[signex_types::schematic::SchDirective],
    at: signex_types::schematic::Point,
    rule_id: &str,
) -> bool {
    directives.iter().any(|directive| {
        directive.region_contains(at)
            && (matches!(
                directive,
                signex_types::schematic::SchDirective::CompileMask { .. }
            ) || directive.excludes_rule(rule_id))
    })
}

/// Run built-in rules **and** any DSL-compiled rules in a single pass.
pub fn run_all_with_dsl(ctx: &ErcContext, dsl_rules: &[EvalFn]) -> Vec<Diagnostic> {
    run_all_with_dsl_and_config(ctx, dsl_rules, &ErcConfig::default())
}

/// [`run_all_with_dsl`] with an explicit [`ErcConfig`].
pub fn run_all_with_dsl_and_config(
    ctx: &ErcContext,
    dsl_rules: &[EvalFn],
    config: &ErcConfig,
) -> Vec<Diagnostic> {
    let mut out = run_all_with_config(ctx, config);
    let mut dsl = Vec::new();
    for rule in dsl_rules {
        dsl.extend(rule(ctx));
    }
    drop_suppressed(ctx, &mut dsl);
    out.extend(dsl);
    out
}
