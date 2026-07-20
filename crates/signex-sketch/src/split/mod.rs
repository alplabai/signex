//! `split_line` — divide a sketch `Line` at a parameter into two Lines
//! sharing a new mid `Point`.
//!
//! Pure model primitive (issue #360) — no `signex-app` / UI dependency.
//! The footprint editor's Break Track action (issue #372) is the
//! consumer: it hit-tests a click against a Line, projects it to a
//! parameter `t`, and calls [`split_line`] directly.

mod attr_refs;
mod constraints;
#[cfg(test)]
mod tests;

use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::sketch::SketchData;

/// Minimum real-world segment length (mm) [`split_line`] will produce
/// or accept as input. Three orders of magnitude below the ~0.30 mm
/// hit-test tolerance a click-driven caller (issue #372) works with,
/// so it never rejects a split a user could plausibly have aimed for
/// — but it reliably rejects the two cases that used to slip through
/// a purely parametric `t` guard: a line already too short to usefully
/// split, and a `t` that, once multiplied through the line's real
/// coordinates, mints a mid Point within float noise of an endpoint
/// (see [`SplitError::TooCloseToEndpoint`]'s doc for the counter-
/// example that motivated replacing the old parametric-only guard).
pub const MIN_SEGMENT_LEN_MM: f64 = 1e-3;

/// Failure modes for [`split_line`]. On every variant `sketch` is left
/// byte-for-byte unchanged — all validation runs against read-only
/// lookups before any mutation.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum SplitError {
    /// `line` doesn't resolve to an entity, resolves to a non-`Line`
    /// entity, or one of its endpoint Points doesn't resolve. Issue
    /// #372 hit-tests before calling `split_line`, so this branch is
    /// unreachable from that caller and exists for defensive / API
    /// completeness.
    #[error("{0} does not resolve to a splittable Line")]
    NotALine(SketchEntityId),
    /// `t` is NaN, infinite, or outside the open interval `(0.0, 1.0)`.
    #[error("split parameter t={0} is not finite or lies outside (0.0, 1.0)")]
    InvalidParameter(f64),
    /// `t` lies inside `(0.0, 1.0)`, but the resulting mid Point would
    /// land within [`MIN_SEGMENT_LEN_MM`] of `start` or `end` — the
    /// #372 case: the user clicked close enough to an existing
    /// endpoint that splitting there would mint a sliver segment
    /// instead of two useful halves. Checked against the mid Point's
    /// real coordinates, not against `t` alone (a parametric-only
    /// check misses this: on a line from `(500.0, 0.0)` to
    /// `(500.000001, 0.0)`, `t = 2e-9` clears any sane parametric
    /// epsilon by 2x, yet `start.x + t * dx` is absorbed by the ulp of
    /// `500.0` and lands bit-identical to `start`).
    ///
    /// By the time this fires, `validate_split` has already rejected
    /// every line shorter than twice [`MIN_SEGMENT_LEN_MM`] (see
    /// [`DegenerateLine`](Self::DegenerateLine)), so a line long enough
    /// to reach this check always has SOME `t` (e.g. `0.5`) that would
    /// succeed — "click nearer the middle" is always true advice here,
    /// never a dead end.
    #[error("split would land within the minimum segment length of an endpoint")]
    TooCloseToEndpoint,
    /// `line`'s two endpoints resolve to less than twice
    /// [`MIN_SEGMENT_LEN_MM`] apart, or one of them is non-finite
    /// (NaN / infinite) — either way, no `t` in `(0.0, 1.0)` can ever
    /// produce a valid split. The `2x` threshold (not `1x`) matters:
    /// even the best-case split, `t = 0.5`, divides the line exactly in
    /// half, so a line shorter than twice the minimum still leaves at
    /// least one half under it, at ANY `t`. A `1x` threshold would
    /// misclassify that line as [`TooCloseToEndpoint`](Self::TooCloseToEndpoint)
    /// for the caller's specific `t` — a variant whose doc promises a
    /// nearer-middle retry will succeed, which for such a line is
    /// false. `DegenerateLine` is the one variant that means "no retry
    /// will ever help."
    #[error("line is degenerate (shorter than twice the minimum segment length, or non-finite)")]
    DegenerateLine,
}

/// Result of a successful [`split_line`] — the new mid `Point` and the
/// two replacement `Line`s (`start -> mid`, `mid -> end`), so a caller
/// can select or further constrain them without re-querying the
/// sketch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitResult {
    pub mid_point: SketchEntityId,
    /// `start -> mid_point`.
    pub line_a: SketchEntityId,
    /// `mid_point -> end`.
    pub line_b: SketchEntityId,
    /// Constraints that named the retired line and were DROPPED
    /// outright rather than rewritten (currently only `Midpoint` —
    /// see `constraints::is_dropped_midpoint`). Empty on the common
    /// path; a caller that wants to tell the user something was
    /// discarded reads this instead of the primitive formatting its
    /// own user-facing string.
    pub dropped_constraints: Vec<ConstraintId>,
}

/// Split the `Line` entity `line` at parameter `t` (`0.0` = start,
/// `1.0` = end) into two Lines meeting at a new mid `Point`.
///
/// Returns [`SplitError`] — leaving `sketch` byte-for-byte unchanged —
/// per the conditions on each variant.
///
/// On success the original `Line` is removed and replaced by two new
/// `Line`s that inherit its `construction` / `centerline` flags and
/// its PER-SEGMENT bake attributes (`silk`, `v_score`) — so splitting
/// e.g. a silk line yields two silk lines rather than dropping either
/// half out of the bake.
///
/// CLOSED-PROFILE SEED attributes (`courtyard`, `mask_opening`,
/// `mask_exclude`, `paste_aperture`, `pour`, `keepout`,
/// `board_cutout`) stay on `line_a` ONLY. Each of those bakes by
/// tracing the WHOLE closed loop from any entity that carries the
/// attribute (`trace_closed_profile` in `signex-bake`), so a Line that
/// kept the attribute on both halves would make the bake discover —
/// and emit — the same loop twice (two identical pours on the same
/// net, two routed board cutouts on the same slot, a spurious "only
/// one courtyard per footprint" warning, …). `line_a` keeps the
/// original `start`, so it anchors the trace at the exact point the
/// pre-split seed did — the same one-seed-per-loop reasoning
/// `attr_refs::retarget_pad_profiles` already relies on for the pad
/// Custom-shape / Custom-paste-aperture seed lists.
///
/// CALLERS MUST KNOW: the bake is not the only reader of those seven
/// slots. The footprint editor's Properties panel derives its Role and
/// its Pour / Keepout / Cutout sub-forms from the SINGLE selected
/// entity's own `Option`s, so after a split only `line_a` still
/// presents as (say) a pour boundary — selecting `line_b` shows
/// "Unassigned" and edits to it no-op against a `None`. A caller that
/// leaves the user selected on `line_b` invites them to re-tag it,
/// which puts a second seed on the loop and reproduces the double-emit
/// this rule exists to prevent. Re-select onto [`SplitResult::line_a`]
/// after splitting a profile-bearing edge.
///
/// Every reference to the retired line elsewhere in `sketch` is also
/// rewritten or dropped, by kind:
///
/// | Reference | Outcome |
/// |---|---|
/// | `Horizontal` / `Vertical` constraint | duplicated onto BOTH halves — each is a single-line, absolute-frame predicate that still holds independently for the new segment it lands on. |
/// | `PointOnLine` constraint | re-pointed to whichever half the constrained point currently falls nearest, by parametric position. |
/// | `Midpoint` constraint | DROPPED — the retired id is reported via [`SplitResult::dropped_constraints`]. The original line's midpoint is the midpoint of NEITHER half, so re-pointing would relocate the constraint rather than preserve it. |
/// | `Parallel` / `Perpendicular` / `Angle` / `EqualLength` / `TangentLineArc` / `SymmetricAboutLine` / `DistancePtLine` constraint | re-pointed to `line_a` only — each relates the line to a second, independent entity; duplicating would assert the identical relationship for two now-independent segments. |
/// | `Coincident` / `DistancePtPt` / `Fixed` / `PointOnArc` / `DistancePtCircle` / `EqualRadius` / `TangentArcArc` / `SymmetricAboutPoint` constraint | unchanged — none of their fields can name a Line. |
/// | `SketchData::arrays` (`ArrayKind::*.source`, Polar's `center`) | rewritten to `line_a`. An array source must resolve to a Point carrying a `PadAttr` to bake at all, which a Line id never satisfies — this only fires on already-malformed data, but it is rewritten rather than left dangling. |
/// | a pad's Custom-shape / Custom-paste-aperture `source` list | every matching entry rewritten to `line_a` — both are seed lists into the closed-profile walker, which discovers the whole loop from any edge on it, and `line_a` is still wired into the same loop. |
pub fn split_line(
    sketch: &mut SketchData,
    line: SketchEntityId,
    t: f64,
) -> Result<SplitResult, SplitError> {
    let ValidatedSplit {
        line_idx,
        start_id,
        end_id,
        start_xy,
        end_xy,
    } = validate_split(sketch, line, t)?;
    let dx = end_xy.0 - start_xy.0;
    let dy = end_xy.1 - start_xy.1;
    let mid_xy = (start_xy.0 + t * dx, start_xy.1 + t * dy);
    if !mid_xy.0.is_finite() || !mid_xy.1.is_finite() {
        return Err(SplitError::DegenerateLine);
    }
    if mm_distance(mid_xy, start_xy) < MIN_SEGMENT_LEN_MM
        || mm_distance(mid_xy, end_xy) < MIN_SEGMENT_LEN_MM
    {
        return Err(SplitError::TooCloseToEndpoint);
    }

    let mid_id = SketchEntityId::new();
    let line_a_id = SketchEntityId::new();
    let line_b_id = SketchEntityId::new();
    let template = sketch.entities[line_idx].clone();
    let (mid_point, line_a, line_b) = build_split_entities(
        template, start_id, end_id, mid_id, mid_xy, line_a_id, line_b_id,
    );

    let ctx = SplitCtx {
        line,
        line_a: line_a_id,
        line_b: line_b_id,
        start_xy,
        dx,
        dy,
        len_sq: dx * dx + dy * dy,
        t,
    };

    let dropped_constraints = commit_split(sketch, &ctx, line_idx, mid_point, line_a, line_b);

    Ok(SplitResult {
        mid_point: mid_id,
        line_a: line_a_id,
        line_b: line_b_id,
        dropped_constraints,
    })
}

/// Rewrite every other reference to `ctx.line` (constraints, arrays,
/// pad profile seeds — see `split_line`'s doc table), then replace the
/// retired Line entity with the two new halves. Returns the ids of any
/// constraint dropped outright during the rewrite.
fn commit_split(
    sketch: &mut SketchData,
    ctx: &SplitCtx,
    line_idx: usize,
    mid_point: Entity,
    line_a: Entity,
    line_b: Entity,
) -> Vec<ConstraintId> {
    let (new_constraints, dropped_constraints) = constraints::split_constraints(sketch, ctx);
    attr_refs::retarget_arrays(sketch, ctx);
    attr_refs::retarget_pad_profiles(sketch, ctx);

    sketch.entities.remove(line_idx);
    sketch.entities.push(mid_point);
    sketch.entities.push(line_a);
    sketch.entities.push(line_b);
    sketch.constraints = new_constraints;

    dropped_constraints
}

/// Mint the mid `Point` and the two replacement `Line`s, each
/// inheriting `template`'s plane and every bake attribute (`template`
/// is the retired Line entity itself, cloned before removal).
fn build_split_entities(
    template: Entity,
    start_id: SketchEntityId,
    end_id: SketchEntityId,
    mid_id: SketchEntityId,
    mid_xy: (f64, f64),
    line_a_id: SketchEntityId,
    line_b_id: SketchEntityId,
) -> (Entity, Entity, Entity) {
    let mid_point = Entity::new(
        mid_id,
        template.plane,
        EntityKind::Point {
            x: mid_xy.0,
            y: mid_xy.1,
        },
    );
    let mut line_a = template.clone();
    line_a.id = line_a_id;
    line_a.kind = EntityKind::Line {
        start: start_id,
        end: mid_id,
    };
    let mut line_b = template;
    line_b.id = line_b_id;
    line_b.kind = EntityKind::Line {
        start: mid_id,
        end: end_id,
    };
    // Closed-profile SEED attrs trace the WHOLE loop from any carrier
    // (see this module's doc comment) — `line_a` already carries them
    // via `template`; strip them off `line_b` so exactly one entity on
    // the loop remains a seed. Per-segment attrs (silk, v_score) and
    // the construction/centerline flags stay untouched on both.
    line_b.courtyard = None;
    line_b.mask_opening = None;
    line_b.mask_exclude = None;
    line_b.paste_aperture = None;
    line_b.pour = None;
    line_b.keepout = None;
    line_b.board_cutout = None;
    (mid_point, line_a, line_b)
}

/// Read-only results of [`validate_split`]'s checks — the retired
/// line's index plus its resolved endpoint ids/coordinates. Named
/// fields instead of a 5-tuple `Result` payload (clippy's
/// `type_complexity` flags a bare tuple that size).
struct ValidatedSplit {
    line_idx: usize,
    start_id: SketchEntityId,
    end_id: SketchEntityId,
    start_xy: (f64, f64),
    end_xy: (f64, f64),
}

/// Validate `line` / `t` against read-only lookups only — resolves the
/// line's index and endpoints, checks `t`'s range, and rejects a line
/// no `t` could ever split (shorter than `2 * MIN_SEGMENT_LEN_MM` —
/// see [`SplitError::DegenerateLine`]'s doc for why `2x` and not `1x`).
/// The caller still owes the post-mid-point
/// [`SplitError::TooCloseToEndpoint`] / non-finite check, which needs
/// `t` applied to real coordinates rather than a lookup.
fn validate_split(
    sketch: &SketchData,
    line: SketchEntityId,
    t: f64,
) -> Result<ValidatedSplit, SplitError> {
    let line_idx = sketch
        .entities
        .iter()
        .position(|e| e.id == line)
        .ok_or(SplitError::NotALine(line))?;
    let (start_id, end_id) = match sketch.entities[line_idx].kind {
        EntityKind::Line { start, end } => (start, end),
        _ => return Err(SplitError::NotALine(line)),
    };
    if !t.is_finite() || t <= 0.0 || t >= 1.0 {
        return Err(SplitError::InvalidParameter(t));
    }
    let start_xy = entity_point_xy(sketch, start_id).ok_or(SplitError::NotALine(line))?;
    let end_xy = entity_point_xy(sketch, end_id).ok_or(SplitError::NotALine(line))?;
    if mm_distance(start_xy, end_xy) < 2.0 * MIN_SEGMENT_LEN_MM {
        return Err(SplitError::DegenerateLine);
    }
    Ok(ValidatedSplit {
        line_idx,
        start_id,
        end_id,
        start_xy,
        end_xy,
    })
}

/// Euclidean distance between two `(x, y)` pairs in mm.
fn mm_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Coordinates of a `Point` entity, or `None` if `id` doesn't resolve
/// or resolves to a non-`Point` entity.
fn entity_point_xy(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    sketch
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
}

/// Bundled split parameters threaded through constraint / reference
/// carry-over so the per-kind helpers don't each take eight arguments.
struct SplitCtx {
    line: SketchEntityId,
    line_a: SketchEntityId,
    line_b: SketchEntityId,
    start_xy: (f64, f64),
    dx: f64,
    dy: f64,
    len_sq: f64,
    t: f64,
}
