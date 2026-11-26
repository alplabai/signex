//! Footprint sketch tools — Mirror and Offset (carved from `sketch_tools::apply`, ADR-0001 D2).
//! The pattern tools live in `pattern`.
//!
//! Mirror runs once, on OK of the Mirror card ([`mirror`], mockup row
//! 191); its clicks are the card's picks and never reach this module.
//! `apply` routes the Offset click; the preamble locals it reads (`flag`,
//! `ctx.plane_id`, raw click `ctx.x_mm`/`ctx.y_mm`) arrive via
//! [`ToolClickCtx`].
//!
//! [`offset`] additionally splits by the source entity's kind
//! (`offset_line` / `offset_circle` / `offset_arc`) — the three cases are
//! independent geometric derivations sharing only `source_id`/`dist`/
//! `ctx`, so splitting them is pure code motion along an existing
//! conceptual boundary, not a fragmentation of one cohesive routine
//! (ADR-0001 D2 per-gesture judgment call, #177).

use std::collections::HashMap;

use super::ToolClickCtx;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::SketchEdit;
use crate::library::editor::footprint::sketch_mode::mirror_card::{MirrorObject, MirrorRequest};
use crate::library::editor::footprint::state::{PlacementInputKind, SketchTool, ToolPending};
use signex_sketch::SketchData;
use signex_sketch::array::ArrayId;
use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::PlaneId;
use signex_sketch::sketch::{PatternCycle, PatternMirror};

pub(super) fn apply(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    tool: SketchTool,
) {
    match tool {
        SketchTool::Offset => offset(editor, ctx),
        _ => unreachable!("non-transform tool routed here"),
    }
}

/// OK on the Mirror card (mockup row 191): reflect every object about the
/// request's Line, in one undo step. Geometry gets fresh entities over
/// mirrored copies of its Points, one copy per source Point even when
/// objects share it; with Keep linked each pair gets a `SymmetricAboutLine`
/// so the copy follows the source, without it the copy is free. A pattern
/// gets a live mirror whatever Keep linked says. Mirrored Arcs flip
/// `sweep_ccw`, as reflection inverts winding. The tool goes back to
/// Select.
pub(super) fn mirror(editor: &mut crate::app::FootprintDocument, request: &MirrorRequest) {
    editor.state.active_tool = SketchTool::Select;
    editor.state.tool_pending = ToolPending::Idle;
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return;
    };
    let Some(axis) = MirrorAxis::of(sketch, request.line) else {
        super::mirror_pattern::refuse(editor, "Mirror: degenerate Line (zero length)".into());
        return;
    };
    let flags = (editor.state.construction_mode, editor.state.centerline_mode);
    let linked = super::mirror_pattern::link_patterns(editor, |sketch| {
        mirror_into(sketch, request, &axis, flags)
    });
    if let Err(cycle) = linked {
        super::mirror_pattern::refuse(editor, format!("Mirror: {cycle} — no mirror was added"));
    }
}

/// The mirror line, read once before the sketch changes.
struct MirrorAxis {
    a: (f64, f64),
    v: (f64, f64),
    vv: f64,
}

impl MirrorAxis {
    fn of(sketch: &SketchData, line: SketchEntityId) -> Option<Self> {
        let EntityKind::Line { start, end } = sketch.entities.iter().find(|e| e.id == line)?.kind
        else {
            return None;
        };
        let (a, b) = (point_at(sketch, start)?, point_at(sketch, end)?);
        let v = (b.0 - a.0, b.1 - a.1);
        let vv = v.0 * v.0 + v.1 * v.1;
        (vv > 1e-12).then_some(Self { a, v, vv })
    }

    fn reflect(&self, p: (f64, f64)) -> (f64, f64) {
        let t = ((p.0 - self.a.0) * self.v.0 + (p.1 - self.a.1) * self.v.1) / self.vv;
        let foot = (self.a.0 + t * self.v.0, self.a.1 + t * self.v.1);
        (2.0 * foot.0 - p.0, 2.0 * foot.1 - p.1)
    }
}

fn point_at(sketch: &SketchData, id: SketchEntityId) -> Option<(f64, f64)> {
    match sketch.entities.iter().find(|e| e.id == id)?.kind {
        EntityKind::Point { x, y } => Some((x, y)),
        _ => None,
    }
}

/// Write the mirror of `request` into `sketch`: pattern mirrors first, so
/// a refused one leaves the copy untouched for the caller to drop.
fn mirror_into(
    sketch: &mut SketchData,
    request: &MirrorRequest,
    axis: &MirrorAxis,
    flags: (bool, bool),
) -> Result<(), PatternCycle> {
    for object in &request.objects {
        if let MirrorObject::Pattern(pattern) = *object {
            sketch.add_pattern_mirror(PatternMirror {
                id: ArrayId::new(),
                pattern,
                line: request.line,
            })?;
        }
    }
    let mut copy = PointCopies {
        images: HashMap::new(),
        request,
        axis,
        flags,
    };
    for object in &request.objects {
        if let MirrorObject::Entity(id) = *object {
            copy.entity(sketch, id);
        }
    }
    Ok(())
}

/// The mirrored Points made so far, by source Point.
struct PointCopies<'a> {
    images: HashMap<SketchEntityId, SketchEntityId>,
    request: &'a MirrorRequest,
    axis: &'a MirrorAxis,
    flags: (bool, bool),
}

impl PointCopies<'_> {
    fn flagged(&self, plane: PlaneId, kind: EntityKind) -> Entity {
        let mut entity = Entity::new(SketchEntityId::new(), plane, kind);
        (entity.construction, entity.centerline) = self.flags;
        entity
    }

    /// The copy of the Point `source`, made on first use.
    fn point(&mut self, sketch: &mut SketchData, source: SketchEntityId) -> Option<SketchEntityId> {
        if let Some(image) = self.images.get(&source) {
            return Some(*image);
        }
        let plane = sketch.entities.iter().find(|e| e.id == source)?.plane;
        let (x, y) = self.axis.reflect(point_at(sketch, source)?);
        let image = self.flagged(plane, EntityKind::Point { x, y });
        let image_id = image.id;
        sketch.entities.push(image);
        if self.request.keep_linked {
            sketch.constraints.push(Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::SymmetricAboutLine {
                    p1: source,
                    p2: image_id,
                    line: self.request.line,
                },
            });
        }
        self.images.insert(source, image_id);
        Some(image_id)
    }

    /// The copy of the entity `id`; a Point already copied is not copied
    /// again.
    fn entity(&mut self, sketch: &mut SketchData, id: SketchEntityId) -> Option<()> {
        let source = sketch.entities.iter().find(|e| e.id == id)?;
        let (plane, kind) = (source.plane, source.kind.clone());
        let kind = match kind {
            EntityKind::Point { .. } => {
                self.point(sketch, id)?;
                return Some(());
            }
            EntityKind::Line { start, end } => EntityKind::Line {
                start: self.point(sketch, start)?,
                end: self.point(sketch, end)?,
            },
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => EntityKind::Arc {
                center: self.point(sketch, center)?,
                start: self.point(sketch, start)?,
                end: self.point(sketch, end)?,
                sweep_ccw: !sweep_ccw,
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: self.point(sketch, center)?,
                radius,
            },
        };
        let copy = self.flagged(plane, kind);
        sketch.entities.push(copy);
        Some(())
    }
}

// v0.22 Phase B2 — Offset tool. Pre-condition: a Line / Arc / Circle is
// in `selected_sketch`. The click position determines which side of the
// source curve the offset lands on. Offset distance comes from
// `state.placement_input` (kind OffsetDistance) when the user typed
// one; else `state.dimension_input`; else DEFAULT_TOOL_DIMENSION_MM —
// and only when both buffers are empty (#599).
fn offset(editor: &mut crate::app::FootprintDocument, ctx: &ToolClickCtx) {
    let source_id = match editor.state.selected_sketch {
        Some(id) => id,
        None => {
            editor.state.solve_warnings.push(
                        "Offset: select a Line / Arc / Circle first (Select tool, click the curve, then click on the side to offset)"
                            .into(),
                    );
            editor.state.tool_pending = ToolPending::Idle;
            editor.canvas_cache.clear();
            return;
        }
    };
    // v0.25 polish — prefer placement_input over the
    // legacy `dimension_input` text field. The
    // keypress-driven cursor overlay is the
    // discoverable path; `dimension_input` stays as
    // the Properties-panel fallback for users who
    // already have a value there.
    //
    // GH #599 — a typed-but-unreadable buffer aborts the
    // gesture with a report instead of quietly offsetting
    // at DEFAULT_TOOL_DIMENSION_MM.
    let dist = match super::resolve_tool_dimension_mm(editor, PlacementInputKind::OffsetDistance) {
        super::ToolDimension::Accepted { mm, from_placement } => {
            // Clear the buffer so the next Offset click
            // doesn't accidentally reuse the old value.
            if from_placement {
                editor.state.placement_input = None;
            }
            mm
        }
        super::ToolDimension::Rejected { buffer } => {
            super::reject_tool_dimension(editor, "Offset", "offset distance", &buffer);
            return;
        }
    };

    let sketch_ref = match editor.primitive().sketch.as_ref() {
        Some(s) => s,
        None => {
            editor.state.tool_pending = ToolPending::Idle;
            return;
        }
    };
    let source_kind = sketch_ref
        .entities
        .iter()
        .find(|e| e.id == source_id)
        .map(|e| e.kind.clone());
    let source_kind = match source_kind {
        Some(k) => k,
        None => {
            editor
                .state
                .solve_warnings
                .push("Offset: selection no longer exists in the sketch".into());
            editor.state.tool_pending = ToolPending::Idle;
            editor.canvas_cache.clear();
            return;
        }
    };

    match source_kind {
        EntityKind::Line { start, end } => offset_line(editor, ctx, source_id, start, end, dist),
        EntityKind::Circle { center, radius } => {
            offset_circle(editor, ctx, source_id, center, radius, dist)
        }
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => offset_arc(editor, ctx, source_id, center, start, end, sweep_ccw, dist),
        EntityKind::Point { .. } => {
            editor
                .state
                .solve_warnings
                .push("Offset: selection is a Point — pick a Line / Arc / Circle".into());
        }
    }
    editor.state.tool_pending = ToolPending::Idle;
}

// Offset a Line: emits a parallel Line at perpendicular distance and
// adds (Parallel + DistancePtLine) constraints so the relationship
// survives source edits.
fn offset_line(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    source_id: SketchEntityId,
    start: SketchEntityId,
    end: SketchEntityId,
    dist: f64,
) {
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::id::ConstraintId;

    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
        editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == id))
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let (Some((ax, ay)), Some((bx, by))) = (pos_of(start), pos_of(end)) else {
        return;
    };
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        editor
            .state
            .solve_warnings
            .push("Offset: degenerate Line (zero length)".into());
        editor.canvas_cache.clear();
        return;
    }
    // Perpendicular unit vector. Sign from the
    // cross of (line direction) × (click −
    // line start): positive = click is on the
    // (-dy, dx) side, negative = (dy, -dx)
    // side.
    let cx = ctx.x_mm - ax;
    let cy = ctx.y_mm - ay;
    let cross = dx * cy - dy * cx;
    let sign = if cross >= 0.0 { 1.0 } else { -1.0 };
    let nx = -dy / len * sign;
    let ny = dx / len * sign;
    let off_x = nx * dist;
    let off_y = ny * dist;

    let new_a = SketchEntityId::new();
    let new_b = SketchEntityId::new();
    let new_line_id = SketchEntityId::new();
    let a_entity = ctx.flag(Entity::new(
        new_a,
        ctx.plane_id,
        EntityKind::Point {
            x: ax + off_x,
            y: ay + off_y,
        },
    ));
    let b_entity = ctx.flag(Entity::new(
        new_b,
        ctx.plane_id,
        EntityKind::Point {
            x: bx + off_x,
            y: by + off_y,
        },
    ));
    let new_line = ctx.flag(Entity::new(
        new_line_id,
        ctx.plane_id,
        EntityKind::Line {
            start: new_a,
            end: new_b,
        },
    ));
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(a_entity));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(b_entity));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(new_line));
    });
    // Parallel + DistancePtLine on the start
    // endpoint pins the offset distance
    // parametrically. The end endpoint is left
    // free along the offset line direction —
    // the user can drag it without breaking
    // the offset relationship.
    let parallel = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel {
            l1: source_id,
            l2: new_line_id,
        },
    };
    let dist_constraint = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtLine {
            point: new_a,
            line: source_id,
            target: DimTarget::Literal(dist),
        },
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(parallel));
        apply_sketch_edit_with_warnings(
            state,
            primitive,
            SketchEdit::AddConstraint(dist_constraint),
        );
    });
}

// Offset a Circle: emits a concentric copy that shares the source's
// centre Point so the centres stay locked. The new radius is a literal
// (source.radius ± dist) — the schema has no radius-dimension
// constraint, so further radius edits don't auto-propagate; the user
// can re-offset or edit the literal directly.
fn offset_circle(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    source_id: SketchEntityId,
    center: SketchEntityId,
    radius: f64,
    dist: f64,
) {
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::id::ConstraintId;

    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
        editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == id))
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let Some((cx, cy)) = pos_of(center) else {
        return;
    };
    // Click distance from centre — inside the
    // circle = shrink (-dist), outside =
    // expand (+dist). Clamp to a positive
    // radius so we don't mint a degenerate
    // shape.
    let click_r = ((ctx.x_mm - cx).powi(2) + (ctx.y_mm - cy).powi(2)).sqrt();
    let signed = if click_r < radius { -dist } else { dist };
    let new_radius = (radius + signed).max(1e-6);
    let new_circle_id = SketchEntityId::new();
    let new_circle = ctx.flag(Entity::new(
        new_circle_id,
        ctx.plane_id,
        EntityKind::Circle {
            center,
            radius: new_radius,
        },
    ));
    // v0.23 — parametric link: mint an anchor
    // Point on the new circle and pin its
    // distance to the source circle to
    // `signed`. Combined with a DistancePtCircle
    // on the new circle (target=0), this
    // forces `new_radius = source_radius +
    // signed` through the solver — so when
    // the user edits the target via the
    // Properties panel later, the new
    // circle's radius follows.
    let scale = if click_r > 1e-9 {
        new_radius / click_r
    } else {
        1.0
    };
    let anchor_id = SketchEntityId::new();
    let anchor = ctx.flag(Entity::new(
        anchor_id,
        ctx.plane_id,
        EntityKind::Point {
            x: cx + (ctx.x_mm - cx) * scale,
            y: cy + (ctx.y_mm - cy) * scale,
        },
    ));
    let on_new_circle = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: anchor_id,
            circle: new_circle_id,
            target: DimTarget::Literal(0.0),
        },
    };
    let parametric_offset = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: anchor_id,
            circle: source_id,
            target: DimTarget::Literal(signed),
        },
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(anchor));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(new_circle));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(on_new_circle));
        apply_sketch_edit_with_warnings(
            state,
            primitive,
            SketchEdit::AddConstraint(parametric_offset),
        );
    });
}

// Offset an Arc: emits a concentric arc (same `center`) at radius
// `source_r ± dist`, with both new endpoints pinned to the source arc's
// underlying circle via `DistancePtCircle` so the parametric offset
// tracks a later radius edit.
#[expect(
    clippy::too_many_arguments,
    reason = "a sketch transform takes the full source geometry plus the transform spec"
)]
fn offset_arc(
    editor: &mut crate::app::FootprintDocument,
    ctx: &ToolClickCtx,
    source_id: SketchEntityId,
    center: SketchEntityId,
    start: SketchEntityId,
    end: SketchEntityId,
    sweep_ccw: bool,
    dist: f64,
) {
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::id::ConstraintId;

    let pos_of = |id: SketchEntityId| -> Option<(f64, f64)> {
        editor
            .primitive()
            .sketch
            .as_ref()
            .and_then(|s| s.entities.iter().find(|e| e.id == id))
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let (Some((cx, cy)), Some((sx, sy)), Some((ex, ey))) =
        (pos_of(center), pos_of(start), pos_of(end))
    else {
        return;
    };
    // Source radius from start position;
    // direction from start angle.
    let source_r = ((sx - cx).powi(2) + (sy - cy).powi(2)).sqrt();
    let click_r = ((ctx.x_mm - cx).powi(2) + (ctx.y_mm - cy).powi(2)).sqrt();
    let signed = if click_r < source_r { -dist } else { dist };
    let new_r = (source_r + signed).max(1e-6);
    let scale = new_r / source_r.max(1e-9);

    let new_start = SketchEntityId::new();
    let new_end = SketchEntityId::new();
    let new_arc_id = SketchEntityId::new();
    let s_entity = ctx.flag(Entity::new(
        new_start,
        ctx.plane_id,
        EntityKind::Point {
            x: cx + (sx - cx) * scale,
            y: cy + (sy - cy) * scale,
        },
    ));
    let e_entity = ctx.flag(Entity::new(
        new_end,
        ctx.plane_id,
        EntityKind::Point {
            x: cx + (ex - cx) * scale,
            y: cy + (ey - cy) * scale,
        },
    ));
    let new_arc = ctx.flag(Entity::new(
        new_arc_id,
        ctx.plane_id,
        EntityKind::Arc {
            center,
            start: new_start,
            end: new_end,
            sweep_ccw,
        },
    ));
    // v0.23 — parametric link: pin both new
    // endpoints to be `signed` away from the
    // source arc's underlying circle. Since
    // both arcs share the same `center`, this
    // forces the new arc's radius to track
    // source_radius + signed through the
    // solver. End Point's angle is left free
    // — the user can drag it without breaking
    // the parametric distance.
    let dist_start = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: new_start,
            circle: source_id,
            target: DimTarget::Literal(signed),
        },
    };
    let dist_end = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: new_end,
            circle: source_id,
            target: DimTarget::Literal(signed),
        },
    };
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(s_entity));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(e_entity));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::add_entity(new_arc));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(dist_start));
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::AddConstraint(dist_end));
    });
}
