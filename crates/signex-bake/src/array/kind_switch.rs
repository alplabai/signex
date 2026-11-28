//! Switching a pattern between Linear, Grid and Polar (#2596, DECISIONS
//! 2026-11-27): the instance count carries over, the spacing is kept where
//! it maps, and suppressed instances and pad overrides keep their place
//! when their index still names an instance of the new kind. See
//! docs/FOOTPRINT_SKETCH.md, "Switching a pattern's kind".

use std::collections::BTreeMap;

use signex_expr::{ExprNode, LiteralContext, UnitFamily};
use signex_sketch::array::{ArrayId, ArrayKind, GridDepopulation};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve_quantities;
use signex_sketch::sketch::SketchData;

use crate::eval_util::{parse_in, strip_eq_prefix};

/// The three kinds of a pattern, without their fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternKind {
    Linear,
    Grid,
    Polar,
}

impl PatternKind {
    pub const ALL: [Self; 3] = [Self::Linear, Self::Grid, Self::Polar];

    pub fn of(kind: &ArrayKind) -> Self {
        match kind {
            ArrayKind::Linear { .. } => Self::Linear,
            ArrayKind::Grid { .. } => Self::Grid,
            ArrayKind::Polar { .. } => Self::Polar,
        }
    }
}

/// The kind a pattern switches to; a polar one turns about `centre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindTarget {
    Linear,
    Grid,
    Polar { centre: SketchEntityId },
}

impl KindTarget {
    pub fn kind(self) -> PatternKind {
        match self {
            Self::Linear => PatternKind::Linear,
            Self::Grid => PatternKind::Grid,
            Self::Polar { .. } => PatternKind::Polar,
        }
    }
}

/// What a switch left behind: the `(i, j)` of the old pattern whose index
/// names no instance of the new one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KindSwitched {
    pub suppressed_dropped: Vec<(u32, u32)>,
    pub overrides_dropped: Vec<(u32, u32)>,
}

/// Why a switch changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindSwitchRefused {
    NoSuchPattern,
    SameKind,
}

/// The step a pattern switched from Polar gets, which has none: one row at
/// the pitch a new Rectangular Pattern uses.
const DEFAULT_DX: &str = "5mm";
const NO_STEP: &str = "0mm";
const FULL_TURN: &str = "360deg";

/// How the instances of a pattern line up: their row length and row count,
/// each `None` while its expression does not resolve. A linear or a polar
/// pattern is one row of `count`.
#[derive(Debug, Clone, Copy)]
struct Layout {
    cols: Option<u32>,
    rows: Option<u32>,
}

/// Whether a one-dimensional pattern becomes a grid row or a grid column.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Line {
    Row,
    Column,
}

/// Switch pattern `array_id` to `target`, in place. The count, spacing,
/// mask expression and numbering carry over; the suppression list and the
/// pattern's pad overrides are re-indexed in walk order, and the entries
/// with no place in the new kind are dropped and returned. A linked
/// pattern's override keeps its key: its outer slot is already a walk
/// place, and only one past the slot count is dropped.
pub fn switch_pattern_kind(
    sketch: &mut SketchData,
    array_id: ArrayId,
    target: KindTarget,
) -> Result<KindSwitched, KindSwitchRefused> {
    let ctx = eval_context(sketch);
    let old = sketch
        .arrays
        .iter()
        .find(|a| a.id == array_id)
        .map(|a| a.kind.clone())
        .ok_or(KindSwitchRefused::NoSuchPattern)?;
    if PatternKind::of(&old) == target.kind() {
        return Err(KindSwitchRefused::SameKind);
    }
    let layout = layout_of(&old, &ctx);
    let (mut new, line) = reshaped(&old, target, layout, &ctx);
    let remap = |cell: (u32, u32)| {
        let place = walk_place(cell, layout)?;
        Some(match (target.kind(), line) {
            (PatternKind::Grid, Line::Column) => (0, place),
            _ => (place, 0),
        })
    };
    let mut switched = KindSwitched::default();
    if let Some(d) = old.depopulation() {
        let mut kept = Vec::new();
        for &cell in &d.suppressed_instances {
            match remap(cell) {
                Some(to) => kept.push(to),
                None => switched.suppressed_dropped.push(cell),
            }
        }
        // A blank mask over an emptied list leaves no depopulation.
        if !(d.mask_expr.trim().is_empty() && kept.is_empty()) {
            *depopulation_of(&mut new) = Some(GridDepopulation {
                mask_expr: d.mask_expr.clone(),
                suppressed_instances: kept,
            });
        }
    }
    // A linked pattern keys an override by (outer slot, inner instance),
    // the outer slot numbered in walk order, which the switch keeps.
    let is_linked = sketch
        .pattern_source_of(array_id)
        .is_some_and(|s| sketch.is_pattern(s));
    let slot_count = layout.cols.zip(layout.rows).map(|(c, r)| c.saturating_mul(r));
    let remap_key = |(i, j): (u32, u32)| {
        if is_linked {
            slot_count.is_none_or(|n| i < n).then_some((i, j))
        } else {
            remap((i, j))
        }
    };
    sketch.pad_overrides.retain_mut(|o| {
        if o.array != array_id {
            return true;
        }
        match remap_key((o.i, o.j)) {
            Some((i, j)) => {
                (o.i, o.j) = (i, j);
                true
            }
            None => {
                switched.overrides_dropped.push((o.i, o.j));
                false
            }
        }
    });
    if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == array_id) {
        array.kind = new;
    }
    Ok(switched)
}

/// Remove `point` when nothing uses it: no entity, constraint, pattern or
/// mirror names it and it carries no role of its own. A centre the switch to
/// Polar minted goes this way when the pattern leaves Polar. Returns whether
/// it went.
pub fn remove_unused_point(sketch: &mut SketchData, point: SketchEntityId) -> bool {
    let Some(entity) = sketch.entities.iter().find(|e| e.id == point) else {
        return false;
    };
    if !matches!(entity.kind, EntityKind::Point { .. }) {
        return false;
    }
    let mut bare = Entity::new(entity.id, entity.plane, entity.kind.clone());
    bare.construction = entity.construction;
    bare.centerline = entity.centerline;
    let is_named = *entity != bare
        || sketch
            .entities
            .iter()
            .any(|e| e.id != point && e.point_refs().contains(&point))
        || sketch
            .constraints
            .iter()
            .any(|c| c.kind.entity_refs().contains(&point))
        || sketch
            .arrays
            .iter()
            .any(|a| a.kind.entity_refs().contains(&point))
        || sketch.pattern_mirrors.iter().any(|m| m.line == point);
    if is_named {
        return false;
    }
    sketch.entities.retain(|e| e.id != point);
    true
}

fn depopulation_of(kind: &mut ArrayKind) -> &mut Option<GridDepopulation> {
    match kind {
        ArrayKind::Linear { depopulation, .. }
        | ArrayKind::Grid { depopulation, .. }
        | ArrayKind::Polar { depopulation, .. } => depopulation,
    }
}

/// The new kind with the old one's count and spacing, and how a one-row
/// source lies in a grid. The new kind starts with no depopulation.
fn reshaped(
    old: &ArrayKind,
    target: KindTarget,
    layout: Layout,
    ctx: &EvalContext,
) -> (ArrayKind, Line) {
    let source = old.source();
    // (count, dx, dy) of the old pattern read as one row.
    let (count_expr, dx_expr, dy_expr, line) = match old {
        ArrayKind::Linear {
            count_expr,
            dx_expr,
            dy_expr,
            ..
        } => {
            let is_zero = |expr: &str| length_mm(expr, ctx).is_some_and(|mm| mm.abs() < 1e-9);
            let vertical = is_zero(dx_expr) && !is_zero(dy_expr);
            let line = if vertical { Line::Column } else { Line::Row };
            (count_expr.clone(), dx_expr.clone(), dy_expr.clone(), line)
        }
        ArrayKind::Grid {
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            ..
        } => {
            let count = grid_count(nx_expr, ny_expr, layout);
            // A single column walks down its rows; anything else along its row.
            if layout.cols == Some(1) && layout.rows != Some(1) {
                (count, NO_STEP.into(), dy_expr.clone(), Line::Column)
            } else {
                (count, dx_expr.clone(), NO_STEP.into(), Line::Row)
            }
        }
        ArrayKind::Polar { count_expr, .. } => (
            count_expr.clone(),
            DEFAULT_DX.into(),
            NO_STEP.into(),
            Line::Row,
        ),
    };
    let kind = match target {
        KindTarget::Linear => ArrayKind::Linear {
            source,
            count_expr,
            dx_expr,
            dy_expr,
            depopulation: None,
        },
        KindTarget::Grid => {
            let (nx_expr, ny_expr) = match line {
                Line::Row => (count_expr, "1".to_string()),
                Line::Column => ("1".to_string(), count_expr),
            };
            ArrayKind::Grid {
                source,
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation: None,
            }
        }
        KindTarget::Polar { centre } => ArrayKind::Polar {
            source,
            center: centre,
            count_expr,
            sweep_angle_expr: FULL_TURN.into(),
            depopulation: None,
        },
    };
    (kind, line)
}

/// A grid's instance count as one count expression: the side that is not 1
/// as written, else the product (a number when both resolve).
fn grid_count(nx_expr: &str, ny_expr: &str, layout: Layout) -> String {
    match (layout.cols, layout.rows) {
        (_, Some(1)) => nx_expr.to_string(),
        (Some(1), _) => ny_expr.to_string(),
        (Some(nx), Some(ny)) => (u64::from(nx) * u64::from(ny)).to_string(),
        _ => format!(
            "({}) * ({})",
            strip_eq_prefix(nx_expr),
            strip_eq_prefix(ny_expr)
        ),
    }
}

fn layout_of(kind: &ArrayKind, ctx: &EvalContext) -> Layout {
    match kind {
        ArrayKind::Linear { count_expr, .. } | ArrayKind::Polar { count_expr, .. } => Layout {
            cols: count(count_expr, ctx),
            rows: Some(1),
        },
        ArrayKind::Grid {
            nx_expr, ny_expr, ..
        } => Layout {
            cols: count(nx_expr, ctx),
            rows: count(ny_expr, ctx),
        },
    }
}

/// The place of cell `(i, j)` in the row-by-row walk of `layout`, or `None`
/// when the cell lies outside it (a stale entry, or a mirrored instance past
/// the last row) or its row needs a row length that does not resolve.
fn walk_place((i, j): (u32, u32), layout: Layout) -> Option<u32> {
    if layout.rows.is_some_and(|rows| j >= rows) || layout.cols.is_some_and(|cols| i >= cols) {
        return None;
    }
    if j == 0 {
        return Some(i);
    }
    j.checked_mul(layout.cols?)?.checked_add(i)
}

fn eval_context(sketch: &SketchData) -> EvalContext {
    let params: BTreeMap<String, ExprNode> = resolve_quantities(&sketch.parameters)
        .map(|resolved| {
            resolved
                .into_iter()
                .map(|(name, q)| (name, ExprNode::Literal(q)))
                .collect()
        })
        .unwrap_or_default();
    EvalContext {
        params,
        array_index: Some((0, 0)),
    }
}

/// A count expression's value, when it is a plain number of zero or more.
fn count(expr: &str, ctx: &EvalContext) -> Option<u32> {
    let ast = parse_in(expr, LiteralContext::Count).ok()?;
    let q = eval(&ast, ctx).ok()?;
    if q.unit.family() != UnitFamily::Count || !q.value.is_finite() || q.value < 0.0 {
        return None;
    }
    u32::try_from(q.value.round() as u64).ok()
}

fn length_mm(expr: &str, ctx: &EvalContext) -> Option<f64> {
    let ast = parse_in(expr, LiteralContext::Length).ok()?;
    crate::params::eval_length_mm(&ast, ctx).ok()
}
