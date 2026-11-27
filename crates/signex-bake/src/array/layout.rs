//! One array's own layout for the pattern walk ([`super::links::Walk`]):
//! the slots its counts, steps, sweep, suppression list and mask give, with
//! the bake's warnings for a count or sweep it cannot use.

use signex_expr::{ExprNode, LiteralContext, parse};
use signex_sketch::SketchError;
use signex_sketch::array::{Array, ArrayKind, GridDepopulation};
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::id::SketchEntityId;

use super::links::Walk;
use super::place::Place;
use crate::eval_util::{parse_in, strip_eq_prefix};

/// One slot of an array's layout.
pub(crate) struct Slot {
    pub i: usize,
    pub j: usize,
    pub(super) flat: u64,
    pub place: Place,
    /// Held back by the suppression list (only kept by a walk
    /// [`Walk::keeping_listed`]).
    pub listed: bool,
}

impl Slot {
    fn new(i: usize, j: usize, flat: u64, place: Place, listed: bool) -> Self {
        Self {
            i,
            j,
            flat,
            place,
            listed,
        }
    }
}

/// The slots one array lays out.
pub(crate) struct Walked {
    pub slots: Vec<Slot>,
    /// Every slot of the layout, suppressed ones included.
    pub count: u64,
    /// The row width of a grid.
    pub(super) nx: Option<usize>,
}

impl Walked {
    fn new(slots: Vec<Slot>, count: u64, nx: Option<usize>) -> Self {
        Self { slots, count, nx }
    }
}

/// How a slot's mask failure is reported: a grid names its cell, a line
/// or a ring its index.
struct MaskReport {
    label: &'static str,
    source: SketchEntityId,
    is_grid: bool,
}

impl MaskReport {
    fn warning(&self, (i, j): (usize, usize), error: &signex_expr::ExprError) -> String {
        let (label, source) = (self.label, self.source);
        let (at, outcome) = if self.is_grid {
            (format!("({i}, {j})"), "keeping cell")
        } else {
            (format!("i={i}"), "keeping instance")
        };
        format!(
            "{label} array source {source}: depopulation eval failed at {at}: {error:?} — {outcome}"
        )
    }
}

impl Walk<'_, '_> {
    fn context(&self) -> EvalContext {
        EvalContext {
            params: self.params_ast.clone(),
            array_index: Some((0, 0)),
        }
    }

    /// The kept slots of one array's own layout (and the listed ones, for
    /// a walk [`Walk::keeping_listed`]). `None`, with a warning, when a
    /// count or sweep is not usable or the array is over the cap.
    pub(crate) fn walk_slots(&mut self, array: &Array) -> Result<Option<Walked>, SketchError> {
        match &array.kind {
            ArrayKind::Linear {
                source,
                count_expr,
                dx_expr,
                dy_expr,
                depopulation,
            } => self.linear_slots(
                *source,
                [count_expr, dx_expr, dy_expr].map(String::as_str),
                depopulation.as_ref(),
            ),
            ArrayKind::Grid {
                source,
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation,
            } => self.grid_slots(
                *source,
                [nx_expr, ny_expr, dx_expr, dy_expr].map(String::as_str),
                depopulation.as_ref(),
            ),
            ArrayKind::Polar {
                source,
                center,
                count_expr,
                sweep_angle_expr,
                depopulation,
            } => self.polar_slots(
                *source,
                *center,
                count_expr,
                sweep_angle_expr,
                depopulation.as_ref(),
            ),
        }
    }

    /// `count` slots stepped by (`dx`, `dy`). `exprs` is
    /// `[count, dx, dy]`; the suppression list holds `(i, 0)` entries.
    fn linear_slots(
        &mut self,
        source: SketchEntityId,
        exprs: [&str; 3],
        depopulation: Option<&GridDepopulation>,
    ) -> Result<Option<Walked>, SketchError> {
        let [count_expr, dx_expr, dy_expr] = exprs;
        let count_ast = parse_in(count_expr, LiteralContext::Count)?;
        let dx_ast = parse_in(dx_expr, LiteralContext::Length)?;
        let dy_ast = parse_in(dy_expr, LiteralContext::Length)?;
        let mask = mask_ast(depopulation)?;
        let mut ctx = self.context();
        let warnings = &mut *self.warnings;
        let Some(count) =
            super::eval_count("linear", "count_expr", source, &count_ast, &ctx, warnings)?
        else {
            return Ok(None);
        };
        if count <= 0 {
            warnings.push(format!(
                "linear array source {source}: count_expr resolved to {count} — array skipped"
            ));
            return Ok(None);
        }
        let count = count as usize;
        if !self.within_cap("linear", source, Some(count)) {
            return Ok(None);
        }
        let report = MaskReport {
            label: "linear",
            source,
            is_grid: false,
        };
        let mut slots = Vec::with_capacity(count);
        for i in 0..count {
            ctx.array_index = Some((i, 0));
            let Some(listed) = self.visits(depopulation, mask.as_ref(), &ctx, &report) else {
                continue;
            };
            let dx = crate::params::eval_length_mm(&dx_ast, &ctx)?;
            let dy = crate::params::eval_length_mm(&dy_ast, &ctx)?;
            let place = Place::translation(i as f64 * dx, i as f64 * dy);
            slots.push(Slot::new(i, 0, i as u64, place, listed));
        }
        Ok(Some(Walked::new(slots, count as u64, None)))
    }

    /// `nx` × `ny` cells stepped by (`dx`, `dy`), row by row. `exprs` is
    /// `[nx, ny, dx, dy]`.
    fn grid_slots(
        &mut self,
        source: SketchEntityId,
        exprs: [&str; 4],
        depopulation: Option<&GridDepopulation>,
    ) -> Result<Option<Walked>, SketchError> {
        let [nx_expr, ny_expr, dx_expr, dy_expr] = exprs;
        let nx_ast = parse_in(nx_expr, LiteralContext::Count)?;
        let ny_ast = parse_in(ny_expr, LiteralContext::Count)?;
        let dx_ast = parse_in(dx_expr, LiteralContext::Length)?;
        let dy_ast = parse_in(dy_expr, LiteralContext::Length)?;
        let mask = mask_ast(depopulation)?;
        let mut ctx = self.context();
        let warnings = &mut *self.warnings;
        let Some(nx) = super::eval_count("grid", "nx_expr", source, &nx_ast, &ctx, warnings)?
        else {
            return Ok(None);
        };
        let Some(ny) = super::eval_count("grid", "ny_expr", source, &ny_ast, &ctx, warnings)?
        else {
            return Ok(None);
        };
        if nx <= 0 || ny <= 0 {
            warnings.push(format!(
                "grid array source {source}: nx={nx}, ny={ny} — array skipped"
            ));
            return Ok(None);
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if !self.within_cap("grid", source, nx.checked_mul(ny)) {
            return Ok(None);
        }
        let report = MaskReport {
            label: "grid",
            source,
            is_grid: true,
        };
        let mut slots = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                ctx.array_index = Some((i, j));
                let Some(listed) = self.visits(depopulation, mask.as_ref(), &ctx, &report) else {
                    continue;
                };
                let dx = crate::params::eval_length_mm(&dx_ast, &ctx)?;
                let dy = crate::params::eval_length_mm(&dy_ast, &ctx)?;
                let place = Place::translation(i as f64 * dx, j as f64 * dy);
                slots.push(Slot::new(i, j, (j * nx + i) as u64, place, listed));
            }
        }
        Ok(Some(Walked::new(slots, (nx * ny) as u64, Some(nx))))
    }

    /// `count` turns of the source about `center` over the sweep; the
    /// source itself is slot 0.
    fn polar_slots(
        &mut self,
        source: SketchEntityId,
        center: SketchEntityId,
        count_expr: &str,
        sweep_angle_expr: &str,
        depopulation: Option<&GridDepopulation>,
    ) -> Result<Option<Walked>, SketchError> {
        let Some(centre) = self.xy(center) else {
            self.warnings.push(format!(
                "polar array center {center}: position unknown — array skipped"
            ));
            return Ok(None);
        };
        let count_ast = parse_in(count_expr, LiteralContext::Count)?;
        let sweep_ast = parse_in(sweep_angle_expr, LiteralContext::Angle)?;
        let mut ctx = self.context();
        let warnings = &mut *self.warnings;
        let Some(count) =
            super::eval_count("polar", "count_expr", source, &count_ast, &ctx, warnings)?
        else {
            return Ok(None);
        };
        if count <= 0 {
            warnings.push(format!(
                "polar array source {source}: count={count} — array skipped"
            ));
            return Ok(None);
        }
        let count = count as usize;
        if !self.within_cap("polar", source, Some(count)) {
            return Ok(None);
        }
        // The sweep keeps its written unit; anything but an angle refuses.
        let sweep = eval(&sweep_ast, &ctx).map_err(SketchError::Expr)?;
        let Ok(sweep_rad) = sweep.as_rad() else {
            self.warnings.push(format!(
                "polar array source {source}: sweep_angle_expr did not resolve to an angle — array skipped"
            ));
            return Ok(None);
        };
        let mask = mask_ast(depopulation)?;
        let report = MaskReport {
            label: "polar",
            source,
            is_grid: false,
        };
        let mut slots = Vec::new();
        for i in 0..count {
            ctx.array_index = Some((i, 0));
            let Some(listed) = self.visits(depopulation, mask.as_ref(), &ctx, &report) else {
                continue;
            };
            let theta = i as f64 * sweep_rad / count as f64;
            let place = Place::rotation_about(centre, theta);
            slots.push(Slot::new(i, 0, i as u64, place, listed));
        }
        Ok(Some(Walked::new(slots, count as u64, None)))
    }

    /// Whether the walk visits the slot at `ctx`'s index: `Some(false)`
    /// when it survives the suppression list and the mask (a mask that
    /// fails to evaluate keeps it, warned), `Some(true)` when the list
    /// holds it back and the walk keeps listed slots, else `None`.
    fn visits(
        &mut self,
        depopulation: Option<&GridDepopulation>,
        mask: Option<&ExprNode>,
        ctx: &EvalContext,
        report: &MaskReport,
    ) -> Option<bool> {
        let Some((i, j)) = ctx.array_index else {
            return Some(false);
        };
        let listed = depopulation.is_some_and(|d| {
            d.suppressed_instances
                .iter()
                .any(|&(si, sj)| (si as usize, sj as usize) == (i, j))
        });
        if listed {
            return self.keep_listed.then_some(true);
        }
        match mask.map(|ast| eval(ast, ctx)) {
            Some(Ok(q)) => (q.value.abs() >= 1e-9).then_some(false),
            Some(Err(e)) => {
                self.warnings.push(report.warning((i, j), &e));
                Some(false)
            }
            None => Some(false),
        }
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
