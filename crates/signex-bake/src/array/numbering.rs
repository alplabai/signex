//! Per-instance pad-numbering schemes for sketch arrays — both
//! the 1D  (Linear) and the 2D
//!  (Grid). Polar reuses the 1D form.

use signex_expr::parse;
use signex_sketch::array::NumberingScheme;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::id::SketchEntityId;

use super::preview::ArrayPreview;
use crate::eval_util::strip_eq_prefix;

/// Resolve the pad number for the i-th instance of a linear array.
pub(super) fn derive_pad_number(
    numbering: &NumberingScheme,
    i: usize,
    ctx: &mut EvalContext,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
            ..
        } => match linear_increment_number(start_expr, step_expr, i, ctx) {
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
    ctx: &mut EvalContext,
) -> Result<String, String> {
    ctx.array_index = Some((i, 0));
    let start = eval_numbering_expr("start", start_expr, ctx)?;
    let step = eval_numbering_expr("step", step_expr, ctx)?;
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
    ctx: &mut EvalContext,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    use signex_sketch::array::bga_row_letter;
    match numbering {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
            ..
        } => {
            // Row-major: (j, i) → idx = j*nx + i.
            let idx = j * nx + i;
            match linear_increment_number(start_expr, step_expr, idx, ctx) {
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
            if !signex_sketch::array::is_valid_bga_start_row(*skip_letters, *start_row) {
                record_numbering_warning(
                    warnings,
                    format!(
                        "grid array source {source}: BGA start_row '{start_row}' is not in the row alphabet; rows start at 'A' instead",
                    ),
                );
            }
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

/// The number of grid cell `(i, j)` under the scheme's sweep (mockup row
/// 126). `ny` is `None` for a pattern of a pattern, which numbers row by row.
#[allow(
    clippy::too_many_arguments,
    reason = "the cell, the grid's size and the numbering context"
)]
pub(super) fn derive_swept_pad_number(
    numbering: &NumberingScheme,
    i: usize,
    j: usize,
    nx: usize,
    ny: Option<usize>,
    ctx: &mut EvalContext,
    warnings: &mut Vec<String>,
    source: SketchEntityId,
) -> String {
    let (i, j) = match (numbering, ny) {
        (NumberingScheme::LinearIncrement { sweep, .. }, Some(ny)) if nx > 0 => {
            let place = sweep.sequence(i, j, nx, ny);
            (place % nx, place / nx)
        }
        _ => (i, j),
    };
    derive_pad_number_2d(numbering, i, j, nx, ctx, warnings, source)
}

/// Why a pattern's Numbering Mask text is not taken (mockup row 126).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberMaskRefusal {
    /// The pattern's instances are not known: a count, spacing or mask does
    /// not evaluate, or there are more than the preview walks.
    Unresolved,
    /// No instance of the pattern has this number.
    NotInPattern(String),
}

/// The pad numbers a Numbering Mask lists, split at commas and spaces.
pub fn number_mask_entries(text: &str) -> Vec<&str> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// The instances `(i, j)` a Numbering Mask skips: each instance whose number
/// it lists, letters matched in either case. A blank mask skips none.
pub fn instances_numbered(
    preview: &ArrayPreview,
    mask: &str,
) -> Result<Vec<(u32, u32)>, NumberMaskRefusal> {
    let entries = number_mask_entries(mask);
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let ArrayPreview::Instances(all) = preview else {
        return Err(NumberMaskRefusal::Unresolved);
    };
    let mut cells = Vec::new();
    for entry in entries {
        let mut is_found = false;
        for instance in all.iter().filter(|c| c.number.eq_ignore_ascii_case(entry)) {
            is_found = true;
            let cell = (instance.i as u32, instance.j as u32);
            if !cells.contains(&cell) {
                cells.push(cell);
            }
        }
        if !is_found {
            return Err(NumberMaskRefusal::NotInPattern(entry.to_owned()));
        }
    }
    Ok(cells)
}

/// The numbers of the instances on `suppressed`, in the pattern's order:
/// what the Numbering Mask shows.
pub fn suppressed_numbers(preview: &ArrayPreview, suppressed: &[(u32, u32)]) -> Vec<String> {
    let ArrayPreview::Instances(all) = preview else {
        return Vec::new();
    };
    all.iter()
        .filter(|c| suppressed.contains(&(c.i as u32, c.j as u32)))
        .map(|c| c.number.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::preview::InstancePreview;
    use super::*;
    use std::collections::BTreeMap;

    fn instance(i: usize, j: usize, number: &str) -> InstancePreview {
        InstancePreview {
            i,
            j,
            offset_mm: [i as f64, j as f64],
            number: number.into(),
            is_skipped: false,
        }
    }

    /// A 2 × 2 grid numbered A1, A2, B1, B2.
    fn bga() -> ArrayPreview {
        ArrayPreview::Instances(vec![
            instance(0, 0, "A1"),
            instance(1, 0, "A2"),
            instance(0, 1, "B1"),
            instance(1, 1, "B2"),
        ])
    }

    /// Mockup row 126: the mask lists pad numbers ("4,5" or "A1,B4"), split at
    /// commas and spaces, and skips the instances carrying them.
    #[test]
    fn a_number_mask_skips_the_listed_pads() {
        assert_eq!(number_mask_entries(" A1, b2 ,,"), ["A1", "b2"]);
        assert_eq!(
            instances_numbered(&bga(), "A1, b2"),
            Ok(vec![(0, 0), (1, 1)])
        );
        assert_eq!(instances_numbered(&bga(), "B1 B1"), Ok(vec![(0, 1)]));
        assert_eq!(instances_numbered(&bga(), "  "), Ok(Vec::new()));
        assert_eq!(
            suppressed_numbers(&bga(), &[(1, 1), (0, 0)]),
            ["A1", "B2"],
            "in the pattern's order"
        );
    }

    #[test]
    fn a_number_the_pattern_does_not_have_is_refused() {
        assert_eq!(
            instances_numbered(&bga(), "A1,B4"),
            Err(NumberMaskRefusal::NotInPattern("B4".into()))
        );
        assert_eq!(
            instances_numbered(&ArrayPreview::Unresolved, "1"),
            Err(NumberMaskRefusal::Unresolved)
        );
        assert_eq!(
            instances_numbered(&ArrayPreview::TooMany(900), ""),
            Ok(Vec::new())
        );
    }

    /// Mockup row 126: a 2 × 4 grid in each sweep, read column by column.
    #[test]
    fn a_grid_sweep_orders_the_numbers() {
        use signex_sketch::array::GridSweep;
        let numbers = |sweep| {
            let scheme = NumberingScheme::LinearIncrement {
                start_expr: "1".into(),
                step_expr: "1".into(),
                sweep,
            };
            let (mut ctx, mut warnings) = (test_ctx(), Vec::new());
            let source = SketchEntityId::new();
            let mut out = Vec::new();
            for i in 0..2 {
                for j in 0..4 {
                    out.push(derive_swept_pad_number(
                        &scheme,
                        i,
                        j,
                        2,
                        Some(4),
                        &mut ctx,
                        &mut warnings,
                        source,
                    ));
                }
            }
            assert!(warnings.is_empty(), "{warnings:?}");
            out
        };
        assert_eq!(
            numbers(GridSweep::RowByRow),
            ["1", "3", "5", "7", "2", "4", "6", "8"]
        );
        assert_eq!(
            numbers(GridSweep::ColumnByColumn),
            ["1", "2", "3", "4", "5", "6", "7", "8"]
        );
        assert_eq!(
            numbers(GridSweep::DownThenUp),
            ["1", "2", "3", "4", "8", "7", "6", "5"]
        );
    }

    /// A mirrored instance sits past the last row and keeps its place after
    /// the grid's own numbers, in every sweep.
    #[test]
    fn an_instance_past_the_last_row_numbers_after_the_grid() {
        use signex_sketch::array::GridSweep;
        for sweep in GridSweep::ALL {
            assert_eq!(sweep.sequence(1, 4, 2, 4), 9, "{sweep:?}");
        }
    }

    fn test_ctx() -> EvalContext {
        EvalContext {
            params: BTreeMap::new(),
            array_index: None,
        }
    }

    fn broken_scheme() -> NumberingScheme {
        // `1 +` is a syntactically invalid expression — the realistic
        // trigger is a typo in the authored designator scheme.
        NumberingScheme::LinearIncrement {
            start_expr: "1 +".to_string(),
            step_expr: "1".to_string(),
            sweep: Default::default(),
        }
    }

    #[test]
    fn invalid_bga_start_row_is_reported() {
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let mut ctx = test_ctx();
        let scheme = NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'I',
            start_col: 1,
        };
        let n = derive_pad_number_2d(&scheme, 0, 0, 2, &mut ctx, &mut warnings, source);
        assert_eq!(n, "A1");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("start_row 'I'"), "{warnings:?}");
        let ok = NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'B',
            start_col: 1,
        };
        let mut w2 = Vec::new();
        derive_pad_number_2d(&ok, 0, 0, 2, &mut ctx, &mut w2, source);
        assert!(w2.is_empty());
    }

    #[test]
    fn linear_increment_expression_error_is_reported_not_swallowed() {
        // GH #599 — an unparseable start expression used to renumber
        // the array 0, 1, 2, … with `warnings` left empty, so an
        // array whose pad numbers the netlist binds to came out wrong
        // and the bake reported success.
        let mut warnings = Vec::new();
        let source = SketchEntityId::new();
        let mut ctx = test_ctx();

        let number = derive_pad_number(&broken_scheme(), 0, &mut ctx, &mut warnings, source);

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
        let mut ctx = test_ctx();
        let scheme = broken_scheme();

        for i in 0..5 {
            let number = derive_pad_number(&scheme, i, &mut ctx, &mut warnings, source);
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
        let mut ctx = test_ctx();

        let number =
            derive_pad_number_2d(&broken_scheme(), 1, 0, 2, &mut ctx, &mut warnings, source);

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
        let mut ctx = test_ctx();
        let scheme = NumberingScheme::LinearIncrement {
            start_expr: "= 10".to_string(),
            step_expr: "2".to_string(),
            sweep: Default::default(),
        };

        let number = derive_pad_number(&scheme, 3, &mut ctx, &mut warnings, source);

        assert_eq!(number, "16", "start 10 + i(3) * step 2");
        assert!(warnings.is_empty(), "no warning expected, got {warnings:?}");
    }
}
