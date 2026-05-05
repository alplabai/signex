//! Courtyard bake — turns CourtyardAttr-tagged closed-profile sketches
//! into the footprint's `courtyard: Polygon` field.
//!
//! Phase B / Stage 3 of the v0.14 sketch-mode plan. The courtyard is
//! a single polygon (IPC-7351 calls it the courtyard outline) that
//! tells PCB DRC how much room the part claims. Per the v3 schema,
//! `Footprint::courtyard: Polygon` is a single polygon — only the
//! first closed profile tagged with [`CourtyardAttr`] becomes the
//! courtyard; subsequent tagged profiles emit a warning.
//!
//! v0.14 scope:
//! - One CourtyardAttr per footprint becomes the courtyard polygon.
//! - Additional CourtyardAttr-tagged entities warn + skip.
//! - Open / branching / arc-containing profiles surface a warning
//!   from `signex_bake::trace_closed_profile` and skip.
//! - Construction entities are excluded from the trace by the walker.

use signex_library::primitive::footprint::Polygon;
use signex_sketch::SketchError;
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use crate::profile::{TraceError, trace_closed_profile};

/// Bake the first CourtyardAttr-tagged closed profile into
/// `courtyard_out`. The first non-construction Line entity carrying
/// CourtyardAttr is used as the trace seed.
///
/// Returns Ok even when the trace fails — failures are reported via
/// `warnings` so the bake pipeline can continue.
pub fn bake_courtyard(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    courtyard_out: &mut Polygon,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let mut found_first = false;
    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        if entity.courtyard.is_none() {
            continue;
        }
        // The walker takes Line + Arc seeds (Circles still skipped in
        // v0.14.1 — Circle bake lands in v0.14.2).
        if !matches!(
            entity.kind,
            EntityKind::Line { .. } | EntityKind::Arc { .. }
        ) {
            warnings.push(format!(
                "entity {}: CourtyardAttr requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }
        if found_first {
            warnings.push(format!(
                "entity {}: CourtyardAttr ignored — only one courtyard polygon per footprint (first closed profile wins)",
                entity.id
            ));
            continue;
        }

        match trace_closed_profile(sketch, solve, entity.id) {
            Ok(vertices) => {
                *courtyard_out = Polygon::new(vertices);
                found_first = true;
            }
            Err(TraceError::OpenChain) => warnings.push(format!(
                "entity {}: CourtyardAttr profile is not closed (open chain); skipping",
                entity.id
            )),
            Err(TraceError::Branching) => warnings.push(format!(
                "entity {}: CourtyardAttr profile branches at a vertex; skipping",
                entity.id
            )),
            Err(TraceError::CircleInProfile) => warnings.push(format!(
                "entity {}: CourtyardAttr profile is a Circle — Circle bake lands in v0.14.2",
                entity.id
            )),
            Err(other) => warnings.push(format!(
                "entity {}: CourtyardAttr trace failed ({other:?}); skipping",
                entity.id
            )),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::CourtyardAttr;
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        Solver::default()
            .solve(sketch, &ResolvedParams::new())
            .unwrap()
    }

    /// Build a 1×1 mm rectangle of 4 Lines and tag the first Line with
    /// CourtyardAttr. Returns the sketch + the seed Line ID.
    fn rectangle_with_courtyard() -> (SketchData, SketchEntityId) {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        let p4 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 1.0, y: 1.0 }));
        data.entities
            .push(Entity::new(p4, plane, EntityKind::Point { x: 0.0, y: 1.0 }));
        let l1 = SketchEntityId::new();
        let mut line1 = Entity::new(l1, plane, EntityKind::Line { start: p1, end: p2 });
        line1.courtyard = Some(CourtyardAttr);
        data.entities.push(line1);
        let l2 = SketchEntityId::new();
        let l3 = SketchEntityId::new();
        let l4 = SketchEntityId::new();
        data.entities.push(Entity::new(
            l2,
            plane,
            EntityKind::Line { start: p2, end: p3 },
        ));
        data.entities.push(Entity::new(
            l3,
            plane,
            EntityKind::Line { start: p3, end: p4 },
        ));
        data.entities.push(Entity::new(
            l4,
            plane,
            EntityKind::Line { start: p4, end: p1 },
        ));
        (data, l1)
    }

    #[test]
    fn bake_courtyard_rectangle() {
        let (data, _l1) = rectangle_with_courtyard();
        let solved = solve(&data);
        let mut courtyard = Polygon::default();
        let mut warnings = Vec::new();
        bake_courtyard(&data, &solved, &mut courtyard, &mut warnings).unwrap();
        assert_eq!(courtyard.points.len(), 4);
        assert!(
            warnings.is_empty(),
            "rectangle should bake without warnings, got {warnings:?}"
        );
    }

    #[test]
    fn bake_courtyard_second_attr_warns() {
        // Add a second CourtyardAttr to a different line — only the
        // first should bake.
        let (mut data, _l1) = rectangle_with_courtyard();
        // Find l2 and tag it too.
        let second_line_id = data
            .entities
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }) && e.courtyard.is_none())
            .map(|e| e.id)
            .next()
            .unwrap();
        let second = data
            .entities
            .iter_mut()
            .find(|e| e.id == second_line_id)
            .unwrap();
        second.courtyard = Some(CourtyardAttr);

        let solved = solve(&data);
        let mut courtyard = Polygon::default();
        let mut warnings = Vec::new();
        bake_courtyard(&data, &solved, &mut courtyard, &mut warnings).unwrap();

        assert_eq!(courtyard.points.len(), 4);
        assert!(
            warnings.iter().any(|w| w.contains("only one courtyard")),
            "expected dedup warning, got {warnings:?}"
        );
    }

    #[test]
    fn bake_courtyard_open_chain_warns() {
        // Two-line open path tagged with CourtyardAttr.
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
        let mut l1 = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        l1.courtyard = Some(CourtyardAttr);
        data.entities.push(l1);
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p2, end: p3 },
        ));

        let solved = solve(&data);
        let mut courtyard = Polygon::default();
        let mut warnings = Vec::new();
        bake_courtyard(&data, &solved, &mut courtyard, &mut warnings).unwrap();

        assert!(courtyard.points.is_empty());
        assert!(
            warnings.iter().any(|w| w.contains("not closed")),
            "expected open-chain warning, got {warnings:?}"
        );
    }

    #[test]
    fn bake_courtyard_construction_seed_skipped() {
        // The CourtyardAttr is on a construction line — walker won't
        // reach it (construction entities are excluded), but the
        // outer bake_courtyard's `if entity.construction continue`
        // also short-circuits.
        let (mut data, l1) = rectangle_with_courtyard();
        let l1_entity = data.entities.iter_mut().find(|e| e.id == l1).unwrap();
        l1_entity.construction = true;

        let solved = solve(&data);
        let mut courtyard = Polygon::default();
        let mut warnings = Vec::new();
        bake_courtyard(&data, &solved, &mut courtyard, &mut warnings).unwrap();

        assert!(
            courtyard.points.is_empty(),
            "construction-tagged seed must not bake"
        );
        assert!(warnings.is_empty(), "construction skip is silent");
    }
}
