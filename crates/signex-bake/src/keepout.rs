//! Keepout bake — turns KeepoutAttr-tagged closed profiles into
//! `Footprint::keepouts: Vec<FpKeepout>` records.
//!
//! Phase B / Stage 4 of the v0.14.1 sketch-mode plan. v0.14.1 records
//! the polygon boundary + layer + KeepoutForbid (mapped from the
//! sketch-side KeepoutKinds bitfield). DRC enforcement is a v0.15
//! consumer concern.
//!
//! Mapping `KeepoutKinds` (6 booleans) to `KeepoutForbid` (5
//! variants):
//! - More than one bit set → `KeepoutForbid::All`.
//! - Single bit:
//!   - `no_components` → `Pads`
//!   - `no_routing`    → `Tracks`
//!   - `no_vias`       → `Vias`
//!   - `no_copper`     → `Copper`
//!   - `no_pours`      → `Copper` (closest match in v0.14.1 enum)
//!   - `no_drilling`   → `All` (no dedicated drill-only variant in
//!     v0.14.1 — the lib enum can grow in v0.15+)
//! - No bits set → `KeepoutForbid::All` (defensive default — an
//!   untyped keepout zone forbids everything).

use signex_library::primitive::footprint::{FpKeepout, KeepoutForbid, LayerId, Polygon};
use signex_sketch::SketchError;
use signex_sketch::attr::KeepoutKinds;
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use crate::profile::{TraceError, trace_closed_profile};

pub fn bake_keepouts(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<FpKeepout>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        let attr = match entity.keepout.as_ref() {
            Some(a) => a,
            None => continue,
        };
        if !matches!(
            entity.kind,
            EntityKind::Line { .. } | EntityKind::Arc { .. }
        ) {
            warnings.push(format!(
                "entity {}: KeepoutAttr requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }

        match trace_closed_profile(sketch, solve, entity.id) {
            Ok(vertices) => out.push(FpKeepout {
                boundary: Polygon::new(vertices),
                layer: LayerId::new(attr.layer.altium_label()),
                forbids: map_kinds(attr.kinds),
            }),
            Err(TraceError::OpenChain) => warnings.push(format!(
                "entity {}: KeepoutAttr profile is not closed (open chain); skipping",
                entity.id
            )),
            Err(TraceError::Branching) => warnings.push(format!(
                "entity {}: KeepoutAttr profile branches at a vertex; skipping",
                entity.id
            )),
            Err(other) => warnings.push(format!(
                "entity {}: KeepoutAttr trace failed ({other:?}); skipping",
                entity.id
            )),
        }
    }
    Ok(())
}

fn map_kinds(k: KeepoutKinds) -> KeepoutForbid {
    let active_count = [
        k.no_routing,
        k.no_components,
        k.no_copper,
        k.no_vias,
        k.no_drilling,
        k.no_pours,
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if active_count == 0 || active_count > 1 {
        return KeepoutForbid::All;
    }
    if k.no_routing {
        KeepoutForbid::Tracks
    } else if k.no_components {
        KeepoutForbid::Pads
    } else if k.no_vias {
        KeepoutForbid::Vias
    } else if k.no_copper || k.no_pours {
        KeepoutForbid::Copper
    } else {
        KeepoutForbid::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::KeepoutAttr;
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_types::layer::SignexLayer;

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        Solver::default()
            .solve(sketch, &ResolvedParams::new())
            .unwrap()
    }

    fn rectangle_with_keepout(attr: KeepoutAttr) -> SketchData {
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
            .push(Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 5.0, y: 5.0 }));
        data.entities
            .push(Entity::new(p4, plane, EntityKind::Point { x: 0.0, y: 5.0 }));
        let mut l1 = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        l1.keepout = Some(attr);
        data.entities.push(l1);
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p2, end: p3 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p3, end: p4 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p4, end: p1 },
        ));
        data
    }

    #[test]
    fn bake_keepout_single_kind_routing_maps_to_tracks() {
        let mut kinds = KeepoutKinds::default();
        kinds.no_routing = true;
        let attr = KeepoutAttr {
            layer: SignexLayer::TopCopper,
            kinds,
        };
        let data = rectangle_with_keepout(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_keepouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].forbids, KeepoutForbid::Tracks);
        assert_eq!(out[0].layer.as_str(), "Top Layer");
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_keepout_multiple_kinds_maps_to_all() {
        let mut kinds = KeepoutKinds::default();
        kinds.no_routing = true;
        kinds.no_vias = true;
        let attr = KeepoutAttr {
            layer: SignexLayer::BottomCopper,
            kinds,
        };
        let data = rectangle_with_keepout(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_keepouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].forbids, KeepoutForbid::All);
    }

    #[test]
    fn bake_keepout_no_kinds_set_is_all() {
        let attr = KeepoutAttr {
            layer: SignexLayer::TopCopper,
            kinds: KeepoutKinds::default(),
        };
        let data = rectangle_with_keepout(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_keepouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].forbids, KeepoutForbid::All);
    }

    #[test]
    fn bake_keepout_pours_maps_to_copper() {
        let mut kinds = KeepoutKinds::default();
        kinds.no_pours = true;
        let attr = KeepoutAttr {
            layer: SignexLayer::TopCopper,
            kinds,
        };
        let data = rectangle_with_keepout(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_keepouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].forbids, KeepoutForbid::Copper);
    }
}
