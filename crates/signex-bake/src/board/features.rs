//! Keepouts, cutouts, mounting holes and V-scores of a board sketch.
//!
//! Each baked item takes the uuid of the entity it came from: the seed edge
//! of a traced profile (the first tagged edge in sketch order) or the
//! circle. A keepout or cutout may be a circle or a closed loop of lines and
//! arcs, like the outline.

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb::{NetDef, Point};
use signex_types::pcb_sketch::{PcbCutout, PcbHole, PcbKeepout, PcbVScore};
use uuid::Uuid;

use super::geometry::{boundary, circle, point};
use super::patterned::{boundary_places, own_sources, places};
use super::{BoardBakeError, Context, slot};
use crate::eval_util::{build_ctx, eval_dimensionless, eval_mm, opt_eval_mm};
use crate::profile::trace_closed_profile_entities;
use crate::traced_loops::TracedLoops;

/// What a refused hole pattern is called in `BoardBakeError::Expression`.
const HOLE_PATTERN: &str = "mounting hole pattern";

pub(super) fn keepouts(ctx: &Context<'_>) -> Result<Vec<PcbKeepout>, BoardBakeError> {
    let mut loops = TracedLoops::default();
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.keepout.is_some()) {
        let Some(attr) = entity.keepout.as_ref() else {
            continue;
        };
        let attribute = format!("{attr:?}");
        let Some(polygon) = boundary(ctx, &mut loops, &attribute, entity, BakedKind::Keepouts)?
        else {
            continue;
        };
        let kind = BakedKind::Keepouts;
        for place in boundary_places(ctx, entity, &polygon, kind, "keepout pattern")? {
            out.push(PcbKeepout {
                uuid: place.uuid,
                boundary: place.ring(&polygon),
                layers: vec![attr.layer],
                forbids: attr.kinds,
            });
        }
    }
    Ok(out)
}

/// The cutouts, and each one that is a slot with its uuid and size, so the
/// bake preview lists a slot once, as a slot.
pub(super) fn cutouts_with_slots(
    ctx: &Context<'_>,
) -> Result<(Vec<PcbCutout>, Vec<(Uuid, slot::Slot)>), BoardBakeError> {
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut loops = TracedLoops::default();
    let mut out = Vec::new();
    let mut slots = Vec::new();
    for entity in ctx.tagged(|e| e.board_cutout.is_some()) {
        let Some(attr) = entity.board_cutout.as_ref() else {
            continue;
        };
        let attribute = format!("{attr:?}");
        let Some(polygon) = boundary(ctx, &mut loops, &attribute, entity, BakedKind::Cutouts)?
        else {
            continue;
        };
        // A slot's routed edge is its own radius (decision 2, 2026-11-20).
        let slot = if is_slot(ctx, entity) {
            Some(slot::check(ctx, entity.id)?)
        } else {
            None
        };
        let edge_radius = match slot {
            Some(slot) => slot.width / 2.0,
            None => opt_eval_mm(&attr.edge_radius_expr, &eval)
                .and_then(|r| finite(r.unwrap_or(0.0)))
                .map_err(|message| expression(entity.id, "cutout edge radius", message))?,
        };
        let kind = BakedKind::Cutouts;
        for place in boundary_places(ctx, entity, &polygon, kind, "cutout pattern")? {
            if let Some(slot) = slot {
                slots.push((place.uuid, slot));
            }
            out.push(PcbCutout {
                uuid: place.uuid,
                boundary: place.ring(&polygon),
                edge_radius,
                through: attr.through,
            });
        }
    }
    Ok((out, slots))
}

/// Whether the cutout seeded by `seed` is a Slot: the seed, or any edge of
/// its loop, carries `board_slot`. A circle tagged Slot is checked, and
/// refused, as one.
fn is_slot(ctx: &Context<'_>, seed: &Entity) -> bool {
    if seed.board_slot.is_some() {
        return true;
    }
    if !matches!(seed.kind, EntityKind::Line { .. } | EntityKind::Arc { .. }) {
        return false;
    }
    let Ok(profile) = trace_closed_profile_entities(ctx.sketch, seed.id) else {
        return false;
    };
    profile.edges.iter().any(|edge| {
        ctx.sketch
            .entities
            .iter()
            .any(|e| e.id == edge.id && e.board_slot.is_some())
    })
}

/// The board net `name` names, for the entity that names it; `None` for no
/// name. A name the table lacks refuses the bake with `unknown`, which
/// names the kind of entity.
pub(super) fn named_net<'n>(
    entity: SketchEntityId,
    name: Option<&str>,
    nets: &'n [NetDef],
    unknown: fn(SketchEntityId, String) -> BoardBakeError,
) -> Result<Option<&'n NetDef>, BoardBakeError> {
    match name.filter(|name| !name.is_empty()) {
        None => Ok(None),
        Some(name) => nets
            .iter()
            .find(|n| n.name == name)
            .map(Some)
            .ok_or_else(|| unknown(entity, name.to_string())),
    }
}

/// Where one mounting hole is written.
pub(super) struct HolePlace {
    pub(super) uuid: Uuid,
    pub(super) position: Point,
}

/// Every place the hole `entity`, centred at `centre`, is drilled: the
/// instances of the patterns of the circle or of its centre point, or the
/// circle alone. A pattern that does not expand refuses the bake, since a
/// dropped instance would pass DRC unseen.
pub(super) fn hole_places(
    ctx: &Context<'_>,
    entity: &Entity,
    centre: Point,
) -> Result<Vec<HolePlace>, BoardBakeError> {
    let EntityKind::Circle { center, .. } = entity.kind else {
        return Err(BoardBakeError::HoleNotACircle { entity: entity.id });
    };
    let sources = [entity.id, center];
    let found = places(ctx, entity, &sources, &[centre], HOLE_PATTERN)?;
    Ok(found
        .iter()
        .map(|place| HolePlace {
            uuid: place.uuid,
            position: place.at(centre),
        })
        .collect())
}

/// A hole at each place of each circle tagged `mounting_hole`
/// ([`hole_places`]): its diameter or the attribute's drill, and the named
/// net's number.
pub(super) fn holes(ctx: &Context<'_>, nets: &[NetDef]) -> Result<Vec<PcbHole>, BoardBakeError> {
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.mounting_hole.is_some()) {
        let Some(attr) = entity.mounting_hole.as_ref() else {
            continue;
        };
        if !matches!(entity.kind, EntityKind::Circle { .. }) {
            return Err(BoardBakeError::HoleNotACircle { entity: entity.id });
        }
        let (position, radius) = circle(ctx, entity, BakedKind::MountingHoles)?;
        let drill = match attr.drill_expr.as_deref() {
            Some(expr) => eval_mm(expr, &eval)
                .map_err(|message| expression(entity.id, "mounting hole drill", message))?,
            None => 2.0 * radius,
        };
        if !(drill.is_finite() && drill > 0.0) {
            return Err(BoardBakeError::HoleDrill {
                entity: entity.id,
                drill,
            });
        }
        let net = named_net(entity.id, attr.net.as_deref(), nets, |entity, net| {
            BoardBakeError::UnknownNet { entity, net }
        })?
        .map(|n| n.number);
        for place in hole_places(ctx, entity, position)? {
            out.push(PcbHole {
                uuid: place.uuid,
                position: place.position,
                drill,
                plated: attr.plated,
                net,
                unknown_columns: Default::default(),
            });
        }
    }
    Ok(out)
}

/// A V-score along each line tagged `v_score`; depth stays a fraction of
/// the board thickness, so a thickness change needs no re-bake.
pub(super) fn v_scores(ctx: &Context<'_>) -> Result<Vec<PcbVScore>, BoardBakeError> {
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.v_score.is_some()) {
        let Some(attr) = entity.v_score.as_ref() else {
            continue;
        };
        let EntityKind::Line { start, end } = entity.kind else {
            return Err(BoardBakeError::VScoreNotALine { entity: entity.id });
        };
        let depth_fraction = eval_dimensionless(&attr.depth_fraction_expr, &eval)
            .and_then(finite)
            .map_err(|message| expression(entity.id, "V-score depth", message))?
            .clamp(0.0, 1.0);
        let min_web = opt_eval_mm(&attr.min_web_expr, &eval)
            .and_then(|web| finite(web.unwrap_or(0.0)))
            .map_err(|message| expression(entity.id, "V-score minimum web", message))?
            .max(0.0);
        let from = point(ctx, start, BakedKind::VScores)?;
        let to = point(ctx, end, BakedKind::VScores)?;
        for place in places(
            ctx,
            entity,
            &own_sources(entity),
            &[from, to],
            "V-score pattern",
        )? {
            out.push(PcbVScore {
                uuid: place.uuid,
                from: place.at(from),
                to: place.at(to),
                depth_fraction,
                min_web,
                side: attr.side,
            });
        }
    }
    Ok(out)
}

/// A value the board can store: a NaN or infinity would fail the save.
pub(super) fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{value} is not a finite number"))
    }
}

pub(super) fn expression(
    entity: SketchEntityId,
    what: &'static str,
    message: String,
) -> BoardBakeError {
    BoardBakeError::Expression {
        entity,
        what,
        message,
    }
}
