//! Sketch-array expansion — bakes all three `ArrayKind` variants.
//!
//! Walks every `Array` and live mirror in the sketch and produces baked
//! [`signex_library::primitive::footprint::Pad`]s by re-using the
//! per-pad bake body from `crate::pad::bake_one_pad`. Every pattern, plain
//! or linked, places its instances through one walk (`links`, with each
//! array's own layout in `layout`); the Properties preview walks the same
//! counts and steps without a solve (`walk`).
//!
//! Cleanroom: derived from first principles + the Phase 4 expression
//! machinery. No third-party constraint-solver, footprint-generator,
//! or numerical-library source consulted.
//!
//! Numbering:
//! - `LinearIncrement { start, step }` — pad number =
//!   `start + i * step` rounded to integer.
//! - `BgaRowCol { .. }` on a 1D Linear — warns (BGA on Linear is not
//!   semantically meaningful) and falls back to a default 1-based
//!   `LinearIncrement`.
//! - `Explicit { names }` — uses `names[i]` if present; warns and
//!   falls back to `format!("{i}")` otherwise.

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

mod instance_at;
mod kind_switch;
#[cfg(test)]
mod kind_switch_tests;
mod layout;
mod links;
#[cfg(test)]
mod links_tests;
#[cfg(test)]
mod literal_context_tests;
mod numbering;
#[cfg(test)]
mod one_walk_tests;
mod overrides;
mod place;
mod placements;
#[cfg(test)]
mod polar_tests;
mod preview;
mod slots;
mod walk;

pub use instance_at::instance_at;
pub use kind_switch::{
    KindSwitchRefused, KindSwitched, KindTarget, PatternKind, remove_unused_point,
    switch_pattern_kind,
};
pub use numbering::{
    NumberMaskRefusal, instances_numbered, number_mask_entries, suppressed_numbers,
};
pub(crate) use placements::{
    InstanceBudget, PlacesRefused, SourcePlace, is_patterned, source_places,
};
pub use preview::{ArrayPreview, InstancePreview, preview_array};
pub use slots::{PatternSlot, pattern_slots};

/// How a bake warning names a pattern source: its kind and where it sits,
/// not its id (mockup row 179e, #2772).
fn source_name(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    source: signex_sketch::id::SketchEntityId,
) -> String {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::solver::state::point_xy;
    let Some(entity) = sketch.entities.iter().find(|e| e.id == source) else {
        return format!("entity {source}");
    };
    let (kind, at) = match entity.kind {
        EntityKind::Point { .. } => ("point", source),
        EntityKind::Line { start, .. } => ("line", start),
        EntityKind::Arc { center, .. } => ("arc", center),
        EntityKind::Circle { center, .. } => ("circle", center),
    };
    match point_xy(at, &solve.result.state, &solve.result.index, sketch) {
        Some((x, y)) => format!("the {kind} at ({x:.3}, {y:.3}) mm"),
        None => format!("a {kind}"),
    }
}

/// The warning for a pattern whose source carries no pad: a footprint
/// pattern repeats pads only. `what` is "linear array", "pattern", ….
fn not_a_pad(
    what: &str,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    source: signex_sketch::id::SketchEntityId,
) -> String {
    let name = source_name(sketch, solve, source);
    let skipped = what.rsplit(' ').next().unwrap_or(what);
    format!("{what} of {name}: the source is not a pad — {skipped} skipped")
}

/// Most pad instances one array may expand to. Array counts come from
/// user / file expressions and the bake runs on the editor's update
/// thread, so an unbounded count (`=100000` by `=100000`) would freeze
/// and then exhaust memory. Real packages (a 40x40 BGA) sit far below it.
pub const MAX_ARRAY_INSTANCES: usize = 10_000;

/// Whether an array expanding to `instances` pads (`None` when the count
/// overflowed `usize`) stays within [`MAX_ARRAY_INSTANCES`]. Pushes a bake
/// warning and returns `false` when it does not, so the caller skips the
/// array.
fn within_instance_cap(
    label: &str,
    source: signex_sketch::id::SketchEntityId,
    instances: Option<usize>,
    warnings: &mut Vec<String>,
) -> bool {
    match instances {
        Some(n) if n <= MAX_ARRAY_INSTANCES => true,
        Some(n) => {
            warnings.push(format!(
                "{label} array source {source}: {n} instances exceeds the limit of \
                 {MAX_ARRAY_INSTANCES} — array skipped"
            ));
            false
        }
        None => {
            warnings.push(format!(
                "{label} array source {source}: instance count overflows, limit is \
                 {MAX_ARRAY_INSTANCES} instances — array skipped"
            ));
            false
        }
    }
}

/// Evaluate a count expression to a rounded integer, requiring a plain
/// number. A length or angle typed as a count (`5mm`, `90deg`) pushes a
/// bake warning and returns `None` so the caller skips the array, rather
/// than silently baking `value.round()` of a canonical mm / rad scalar.
fn eval_count(
    label: &str,
    field: &str,
    source: signex_sketch::id::SketchEntityId,
    ast: &signex_expr::ExprNode,
    ctx: &signex_sketch::expr::eval::EvalContext,
    warnings: &mut Vec<String>,
) -> Result<Option<i64>, signex_sketch::SketchError> {
    let q = signex_sketch::expr::eval::eval(ast, ctx).map_err(signex_sketch::SketchError::Expr)?;
    if q.unit.family() != signex_expr::UnitFamily::Count {
        warnings.push(format!(
            "{label} array source {source}: {field} did not resolve to a plain number — array skipped"
        ));
        return Ok(None);
    }
    Ok(Some(q.value.round() as i64))
}

pub(crate) use links::bake_pattern;

/// Walk every [`signex_sketch::array::Array`] and live mirror and append
/// the pads they place to `out`.
pub fn bake_arrays(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let params_ast = crate::params::params_ast(sketch, params_canonical);
    for array in &sketch.arrays {
        bake_pattern(array.id, sketch, solve, &params_ast, out, warnings)?;
    }
    links::bake_mirrors(sketch, solve, &params_ast, out, warnings)?;
    Ok(())
}
