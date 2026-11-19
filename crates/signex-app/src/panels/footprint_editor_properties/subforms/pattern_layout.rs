//! Where the pattern drawing puts each instance and what number it shows
//! (#126). The expressions are read with the sketch's own parser and
//! evaluator and the parameter table; the bake in `signex-bake::array`
//! stays the authority on the baked pads, this only draws a preview.
//! Gaps against the bake are listed in docs/FOOTPRINT_SKETCH.md, "Role
//! sub-forms".

use std::collections::BTreeMap;

use signex_sketch::array::{NumberingScheme, bga_row_letter};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::parameter::{ParameterTable, resolve_quantities};
use signex_sketch::unit::{Quantity, UnitFamily};

use super::super::super::{ArrayKindSummary, ArraySummary};

/// Most instances the drawing draws one by one.
pub(super) const MAX_DRAWN: usize = 256;

/// One instance of the pattern.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Instance {
    /// Index along the pattern (`i`) and, for a grid, the row (`j`).
    pub i: usize,
    pub j: usize,
    /// Offset from the source in mm.
    pub offset_mm: [f64; 2],
    /// The pad number the numbering scheme gives it.
    pub label: String,
    /// Dropped by the mask or the per-instance list.
    pub is_skipped: bool,
}

/// What the drawing can show of a pattern.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Layout {
    Instances(Vec<Instance>),
    /// A count, spacing or mask does not evaluate.
    Unresolved,
    /// More instances than the drawing draws.
    TooMany(usize),
}

impl Layout {
    /// The instances the pattern keeps, when they are known.
    pub fn kept(&self) -> Option<usize> {
        match self {
            Layout::Instances(all) => Some(all.iter().filter(|c| !c.is_skipped).count()),
            Layout::Unresolved | Layout::TooMany(_) => None,
        }
    }
}

fn strip_eq(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(str::trim_start).unwrap_or(s)
}

fn context(params: &[(String, String)]) -> Option<EvalContext> {
    let table = ParameterTable(params.iter().cloned().collect());
    let params: BTreeMap<String, ExprNode> = resolve_quantities(&table)
        .ok()?
        .into_iter()
        .map(|(name, q)| (name, ExprNode::Literal(q)))
        .collect();
    Some(EvalContext {
        params,
        array_index: Some((0, 0)),
    })
}

fn quantity(src: &str, ctx: &EvalContext) -> Option<Quantity> {
    let ast = parse(strip_eq(src)).ok()?;
    eval(&ast, ctx).ok()
}

/// A count: a plain number, rounded, at least one.
fn count(src: &str, ctx: &EvalContext) -> Option<usize> {
    let q = quantity(src, ctx)?;
    if q.unit.family() != UnitFamily::Count {
        return None;
    }
    let n = q.value.round();
    (1.0..=usize::MAX as f64).contains(&n).then_some(n as usize)
}

/// A length in mm; a plain number reads as mm.
fn length_mm(src: &str, ctx: &EvalContext) -> Option<f64> {
    let read = |ctx: &EvalContext| {
        let q = quantity(src, ctx)?;
        q.as_mm().ok().or_else(|| q.as_count().ok())
    };
    read(ctx).or_else(|| read(&ctx.with_unitless_as_mm()))
}

/// The per-instance mask: `None` keeps every instance.
struct Mask {
    expr: Option<ExprNode>,
}

impl Mask {
    /// `None` when the mask text does not parse.
    fn new(src: &str) -> Option<Self> {
        let src = src.trim();
        if src.is_empty() {
            return Some(Self { expr: None });
        }
        parse(strip_eq(src))
            .ok()
            .map(|ast| Self { expr: Some(ast) })
    }

    /// False drops the instance; an evaluation error keeps it.
    fn keeps(&self, ctx: &EvalContext) -> bool {
        let Some(ast) = &self.expr else {
            return true;
        };
        match eval(ast, ctx) {
            Ok(q) => q.value.abs() >= 1e-9,
            Err(_) => true,
        }
    }
}

/// `start + i * step`, rounded; `None` when an expression fails.
fn linear_number(start: &str, step: &str, i: usize, ctx: &mut EvalContext) -> Option<String> {
    ctx.array_index = Some((i, 0));
    let start = quantity(start, ctx)?.value;
    let step = quantity(step, ctx)?.value;
    Some(format!("{}", (start + i as f64 * step).round() as i64))
}

/// The number of instance `i` of a one-dimensional pattern.
fn number_1d(scheme: &NumberingScheme, i: usize, ctx: &mut EvalContext) -> String {
    match scheme {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => linear_number(start_expr, step_expr, i, ctx).unwrap_or_else(|| i.to_string()),
        NumberingScheme::BgaRowCol { .. } => (i + 1).to_string(),
        NumberingScheme::Explicit { names } => {
            names.get(i).cloned().unwrap_or_else(|| i.to_string())
        }
    }
}

/// The number of cell `(i, j)` of an `nx`-wide grid.
fn number_2d(
    scheme: &NumberingScheme,
    i: usize,
    j: usize,
    nx: usize,
    ctx: &mut EvalContext,
) -> String {
    let idx = j * nx + i;
    match scheme {
        NumberingScheme::LinearIncrement {
            start_expr,
            step_expr,
        } => {
            linear_number(start_expr, step_expr, idx, ctx).unwrap_or_else(|| (idx + 1).to_string())
        }
        NumberingScheme::BgaRowCol {
            skip_letters,
            start_row,
            start_col,
        } => format!(
            "{}{}",
            bga_row_letter(j as u32, *skip_letters, *start_row),
            i as u32 + *start_col
        ),
        NumberingScheme::Explicit { names } => {
            names.get(idx).cloned().unwrap_or_else(|| idx.to_string())
        }
    }
}

/// The instances of `pattern`, with `params` the sketch parameter table.
pub(super) fn layout(pattern: &ArraySummary, params: &[(String, String)]) -> Layout {
    let Some(mut ctx) = context(params) else {
        return Layout::Unresolved;
    };
    let scheme = &pattern.numbering_scheme;
    let mut out = Vec::new();
    match &pattern.kind {
        ArrayKindSummary::Linear {
            count_expr,
            dx_expr,
            dy_expr,
        } => {
            let Some(n) = count(count_expr, &ctx) else {
                return Layout::Unresolved;
            };
            if n > MAX_DRAWN {
                return Layout::TooMany(n);
            }
            for i in 0..n {
                ctx.array_index = Some((i, 0));
                let (Some(dx), Some(dy)) = (length_mm(dx_expr, &ctx), length_mm(dy_expr, &ctx))
                else {
                    return Layout::Unresolved;
                };
                out.push(Instance {
                    i,
                    j: 0,
                    offset_mm: [i as f64 * dx, i as f64 * dy],
                    label: number_1d(scheme, i, &mut ctx),
                    is_skipped: false,
                });
            }
        }
        ArrayKindSummary::Grid {
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            mask_expr,
            suppressed_instances,
            ..
        } => {
            let (Some(nx), Some(ny)) = (count(nx_expr, &ctx), count(ny_expr, &ctx)) else {
                return Layout::Unresolved;
            };
            let Some(mask) = Mask::new(mask_expr) else {
                return Layout::Unresolved;
            };
            let total = nx.saturating_mul(ny);
            if total > MAX_DRAWN {
                return Layout::TooMany(total);
            }
            for j in 0..ny {
                for i in 0..nx {
                    ctx.array_index = Some((i, j));
                    let (Some(dx), Some(dy)) = (length_mm(dx_expr, &ctx), length_mm(dy_expr, &ctx))
                    else {
                        return Layout::Unresolved;
                    };
                    let listed = suppressed_instances
                        .iter()
                        .any(|&(si, sj)| si as usize == i && sj as usize == j);
                    let is_skipped = listed || !mask.keeps(&ctx);
                    out.push(Instance {
                        i,
                        j,
                        offset_mm: [i as f64 * dx, j as f64 * dy],
                        label: number_2d(scheme, i, j, nx, &mut ctx),
                        is_skipped,
                    });
                }
            }
        }
        ArrayKindSummary::Polar {
            count_expr,
            sweep_angle_expr,
            center_position_mm,
            mask_expr,
            suppressed_instances,
            ..
        } => {
            let (Some(n), Some([cx, cy]), Some([sx, sy])) = (
                count(count_expr, &ctx),
                *center_position_mm,
                pattern.source_position_mm,
            ) else {
                return Layout::Unresolved;
            };
            let Some(sweep) = quantity(sweep_angle_expr, &ctx)
                .filter(|q| q.unit.family() == UnitFamily::Angle)
                .map(|q| q.value)
            else {
                return Layout::Unresolved;
            };
            let Some(mask) = Mask::new(mask_expr) else {
                return Layout::Unresolved;
            };
            if n > MAX_DRAWN {
                return Layout::TooMany(n);
            }
            let (rx, ry) = (sx - cx, sy - cy);
            for i in 0..n {
                let theta = i as f64 * sweep / n as f64;
                let (sin, cos) = theta.sin_cos();
                let x = cx + rx * cos - ry * sin;
                let y = cy + rx * sin + ry * cos;
                ctx.array_index = Some((i, 0));
                let is_skipped = suppressed_instances.contains(&(i as u32)) || !mask.keeps(&ctx);
                out.push(Instance {
                    i,
                    j: 0,
                    offset_mm: [x - sx, y - sy],
                    label: number_1d(scheme, i, &mut ctx),
                    is_skipped,
                });
            }
        }
    }
    Layout::Instances(out)
}
