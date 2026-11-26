//! Where a board item repeats: the instances of the patterns of its entity
//! or of the points that define it. Every target bakes through [`places`], on
//! the footprint bake's instance walk. Rules: `docs/PCB_SKETCH.md`,
//! "Patterned board items".

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb::Point;
use uuid::Uuid;

use super::features::{expression, finite};
use super::geometry::point;
use super::{BoardBakeError, Context};
use crate::array::{MAX_ARRAY_INSTANCES, PlacesRefused, SourcePlace, is_patterned, source_places};
use crate::profile::trace_closed_profile_entities;

/// One place a board item is written; the source's own when no pattern
/// repeats it.
pub(super) struct ItemPlace {
    pub(super) uuid: Uuid,
    place: Option<SourcePlace>,
}

impl ItemPlace {
    /// Where this place puts the source point `p`.
    pub(super) fn at(&self, p: Point) -> Point {
        match self.place {
            Some(place) => {
                let (x, y) = place.apply((p.x, p.y));
                Point { x, y }
            }
            None => p,
        }
    }

    /// Whether this place reflects the item.
    pub(super) fn mirrors(&self) -> bool {
        self.place.is_some_and(|place| place.mirrors())
    }

    /// A closed boundary at this place, wound as the source is.
    pub(super) fn ring(&self, ring: &[Point]) -> Vec<Point> {
        let mut out: Vec<Point> = ring.iter().map(|&p| self.at(p)).collect();
        if self.mirrors() {
            out.reverse();
        }
        out
    }
}

/// Every place `item` is written: one per distinct image of the points
/// `shape` under the patterns of `sources`, or the item alone. A pattern that
/// does not expand refuses the bake under `what`, since a dropped instance
/// would pass DRC unseen.
pub(super) fn places(
    ctx: &Context<'_>,
    item: &Entity,
    sources: &[SketchEntityId],
    shape: &[Point],
    what: &'static str,
) -> Result<Vec<ItemPlace>, BoardBakeError> {
    if !is_patterned(ctx.sketch, sources) {
        return Ok(vec![ItemPlace {
            uuid: item.id.0,
            place: None,
        }]);
    }
    let params_ast = crate::params::params_ast(ctx.sketch, ctx.params);
    let at: Vec<(f64, f64)> = shape.iter().map(|p| (p.x, p.y)).collect();
    let found = source_places(
        ctx.sketch,
        ctx.solve,
        &params_ast,
        sources,
        &at,
        &ctx.instances,
    )
    .map_err(|refused| match refused {
        PlacesRefused::DoesNotExpand(message) => expression(item.id, what, message),
        PlacesRefused::OverCap => BoardBakeError::TooManyInstances {
            entity: item.id,
            limit: MAX_ARRAY_INSTANCES,
        },
    })?
    .unwrap_or_default();
    found
        .into_iter()
        .map(|place| -> Result<ItemPlace, BoardBakeError> {
            for &(x, y) in &at {
                let (x, y) = place.apply((x, y));
                finite(x)
                    .and_then(|_| finite(y))
                    .map_err(|message| expression(item.id, what, message))?;
            }
            Ok(ItemPlace {
                uuid: place.uuid(item.id.0),
                place: Some(place),
            })
        })
        .collect()
}

/// [`places`] of a closed boundary seeded by `seed`. A circle is placed by
/// its centre, so turning it about that centre is one item.
pub(super) fn boundary_places(
    ctx: &Context<'_>,
    seed: &Entity,
    ring: &[Point],
    kind: BakedKind,
    what: &'static str,
) -> Result<Vec<ItemPlace>, BoardBakeError> {
    let shape = match seed.kind {
        EntityKind::Circle { center, .. } => vec![point(ctx, center, kind)?],
        _ => ring.to_vec(),
    };
    places(ctx, seed, &profile_sources(ctx, seed), &shape, what)
}

/// The entity and the points that define it.
pub(super) fn own_sources(entity: &Entity) -> Vec<SketchEntityId> {
    let mut sources = vec![entity.id];
    match entity.kind {
        EntityKind::Line { start, end } => sources.extend([start, end]),
        EntityKind::Arc {
            center, start, end, ..
        } => sources.extend([center, start, end]),
        EntityKind::Circle { center, .. } => sources.push(center),
        EntityKind::Point { .. } => {}
    }
    sources
}

/// What a pattern of the profile through `seed` may name: any of its edges
/// or points.
fn profile_sources(ctx: &Context<'_>, seed: &Entity) -> Vec<SketchEntityId> {
    if !matches!(seed.kind, EntityKind::Line { .. } | EntityKind::Arc { .. }) {
        return own_sources(seed);
    }
    match trace_closed_profile_entities(ctx.sketch, seed.id) {
        Ok(profile) => profile
            .edges
            .iter()
            .map(|edge| edge.id)
            .chain(profile.points)
            .collect(),
        Err(_) => own_sources(seed),
    }
}

/// Refuse a pattern of a target a board holds one of per entity: the
/// outline, a component anchor.
pub(super) fn refuse_pattern(
    ctx: &Context<'_>,
    entity: &Entity,
    kind: BakedKind,
) -> Result<(), BoardBakeError> {
    if is_patterned(ctx.sketch, &profile_sources(ctx, entity)) {
        return Err(BoardBakeError::NotPatternable {
            kind,
            entity: entity.id,
        });
    }
    Ok(())
}
