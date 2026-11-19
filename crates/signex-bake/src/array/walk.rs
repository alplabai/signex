//! The instances of one array: where each sits relative to the source,
//! which number it gets and whether depopulation drops it. The bake
//! ([`super::bake_one_array`]) and the Properties preview
//! ([`super::preview`]) both walk an array through here, so the preview
//! shows the numbers and positions the bake produces.

use std::collections::BTreeMap;

use signex_sketch::SketchError;
use signex_sketch::array::{GridDepopulation, NumberingScheme};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;

use super::numbering::{derive_pad_number, derive_pad_number_2d};
use crate::eval_util::strip_eq_prefix;

/// One instance of an array.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Instance {
    /// Index along the array (`i`) and, for a grid, the row (`j`).
    pub i: usize,
    pub j: usize,
    /// Offset from the source in mm. `None` only for a dropped instance
    /// whose spacing does not evaluate.
    pub offset_mm: Option<[f64; 2]>,
    /// The pad number the numbering scheme gives it.
    pub number: String,
    /// Dropped by the depopulation mask or the suppressed list.
    pub is_skipped: bool,
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Walked {
    /// Every instance was visited.
    Done,
    /// A count or sweep is not usable; the warning is pushed.
    Refused,
    /// More instances than the walk's limit (`None` on overflow); nothing
    /// was visited.
    TooMany(Option<usize>),
}

/// What every walk shares.
pub(super) struct Walk<'a> {
    pub source: SketchEntityId,
    pub numbering: &'a NumberingScheme,
    pub params_ast: &'a BTreeMap<String, ExprNode>,
    /// Most instances the walk visits.
    pub limit: usize,
    /// Visit dropped instances too (the preview draws them). The bake
    /// passes `false` and never evaluates anything for a dropped cell.
    pub visit_skipped: bool,
}

/// The visitor gets each instance, the context bound to its index and the
/// walk's warnings (so the bake's warnings keep their order).
pub(super) type Visit<'v> =
    dyn FnMut(Instance, &EvalContext, &mut Vec<String>) -> Result<(), SketchError> + 'v;

impl Walk<'_> {
    fn context(&self) -> EvalContext {
        EvalContext {
            params: self.params_ast.clone(),
            array_index: Some((0, 0)),
        }
    }

    /// `count` instances stepped by (`dx`, `dy`).
    pub fn linear(
        &self,
        count_expr: &str,
        dx_expr: &str,
        dy_expr: &str,
        warnings: &mut Vec<String>,
        visit: &mut Visit<'_>,
    ) -> Result<Walked, SketchError> {
        let source = self.source;
        // Parse once; only `array_index` changes between instances.
        let count_ast = parse(strip_eq_prefix(count_expr)).map_err(SketchError::Expr)?;
        let dx_ast = parse(strip_eq_prefix(dx_expr)).map_err(SketchError::Expr)?;
        let dy_ast = parse(strip_eq_prefix(dy_expr)).map_err(SketchError::Expr)?;
        let mut ctx = self.context();
        let Some(count) =
            super::eval_count("linear", "count_expr", source, &count_ast, &ctx, warnings)?
        else {
            return Ok(Walked::Refused);
        };
        if count <= 0 {
            warnings.push(format!(
                "linear array source {source}: count_expr resolved to {count} — array skipped"
            ));
            return Ok(Walked::Refused);
        }
        let count = count as usize;
        if count > self.limit {
            return Ok(Walked::TooMany(Some(count)));
        }
        for i in 0..count {
            ctx.array_index = Some((i, 0));
            let dx_mm = crate::params::eval_length_mm(&dx_ast, &ctx)?;
            let dy_mm = crate::params::eval_length_mm(&dy_ast, &ctx)?;
            let number = derive_pad_number(self.numbering, i, &mut ctx, warnings, source);
            ctx.array_index = Some((i, 0));
            let instance = Instance {
                i,
                j: 0,
                offset_mm: Some([i as f64 * dx_mm, i as f64 * dy_mm]),
                number,
                is_skipped: false,
            };
            visit(instance, &ctx, warnings)?;
        }
        Ok(Walked::Done)
    }

    /// `nx` × `ny` cells stepped by (`dx`, `dy`), row by row.
    #[allow(
        clippy::too_many_arguments,
        reason = "a grid is four expressions, its depopulation and the visitor"
    )]
    pub fn grid(
        &self,
        nx_expr: &str,
        ny_expr: &str,
        dx_expr: &str,
        dy_expr: &str,
        depopulation: Option<&GridDepopulation>,
        warnings: &mut Vec<String>,
        visit: &mut Visit<'_>,
    ) -> Result<Walked, SketchError> {
        let source = self.source;
        let nx_ast = parse(strip_eq_prefix(nx_expr)).map_err(SketchError::Expr)?;
        let ny_ast = parse(strip_eq_prefix(ny_expr)).map_err(SketchError::Expr)?;
        let dx_ast = parse(strip_eq_prefix(dx_expr)).map_err(SketchError::Expr)?;
        let dy_ast = parse(strip_eq_prefix(dy_expr)).map_err(SketchError::Expr)?;
        let mask_ast = mask_ast(depopulation)?;
        let mut ctx = self.context();
        let Some(nx) = super::eval_count("grid", "nx_expr", source, &nx_ast, &ctx, warnings)?
        else {
            return Ok(Walked::Refused);
        };
        let Some(ny) = super::eval_count("grid", "ny_expr", source, &ny_ast, &ctx, warnings)?
        else {
            return Ok(Walked::Refused);
        };
        if nx <= 0 || ny <= 0 {
            warnings.push(format!(
                "grid array source {source}: nx={nx}, ny={ny} — array skipped"
            ));
            return Ok(Walked::Refused);
        }
        let (nx, ny) = (nx as usize, ny as usize);
        match nx.checked_mul(ny) {
            Some(total) if total <= self.limit => {}
            total => return Ok(Walked::TooMany(total)),
        }
        for j in 0..ny {
            for i in 0..nx {
                ctx.array_index = Some((i, j));
                let listed = is_suppressed(depopulation, i, j);
                let is_skipped = listed
                    || !mask_keeps(
                        mask_ast.as_ref(),
                        &ctx,
                        warnings,
                        || {
                            format!(
                                "grid array source {source}: depopulation eval failed at ({i}, {j})"
                            )
                        },
                        "keeping cell",
                    );
                if is_skipped && !self.visit_skipped {
                    continue;
                }
                let offset_mm = if is_skipped {
                    let dx = crate::params::eval_length_mm(&dx_ast, &ctx).ok();
                    let dy = crate::params::eval_length_mm(&dy_ast, &ctx).ok();
                    dx.zip(dy).map(|(dx, dy)| [i as f64 * dx, j as f64 * dy])
                } else {
                    let dx = crate::params::eval_length_mm(&dx_ast, &ctx)?;
                    let dy = crate::params::eval_length_mm(&dy_ast, &ctx)?;
                    Some([i as f64 * dx, j as f64 * dy])
                };
                // A dropped cell's number never reaches the bake, so its
                // numbering warnings are not the bake's.
                let mut dropped_warnings = Vec::new();
                let number_warnings = if is_skipped {
                    &mut dropped_warnings
                } else {
                    &mut *warnings
                };
                let number = derive_pad_number_2d(
                    self.numbering,
                    i,
                    j,
                    nx,
                    &mut ctx,
                    number_warnings,
                    source,
                );
                ctx.array_index = Some((i, j));
                let instance = Instance {
                    i,
                    j,
                    offset_mm,
                    number,
                    is_skipped,
                };
                visit(instance, &ctx, warnings)?;
            }
        }
        Ok(Walked::Done)
    }

    /// `count` instances of the source turned about `center_mm`, sweeping
    /// the sweep angle; the source itself is instance 0.
    #[allow(
        clippy::too_many_arguments,
        reason = "a polar array is two points, two expressions, its depopulation and the visitor"
    )]
    pub fn polar(
        &self,
        source_mm: (f64, f64),
        center_mm: (f64, f64),
        count_expr: &str,
        sweep_angle_expr: &str,
        depopulation: Option<&GridDepopulation>,
        warnings: &mut Vec<String>,
        visit: &mut Visit<'_>,
    ) -> Result<Walked, SketchError> {
        let source = self.source;
        let ((sx, sy), (cx, cy)) = (source_mm, center_mm);
        let count_ast = parse(strip_eq_prefix(count_expr)).map_err(SketchError::Expr)?;
        let sweep_ast = parse(strip_eq_prefix(sweep_angle_expr)).map_err(SketchError::Expr)?;
        let mut ctx = self.context();
        let Some(count) =
            super::eval_count("polar", "count_expr", source, &count_ast, &ctx, warnings)?
        else {
            return Ok(Walked::Refused);
        };
        if count <= 0 {
            warnings.push(format!(
                "polar array source {source}: count={count} — array skipped"
            ));
            return Ok(Walked::Refused);
        }
        let count = count as usize;
        if count > self.limit {
            return Ok(Walked::TooMany(Some(count)));
        }
        // The parser resolves `deg` to rad; anything but an angle refuses.
        let sweep_q = eval(&sweep_ast, &ctx).map_err(SketchError::Expr)?;
        if sweep_q.unit.family() != signex_sketch::unit::UnitFamily::Angle {
            warnings.push(format!(
                "polar array source {source}: sweep_angle_expr did not resolve to an angle — array skipped"
            ));
            return Ok(Walked::Refused);
        }
        let sweep_rad = sweep_q.value;
        let mask_ast = mask_ast(depopulation)?;
        let denom = (count as f64).max(1.0);
        let (dx_src, dy_src) = (sx - cx, sy - cy);
        for i in 0..count {
            // `j` is always 0 for a polar array.
            let listed = is_suppressed(depopulation, i, 0);
            let is_skipped = listed
                || mask_ast.as_ref().is_some_and(|ast| {
                    ctx.array_index = Some((i, 0));
                    !mask_keeps(
                        Some(ast),
                        &ctx,
                        warnings,
                        || {
                            format!(
                                "polar array source {source}: depopulation eval failed at i={i}"
                            )
                        },
                        "keeping instance",
                    )
                });
            if is_skipped && !self.visit_skipped {
                continue;
            }
            let theta = (i as f64) * sweep_rad / denom;
            let (sin_t, cos_t) = theta.sin_cos();
            let rx = cx + dx_src * cos_t - dy_src * sin_t;
            let ry = cy + dx_src * sin_t + dy_src * cos_t;
            let mut dropped_warnings = Vec::new();
            let number_warnings = if is_skipped {
                &mut dropped_warnings
            } else {
                &mut *warnings
            };
            let number = derive_pad_number(self.numbering, i, &mut ctx, number_warnings, source);
            ctx.array_index = Some((i, 0));
            let instance = Instance {
                i,
                j: 0,
                // Offset = rotated source − source, so i = 0 is the source.
                offset_mm: Some([rx - sx, ry - sy]),
                number,
                is_skipped,
            };
            visit(instance, &ctx, warnings)?;
        }
        Ok(Walked::Done)
    }
}

/// The depopulation mask, parsed once. An empty mask means "suppressed
/// list only" (the parser rejects empty input).
fn mask_ast(depopulation: Option<&GridDepopulation>) -> Result<Option<ExprNode>, SketchError> {
    depopulation
        .filter(|d| !d.mask_expr.trim().is_empty())
        .map(|d| parse(strip_eq_prefix(&d.mask_expr)))
        .transpose()
        .map_err(SketchError::Expr)
}

/// Whether cell `(i, j)` is in the suppressed list, which drops it
/// independently of the mask.
fn is_suppressed(depopulation: Option<&GridDepopulation>, i: usize, j: usize) -> bool {
    let (i, j) = (i as u32, j as u32);
    depopulation.is_some_and(|d| {
        d.suppressed_instances
            .iter()
            .any(|&(si, sj)| si == i && sj == j)
    })
}

/// The mask keeps a cell while it is non-zero. An evaluation failure keeps
/// the cell and is warned, never silently dropped.
fn mask_keeps(
    mask: Option<&ExprNode>,
    ctx: &EvalContext,
    warnings: &mut Vec<String>,
    what: impl FnOnce() -> String,
    outcome: &str,
) -> bool {
    let Some(ast) = mask else {
        return true;
    };
    match eval(ast, ctx) {
        Ok(q) => q.value.abs() >= 1e-9,
        Err(e) => {
            warnings.push(format!("{}: {e:?} — {outcome}", what()));
            true
        }
    }
}
