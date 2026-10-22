//! Per-instance pad-numbering schemes for sketch arrays — both
//! the 1D  (Linear) and the 2D
//!  (Grid). Polar reuses the 1D form.

use std::collections::BTreeMap;

use signex_sketch::array::NumberingScheme;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;

/// Resolve the pad number for the i-th instance of a linear array.
pub(super) fn derive_pad_number(
    numbering: &NumberingScheme,
    i: usize,
    params_ast: &BTreeMap<String, ExprNode>,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => match linear_increment_number(start_expr, step_expr, i, params_ast) {
            Ok(number) => number,
            Err(why) => {
                record_numbering_warning(
                    warnings,
                    format!(
                        "linear array source {source}: {why}; members are numbered 0, 1, 2, … instead of the authored scheme",
                    ),
                );
                format!("{}", i)
            }
        },
        NumberingScheme::BgaRowCol { .. } => {
            warnings.push(format!(
                "linear array source {source}: BgaRowCol numbering not meaningful on a 1D Linear array — falling back to LinearIncrement defaults",
                ));
            // Fall back to LinearIncrement default: 1-based, step 1.
            format!("{}", i + 1)
        }
        NumberingScheme::Explicit { names } => {
            if i < names.len() {
                names[i].clone()
            } else {
                warnings.push(format!(
                    "linear array source {source}: Explicit numbering ran out of names at i={i}; using fallback \"{i}\"",
                    ));
                format!("{}", i)
            }
        }
    }
}

/// `LinearIncrement` — `start + i * step`, both rounded to integer
/// after canonical evaluation.
///
/// GH #599 — on an expression error this returns the offending
/// expression and the reason instead of `None`. The caller still
/// falls back to a default number rather than aborting the whole
/// array bake, but it can now say which expression it gave up on:
/// pad and instance numbers are what the netlist binds to, so a
/// typo'd designator expression that silently renumbers the array
/// `0, 1, 2, …` is not a cosmetic failure.
pub(super) fn linear_increment_number(
    start_expr: &str,
    step_expr: &str,
    i: usize,
    params_ast: &BTreeMap<String, ExprNode>,
) -> Result<String, String> {
    let ctx = EvalContext {
        params: params_ast.clone(),
        array_index: Some((i, 0)),
    };
    let start = eval_numbering_expr("start", start_expr, &ctx)?;
    let step = eval_numbering_expr("step", step_expr, &ctx)?;
    let n = (start + i as f64 * step).round() as i64;
    Ok(format!("{}", n))
}

/// Parse + evaluate one numbering expression, naming both the field
/// (`start` / `step`) and the authored source text in the error.
fn eval_numbering_expr(field: &str, src: &str, ctx: &EvalContext) -> Result<f64, String> {
    let ast = parse(strip_eq_prefix(src))
        .map_err(|e| format!("{field} expression \"{src}\" did not parse: {e}"))?;
    let value = eval(&ast, ctx)
        .map_err(|e| format!("{field} expression \"{src}\" did not evaluate: {e}"))?
        .value;
    Ok(value)
}

/// Push a numbering warning and mirror it to the log, skipping
/// duplicates.
///
/// The start / step expressions belong to the array, not to the
/// individual member, so an unparseable one fails identically for
/// every member. Deduplicating against the warnings already recorded
/// for this bake turns what would be one record per array member
/// into one record per array.
fn record_numbering_warning(warnings: &mut Vec<String>, message: String) {
    if warnings.contains(&message) {
        return;
    }
    tracing::warn!(
        target: "signex::bake",
        warning = %message,
        "array numbering expression failed; members were renumbered",
    );
    warnings.push(message);
}

/// Strip the optional Altium-style leading `=` and surrounding
/// whitespace so authored expressions like `= count` parse cleanly.
pub(super) fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

/// v0.22 Phase B3 — bake `ArrayKind::Grid`. Walks `(i, j)` with
/// `i in 0..nx` and `j in 0..ny`, per-instance offset
/// `(i * dx, j * dy)` from the source. Optional `depopulation` is a
/// boolean expression evaluated per cell — `false` skips the cell
/// without breaking the parametric chain.
/// 2D companion to `derive_pad_number` — `BgaRowCol` is meaningful
/// here, falling back to a row-major linear count for `LinearIncrement`
/// and looking up `names[j*nx + i]` for `Explicit`.
pub(super) fn derive_pad_number_2d(
    numbering: &NumberingScheme,
    i: usize,
    j: usize,
    nx: usize,
    params_ast: &BTreeMap<String, ExprNode>,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    use signex_sketch::array::bga_row_letter;
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => {
            // Row-major: (j, i) → idx = j*nx + i.
            let idx = j * nx + i;
            match linear_increment_number(start_expr, step_expr, idx, params_ast) {
                Ok(number) => number,
                Err(why) => {
                    record_numbering_warning(
                        warnings,
                        format!(
                            "grid array source {source}: {why}; members are numbered 1, 2, 3, … row-major instead of the authored scheme",
                        ),
                    );
                    format!("{}", idx + 1)
                }
            }
        }
        NumberingScheme::BgaRowCol {
            skip_letters,
            start_row,
            start_col,
        } => {
            let row = bga_row_letter(j as u32, *skip_letters, *start_row);
            let col = (i as u32) + *start_col;
            format!("{row}{col}")
        }
        NumberingScheme::Explicit { names } => {
            let idx = j * nx + i;
            if idx < names.len() {
                names[idx].clone()
            } else {
                warnings.push(format!(
                    "grid array source {source}: Explicit numbering ran out of names at ({i}, {j}); using fallback \"{idx}\"",
                ));
                format!("{}", idx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn broken_scheme() -> NumberingScheme {
        // `1 +` is a syntactically invalid expression — the realistic
        // trigger is a typo in the authored designator scheme.
        NumberingScheme::LinearIncrement {
            start_expr: "1 +".to_string(),
            step_expr: "1".to_string(),
        }
    }

    #[test]
    fn linear_increment_expression_error_is_reported_not_swallowed() {
        // GH #599 — an unparseable start expression used to renumber
        // the array 0, 1, 2, … with `warnings` left empty, so an
        // array whose pad numbers the netlist binds to came out wrong
        // and the bake reported success.
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let params = BTreeMap::new();

        let number = derive_pad_number(&broken_scheme(), 0, &params, &mut warnings, source);

        assert_eq!(number, "0", "fallback numbering is unchanged");
        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(
            warnings[0].contains("1 +"),
            "the warning must name the offending expression, got {warnings:?}"
        );
        assert!(
            warnings[0].contains("start"),
            "the warning must name which expression failed, got {warnings:?}"
        );
    }

    #[test]
    fn linear_increment_expression_error_is_reported_once_per_array() {
        // The start / step expressions belong to the array, so every
        // member fails identically. One record per array, not one per
        // member.
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let params = BTreeMap::new();
        let scheme = broken_scheme();

        for i in 0..5 {
            let number = derive_pad_number(&scheme, i, &params, &mut warnings, source);
            assert_eq!(number, format!("{i}"));
        }

        assert_eq!(
            warnings.len(),
            1,
            "five members must not produce five identical warnings, got {warnings:?}"
        );
    }

    #[test]
    fn grid_linear_increment_expression_error_is_reported() {
        // The 2D companion swallowed the same error, with a 1-based
        // row-major fallback.
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let params = BTreeMap::new();

        let number =
            derive_pad_number_2d(&broken_scheme(), 1, 0, 2, &params, &mut warnings, source);

        assert_eq!(number, "2", "row-major fallback numbering is unchanged");
        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(
            warnings[0].contains("1 +"),
            "the warning must name the offending expression, got {warnings:?}"
        );
    }

    #[test]
    fn linear_increment_valid_expression_stays_silent() {
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let params = BTreeMap::new();
        let scheme = NumberingScheme::LinearIncrement {
            start_expr: "= 10".to_string(),
            step_expr: "2".to_string(),
        };

        let number = derive_pad_number(&scheme, 3, &params, &mut warnings, source);

        assert_eq!(number, "16", "start 10 + i(3) * step 2");
        assert!(warnings.is_empty(), "no warning expected, got {warnings:?}");
    }
}
