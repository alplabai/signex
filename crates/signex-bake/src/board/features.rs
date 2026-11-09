//! Keepouts, cutouts, mounting holes and V-scores of a board sketch.
//!
//! Each baked item takes the uuid of the entity it came from: the seed edge
//! of a traced profile (the first tagged edge in sketch order) or the
//! circle. A keepout or cutout may be a circle or a closed loop of lines and
//! arcs, like the outline.

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;
use signex_types::pcb::NetDef;
use signex_types::pcb_sketch::{PcbCutout, PcbHole, PcbKeepout, PcbVScore};

use super::geometry::{boundary, circle, point};
use super::{BoardBakeError, Context};
use crate::eval_util::{build_ctx, eval_dimensionless, eval_mm, opt_eval_mm};
use crate::traced_loops::TracedLoops;

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
        out.push(PcbKeepout {
            uuid: entity.id.0,
            boundary: polygon,
            layers: vec![attr.layer],
            forbids: attr.kinds,
        });
    }
    Ok(out)
}

pub(super) fn cutouts(ctx: &Context<'_>) -> Result<Vec<PcbCutout>, BoardBakeError> {
    let eval = build_ctx(ctx.sketch, ctx.params);
    let mut loops = TracedLoops::default();
    let mut out = Vec::new();
    for entity in ctx.tagged(|e| e.board_cutout.is_some()) {
        let Some(attr) = entity.board_cutout.as_ref() else {
            continue;
        };
        let attribute = format!("{attr:?}");
        let Some(polygon) = boundary(ctx, &mut loops, &attribute, entity, BakedKind::Cutouts)?
        else {
            continue;
        };
        let edge_radius = opt_eval_mm(&attr.edge_radius_expr, &eval)
            .and_then(|r| finite(r.unwrap_or(0.0)))
            .map_err(|message| expression(entity.id, "cutout edge radius", message))?;
        out.push(PcbCutout {
            uuid: entity.id.0,
            boundary: polygon,
            edge_radius,
            through: attr.through,
        });
    }
    Ok(out)
}

/// A hole at each circle tagged `mounting_hole`: its solved centre, its
/// diameter or the attribute's drill, and the named net's number.
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
        let net = match attr.net.as_deref().filter(|name| !name.is_empty()) {
            None => None,
            Some(name) => Some(
                nets.iter()
                    .find(|n| n.name == name)
                    .map(|n| n.number)
                    .ok_or_else(|| BoardBakeError::UnknownNet {
                        entity: entity.id,
                        net: name.to_string(),
                    })?,
            ),
        };
        out.push(PcbHole {
            uuid: entity.id.0,
            position,
            drill,
            plated: attr.plated,
            net,
            unknown_columns: Default::default(),
        });
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
        out.push(PcbVScore {
            uuid: entity.id.0,
            from: point(ctx, start, BakedKind::VScores)?,
            to: point(ctx, end, BakedKind::VScores)?,
            depth_fraction,
            min_web,
            side: attr.side,
        });
    }
    Ok(out)
}

/// A value the board can store: a NaN or infinity would fail the save.
fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{value} is not a finite number"))
    }
}

fn expression(entity: SketchEntityId, what: &'static str, message: String) -> BoardBakeError {
    BoardBakeError::Expression {
        entity,
        what,
        message,
    }
}
