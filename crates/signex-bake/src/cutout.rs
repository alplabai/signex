//! Board cutout bake — turns BoardCutoutAttr-tagged closed profiles
//! into `Footprint::cutouts: Vec<FpCutout>` records.
//!
//! Phase B / Stage 4 of the v0.14.1 sketch-mode plan. v0.14.1 records
//! the polygon boundary; PCB outline subtraction (the geometric
//! op that combines cutouts with the board outline) runs at PCB
//! gerber-export time and is out of scope here.
//!
//! `BoardCutoutAttr.edge_radius_expr` (corner radius) and
//! `through` (through-PCB vs partial-depth) are intentionally NOT
//! propagated into `FpCutout` in v0.14.1 — those need new lib fields
//! that haven't been schema-bumped yet. They're reported in a warning
//! so users know the data isn't lost, just deferred.

use signex_library::primitive::footprint::{FpCutout, Polygon};
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::SketchError;

use crate::profile::{trace_closed_profile, TraceError};

pub fn bake_cutouts(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<FpCutout>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        let attr = match entity.board_cutout.as_ref() {
            Some(a) => a,
            None => continue,
        };
        if !matches!(entity.kind, EntityKind::Line { .. } | EntityKind::Arc { .. }) {
            warnings.push(format!(
                "entity {}: BoardCutoutAttr requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }

        match trace_closed_profile(sketch, solve, entity.id) {
            Ok(vertices) => {
                if attr.edge_radius_expr.is_some() {
                    warnings.push(format!(
                        "entity {}: BoardCutoutAttr.edge_radius_expr ignored — corner-radius lib field lands in v0.15",
                        entity.id
                    ));
                }
                if !attr.through {
                    warnings.push(format!(
                        "entity {}: BoardCutoutAttr.through=false ignored — partial-depth cutouts lib field lands in v0.15",
                        entity.id
                    ));
                }
                out.push(FpCutout {
                    boundary: Polygon::new(vertices),
                });
            }
            Err(TraceError::OpenChain) => warnings.push(format!(
                "entity {}: BoardCutoutAttr profile is not closed (open chain); skipping",
                entity.id
            )),
            Err(TraceError::Branching) => warnings.push(format!(
                "entity {}: BoardCutoutAttr profile branches at a vertex; skipping",
                entity.id
            )),
            Err(other) => warnings.push(format!(
                "entity {}: BoardCutoutAttr trace failed ({other:?}); skipping",
                entity.id
            )),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::BoardCutoutAttr;
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_sketch::solver::Solver;

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        Solver::default()
            .solve(sketch, &ResolvedParams::new())
            .unwrap()
    }

    fn rectangle_with_cutout(attr: BoardCutoutAttr) -> SketchData {
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
            .push(Entity::new(p2, plane, EntityKind::Point { x: 3.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p3, plane, EntityKind::Point { x: 3.0, y: 3.0 }));
        data.entities
            .push(Entity::new(p4, plane, EntityKind::Point { x: 0.0, y: 3.0 }));
        let mut l1 = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        l1.board_cutout = Some(attr);
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
    fn bake_cutout_simple() {
        let data = rectangle_with_cutout(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_cutouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].boundary.points.len(), 4);
        assert!(warnings.is_empty(), "default cutout should bake without warnings, got {warnings:?}");
    }

    #[test]
    fn bake_cutout_edge_radius_warns() {
        let data = rectangle_with_cutout(BoardCutoutAttr {
            edge_radius_expr: Some("1mm".into()),
            through: true,
        });
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_cutouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("edge_radius_expr")));
    }

    #[test]
    fn bake_cutout_partial_depth_warns() {
        let data = rectangle_with_cutout(BoardCutoutAttr {
            edge_radius_expr: None,
            through: false,
        });
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_cutouts(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("partial-depth")));
    }
}
