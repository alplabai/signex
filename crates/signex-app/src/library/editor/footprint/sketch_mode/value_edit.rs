//! Values on the sketch drawing: the messages of the canvas values, the
//! constraint menu and the pattern glyphs, and the pure rules they share.
//! What a value reads as on the drawing (`name · value`), what an edit
//! starts from, and what typed text becomes. Evaluation goes through the
//! sketch's own expression engine; nothing is re-derived here.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

use signex_expr::{ExprNode, Quantity, UnitFamily};
use signex_sketch::SketchData;
use signex_sketch::array::{ArrayId, ArrayKind};
use signex_sketch::constraint::{ConstraintKind, DimTarget, parse_dimension_input};
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::id::ConstraintId;

use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::canvas_text::SketchCanvasText;
use crate::library::editor::footprint::state::sketch_overlay::{PatternField, ValueTarget};

/// Longest text an in-place edit takes.
const MAX_BUFFER: usize = signex_sketch::expr::MAX_EXPR_LEN;

/// Most instance slots the canvas lays out for one pattern.
pub const MAX_DRAWN_SLOTS: u64 = 2_000;

/// What happened on the sketch canvas's values, menus and patterns.
#[derive(Debug, Clone, PartialEq)]
pub enum SketchCanvasMsg {
    /// A value on the drawing was clicked (or Edit value picked): edit it.
    ValueEditOpened(ValueTarget),
    /// A character was typed into the open edit.
    ValueTyped(char),
    /// Backspace in the open edit.
    ValueErased,
    /// Enter: write the edit, one undo step, then solve.
    ValueCommitted,
    /// Esc (or a click elsewhere): drop the edit.
    ValueEditCancelled,
    /// Right-click on a constraint's glyph or value, at canvas pixels.
    ConstraintMenuOpened {
        constraint: ConstraintId,
        x: f32,
        y: f32,
    },
    /// Right-click on a pattern glyph, at canvas pixels.
    PatternMenuOpened { array: ArrayId, x: f32, y: f32 },
    /// The menu was dismissed.
    MenuClosed,
    /// Select constrained entities.
    ConstrainedEntitiesSelected(ConstraintId),
    /// Show conflicts: select every entity of the conflict.
    ConflictsShown(ConstraintId),
    /// Delete, from the menu or a Properties conflict row.
    ConstraintDeleted(ConstraintId),
    /// Make parameter, handed to the sketch parameters table.
    ParameterRequested(ConstraintId),
    /// A pattern glyph was clicked: select it, or drop the selection.
    PatternSelected(Option<ArrayId>),
    /// Delete pattern.
    PatternDeleted(ArrayId),
    /// An instance of the selected grid or polar pattern was clicked.
    InstanceSuppressionToggled { array: ArrayId, i: u32, j: u32 },
}

/// Every constraint in conflict: the last solve's over-constrained set and
/// the constraints the last rejected edit clashed with. The canvas badge,
/// the menu's Show conflicts and its selection all read this one set.
pub fn conflicting_constraints(state: &FootprintEditorState) -> HashSet<ConstraintId> {
    state
        .last_solve
        .as_ref()
        .map(|s| s.over_constraints.as_slice())
        .unwrap_or_default()
        .iter()
        .chain(state.conflict.constraints())
        .copied()
        .collect()
}

/// The sketch's parameters as the last solve resolved them, or resolved
/// now when there is no solve yet.
pub fn resolved_params<'a>(
    sketch: &SketchData,
    state: &'a FootprintEditorState,
) -> Cow<'a, HashMap<String, f64>> {
    match state.last_solve.as_ref() {
        Some(solve) => Cow::Borrowed(&solve.params),
        None => {
            Cow::Owned(signex_sketch::parameter::resolve(&sketch.parameters).unwrap_or_default())
        }
    }
}

/// The body of an expression source: no surrounding space, no `=`.
fn expr_body(source: &str) -> &str {
    source.trim().trim_start_matches('=').trim()
}

/// The parameter `source` names when it is exactly one parameter of
/// `sketch` (`pitch`, `= pitch`), so the drawing can show `pitch · 0.65`.
pub fn bound_parameter<'s>(source: &'s str, sketch: &SketchData) -> Option<&'s str> {
    let body = expr_body(source);
    let is_name = body
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && body.chars().all(|c| c.is_alphanumeric() || c == '_');
    (is_name && sketch.parameters.get_raw(body).is_some()).then_some(body)
}

/// A length on the drawing: two decimals and the unit ("0.70 mm").
pub fn format_length(mm: f64, unit: &str) -> String {
    format!("{mm:.2} {unit}")
}

/// An angle on the drawing, in degrees: whole when it is ("90°"), else to
/// a tenth ("22.5°").
pub fn format_angle(degrees: f64, sign: &str) -> String {
    if (degrees - degrees.round()).abs() < 0.05 {
        format!("{:.0}{sign}", degrees.round())
    } else {
        format!("{degrees:.1}{sign}")
    }
}

/// What a dimension constraint shows on the drawing: the parameter its
/// target names, if any, and its value with unit. `None` for a constraint
/// that is not a dimension.
pub fn dimension_label(
    kind: &ConstraintKind,
    sketch: &SketchData,
    params: &HashMap<String, f64>,
    text: &SketchCanvasText,
) -> Option<(Option<String>, String)> {
    let target = kind.dim_target()?;
    let name = match target {
        DimTarget::Expr(source) => bound_parameter(source, sketch).map(str::to_owned),
        DimTarget::Literal(_) => None,
    };
    let value = match signex_sketch::solver::residual::resolve_dim(target, params) {
        Ok(v) if matches!(kind, ConstraintKind::Angle { .. }) => {
            format_angle(v.to_degrees(), &text.unit_deg)
        }
        Ok(v) => format_length(v, &text.unit_mm),
        Err(_) => text.value_unresolved.clone(),
    };
    Some((name, value))
}

/// A number as an edit starts from it: up to four decimals, trailing
/// zeros dropped ("0.7", "90").
fn plain_number(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The text an edit of a constraint's target starts with: its expression,
/// or its number (degrees for an angle).
pub fn constraint_buffer(kind: &ConstraintKind) -> Option<String> {
    Some(match kind.dim_target()? {
        DimTarget::Expr(source) => source.clone(),
        DimTarget::Literal(v) if matches!(kind, ConstraintKind::Angle { .. }) => {
            plain_number(v.to_degrees())
        }
        DimTarget::Literal(v) => plain_number(*v),
    })
}

/// What typed text becomes as the target of `kind`: a plain number is a
/// literal (degrees for an angle, stored in radians), anything else an
/// expression, kept only when it evaluates. `None` when the text is empty
/// or does not evaluate.
pub fn parse_constraint_buffer(
    buffer: &str,
    kind: &ConstraintKind,
    params: &HashMap<String, f64>,
) -> Option<DimTarget> {
    let typed = buffer.trim();
    if typed.is_empty() {
        return None;
    }
    if let Ok(v) = parse_dimension_input(typed) {
        let canonical = if matches!(kind, ConstraintKind::Angle { .. }) {
            v.to_radians()
        } else {
            v
        };
        return Some(DimTarget::Literal(canonical));
    }
    let target = DimTarget::Expr(typed.to_string());
    signex_sketch::solver::residual::resolve_dim(&target, params)
        .is_ok()
        .then_some(target)
}

/// Evaluate `source` against the parameter table of `sketch`, each
/// parameter in the unit family it declares.
pub fn eval_source(sketch: &SketchData, source: &str) -> Option<Quantity> {
    let ast = signex_sketch::expr::parse::parse(expr_body(source)).ok()?;
    let params: BTreeMap<String, ExprNode> =
        signex_sketch::parameter::resolve_quantities(&sketch.parameters)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, q)| (name, ExprNode::Literal(q)))
            .collect();
    let ctx = EvalContext {
        params,
        array_index: None,
    };
    eval(&ast, &ctx).ok()
}

/// A pattern count as a whole number of at least one.
fn as_count(q: Quantity) -> Option<u32> {
    (q.unit.family() == UnitFamily::Count && q.value.is_finite() && q.value >= 0.5)
        .then(|| q.value.round().min(f64::from(u32::MAX)) as u32)
}

/// A pattern step in mm: a length, or a bare number read as mm.
fn as_length(q: Quantity) -> Option<f64> {
    matches!(q.unit.family(), UnitFamily::Length | UnitFamily::Count)
        .then_some(q.value)
        .filter(|v| v.is_finite())
}

/// A sweep in radians.
fn as_angle(q: Quantity) -> Option<f64> {
    (q.unit.family() == UnitFamily::Angle && q.value.is_finite()).then_some(q.value)
}

/// Whether `source` is a valid value for `field`.
pub fn pattern_value_ok(sketch: &SketchData, field: PatternField, source: &str) -> bool {
    let Some(q) = eval_source(sketch, source) else {
        return false;
    };
    match field {
        PatternField::Count | PatternField::Columns | PatternField::Rows => as_count(q).is_some(),
        PatternField::SpacingX | PatternField::SpacingY => as_length(q).is_some(),
        PatternField::Sweep => as_angle(q).is_some(),
    }
}

/// A pattern value as the drawing shows it ("4", "0.65 mm", "360°").
pub fn pattern_value_text(
    sketch: &SketchData,
    field: PatternField,
    source: &str,
    text: &SketchCanvasText,
) -> String {
    let q = eval_source(sketch, source);
    let shown = match field {
        PatternField::Count | PatternField::Columns | PatternField::Rows => {
            q.and_then(as_count).map(|n| n.to_string())
        }
        PatternField::SpacingX | PatternField::SpacingY => q
            .and_then(as_length)
            .map(|mm| format_length(mm, &text.unit_mm)),
        PatternField::Sweep => q
            .and_then(as_angle)
            .map(|rad| format_angle(rad.to_degrees(), &text.unit_deg)),
    };
    shown.unwrap_or_else(|| text.value_unresolved.clone())
}

/// The glyph of a pattern: its letter and counts ("L×4", "G×3×4", "P×8").
pub fn pattern_glyph_text(
    sketch: &SketchData,
    kind: &ArrayKind,
    text: &SketchCanvasText,
) -> String {
    let count = |field: PatternField| {
        field
            .expr(kind)
            .and_then(|src| eval_source(sketch, src))
            .and_then(as_count)
            .map_or_else(|| text.value_unresolved.clone(), |n| n.to_string())
    };
    match kind {
        ArrayKind::Linear { .. } => {
            format!("{}×{}", text.pattern_linear, count(PatternField::Count))
        }
        ArrayKind::Grid { .. } => format!(
            "{}×{}×{}",
            text.pattern_grid,
            count(PatternField::Columns),
            count(PatternField::Rows)
        ),
        ArrayKind::Polar { .. } => format!("{}×{}", text.pattern_polar, count(PatternField::Count)),
    }
}

/// The text an edit of a value starts with.
pub fn initial_buffer(target: ValueTarget, sketch: &SketchData) -> Option<String> {
    match target {
        ValueTarget::Constraint(id) => sketch
            .constraints
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| constraint_buffer(&c.kind)),
        ValueTarget::Pattern(array, field) => sketch
            .arrays
            .iter()
            .find(|a| a.id == array)
            .and_then(|a| field.expr(&a.kind))
            .map(str::to_owned),
    }
}

/// `buffer` with `ch` typed at its end; control characters and text past
/// the length cap are ignored.
pub fn typed(buffer: &str, ch: char) -> Option<String> {
    (!ch.is_control() && buffer.len() + ch.len_utf8() <= MAX_BUFFER).then(|| {
        let mut next = buffer.to_string();
        next.push(ch);
        next
    })
}
