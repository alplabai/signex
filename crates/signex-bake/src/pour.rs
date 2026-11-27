//! Pour bake — turns PourAttr-tagged closed sketch profiles into
//! `Footprint::pours: Vec<FpPour>` records.
//!
//! Phase B / Stage 3 of the v0.14 sketch-mode plan. v0.14 records the
//! polygon boundary + all PourAttr metadata (layer, net, fill_type,
//! thermal_relief, clearance, min_thickness, priority). Actual fill
//! generation (polygon offset + raster fill + thermal-relief geometry)
//! lands with the PCB Routing plan (Copper Pour). The pour's Corners shape
//! the baked outline as a board pour's fill shapes its own (#2598).

use signex_library::primitive::footprint::{
    FpPour, NetRef, Polygon, PourFillType as LibPourFill, ThermalReliefStyle as LibThermal,
};
use signex_sketch::SketchError;
use signex_sketch::attr::PourFillType as SkPourFill;
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::Point;
use std::collections::HashMap;

use crate::eval_util::{build_ctx, opt_eval_mm};
use crate::layer_label::layer_id;
use crate::open_ends::warn_open_profiles;
use crate::profile::TraceError;
use crate::traced_loops::TracedLoops;

/// Bake every PourAttr-tagged closed profile into `out`.
pub fn bake_pours(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<FpPour>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let ctx = build_ctx(sketch, params_canonical);
    let mut open = false;
    let mut loops = TracedLoops::default();
    for entity in &sketch.entities {
        if entity.bake_skipped() {
            continue;
        }
        let attr = match entity.pour.as_ref() {
            Some(a) => a,
            None => continue,
        };
        if !matches!(
            entity.kind,
            EntityKind::Line { .. } | EntityKind::Arc { .. }
        ) {
            warnings.push(format!(
                "entity {}: PourAttr requires a Line or Arc seed (Circle seeds are not supported); skipping",
                entity.id
            ));
            continue;
        }
        let attribute = format!("{attr:?}");
        if loops.contains(&attribute, entity.id) {
            continue;
        }

        let vertices = match loops.trace(&attribute, sketch, solve, entity.id) {
            Ok(v) => v,
            Err(TraceError::OpenChain) => {
                open = true;
                continue;
            }
            Err(TraceError::Branching) => {
                warnings.push(format!(
                    "entity {}: PourAttr profile branches at a vertex; skipping",
                    entity.id
                ));
                continue;
            }
            Err(other) => {
                warnings.push(format!(
                    "entity {}: PourAttr trace failed ({other:?}); skipping",
                    entity.id
                ));
                continue;
            }
        };

        let clearance = match opt_eval_mm(&attr.clearance_expr, &ctx) {
            Ok(v) => v.unwrap_or(0.0),
            Err(e) => {
                warnings.push(format!(
                    "entity {}: PourAttr.clearance_expr failed to evaluate ({e}); pour skipped",
                    entity.id
                ));
                continue;
            }
        };
        let min_thickness = match opt_eval_mm(&attr.min_thickness_expr, &ctx) {
            Ok(v) => v.unwrap_or(0.0),
            Err(e) => {
                warnings.push(format!(
                    "entity {}: PourAttr.min_thickness_expr failed to evaluate ({e}); pour skipped",
                    entity.id
                ));
                continue;
            }
        };

        let Some(vertices) = shaped(vertices, attr) else {
            warnings.push(format!(
                "entity {}: PourAttr corners cannot be shaped at this size; pour skipped",
                entity.id
            ));
            continue;
        };
        out.push(FpPour {
            boundary: Polygon::new(vertices),
            layer: layer_id(attr.layer),
            net: NetRef(attr.net.clone()),
            fill_type: map_fill(attr.fill_type),
            thermal_relief: map_thermal(&attr.thermal_relief),
            clearance,
            min_thickness,
            priority: attr.priority.min(u8::MAX as u32) as u8,
        });
    }
    if open {
        warn_open_profiles(sketch, solve, "Pour", |e| e.pour.is_some(), warnings);
    }
    Ok(())
}

/// The traced outline with its convex corners shaped by the pour's Corners
/// (#2598), through the rule a board pour fills with (#2578,
/// `pcb_geom::shaped_outline_mm`). Sharp, or no positive size, keeps the
/// outline as traced; `None` when the geometry cannot hold it.
fn shaped(
    vertices: Vec<[f64; 2]>,
    attr: &signex_sketch::attr::PourAttr,
) -> Option<Vec<[f64; 2]>> {
    let Some((corners, size)) = attr.corner_shape() else {
        return Some(vertices);
    };
    let outline: Vec<Point> = vertices.iter().map(|&[x, y]| Point::new(x, y)).collect();
    let shaped = pcb_geom::shaped_outline_mm(&outline, corners, size)?;
    Some(shaped.into_iter().map(|p| [p.x, p.y]).collect())
}

fn map_fill(s: SkPourFill) -> LibPourFill {
    match s {
        SkPourFill::Solid => LibPourFill::Solid,
        SkPourFill::Hatched => LibPourFill::Hatched,
        SkPourFill::Outline => LibPourFill::None,
    }
}

fn map_thermal(t: &signex_sketch::attr::ThermalRelief) -> LibThermal {
    if !t.enabled {
        LibThermal::Direct
    } else if t.spoke_count == 0 {
        LibThermal::None
    } else {
        LibThermal::Spoke
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::{PourAttr, PourFillType, ThermalRelief};
    use signex_types::zone_pour_shape::PourCorners;
    use signex_sketch::entity::Entity;
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer};

    fn solve(sketch: &SketchData) -> FullSolveOutput {
        Solver::default()
            .solve(sketch, &ResolvedParams::new())
            .unwrap()
    }

    fn rectangle_with_pour(attr: PourAttr) -> SketchData {
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
        let l1 = SketchEntityId::new();
        let mut line1 = Entity::new(l1, plane, EntityKind::Line { start: p1, end: p2 });
        line1.pour = Some(attr);
        data.entities.push(line1);
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
    fn bake_pour_records_metadata() {
        let attr = PourAttr {
            layer: PcbLayer::Copper(CopperLayer::Top),
            net: Some("GND".into()),
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: Some("0.2mm".into()),
            min_thickness_expr: Some("0.15mm".into()),
            priority: 3,
            corners: Default::default(),
            corner_size: 0.0,
        };
        let data = rectangle_with_pour(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        let pour = &out[0];
        assert_eq!(pour.layer.as_str(), "Top Layer");
        assert_eq!(pour.net.0.as_deref(), Some("GND"));
        assert_eq!(pour.fill_type, LibPourFill::Solid);
        assert_eq!(pour.thermal_relief, LibThermal::Spoke);
        assert!((pour.clearance - 0.2).abs() < 1e-9);
        assert!((pour.min_thickness - 0.15).abs() < 1e-9);
        assert_eq!(pour.priority, 3);
        assert_eq!(pour.boundary.points.len(), 4);
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_pour_thermal_disabled_maps_to_direct() {
        let mut attr = PourAttr {
            layer: PcbLayer::Copper(CopperLayer::Top),
            net: None,
            fill_type: PourFillType::Hatched,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
            corners: Default::default(),
            corner_size: 0.0,
        };
        attr.thermal_relief.enabled = false;
        let data = rectangle_with_pour(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].thermal_relief, LibThermal::Direct);
        assert_eq!(out[0].fill_type, LibPourFill::Hatched);
    }

    #[test]
    fn bake_pour_outline_fill_maps_to_none() {
        let attr = PourAttr {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            net: None,
            fill_type: PourFillType::Outline,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
            corners: Default::default(),
            corner_size: 0.0,
        };
        let data = rectangle_with_pour(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].fill_type, LibPourFill::None);
        assert_eq!(out[0].layer.as_str(), "Bottom Layer");
    }

    #[test]
    fn tagging_every_edge_bakes_the_loop_once() {
        let attr = PourAttr {
            layer: PcbLayer::Copper(CopperLayer::Top),
            net: None,
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
            corners: Default::default(),
            corner_size: 0.0,
        };
        let mut data = rectangle_with_pour(attr.clone());
        for e in data
            .entities
            .iter_mut()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        {
            e.pour = Some(attr.clone());
        }
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    fn pour_attr(layer: PcbLayer, clearance: Option<&str>) -> PourAttr {
        PourAttr {
            layer,
            net: None,
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: clearance.map(String::from),
            min_thickness_expr: None,
            priority: 0,
            corners: Default::default(),
            corner_size: 0.0,
        }
    }

    #[test]
    fn one_loop_tagged_with_two_layers_bakes_once_per_layer() {
        let mut data = rectangle_with_pour(pour_attr(PcbLayer::Copper(CopperLayer::Top), None));
        let third = data
            .entities
            .iter_mut()
            .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
            .nth(2)
            .unwrap();
        third.pour = Some(pour_attr(PcbLayer::Copper(CopperLayer::Bottom), None));
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        let mut layers: Vec<&str> = out.iter().map(|o| o.layer.as_str()).collect();
        layers.sort();
        assert_eq!(layers, vec!["Bottom Layer", "Top Layer"]);
    }

    #[test]
    fn a_bare_number_clearance_is_read_as_millimetres() {
        let data = rectangle_with_pour(pour_attr(PcbLayer::Copper(CopperLayer::Top), Some("0.2")));
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1, "{warnings:?}");
        assert!((out[0].clearance - 0.2).abs() < 1e-9);
    }

    #[test]
    fn a_pour_with_an_unevaluable_clearance_is_skipped_not_baked_at_zero() {
        let data = rectangle_with_pour(pour_attr(
            PcbLayer::Copper(CopperLayer::Top),
            Some("= no_such_param"),
        ));
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert!(out.is_empty(), "{out:?}");
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("clearance_expr") && w.contains("skipped")),
            "{warnings:?}"
        );
    }

    /// The pour of the 5 mm square with `corners` of `size` mm, baked.
    fn baked_with(corners: PourCorners, size: f64) -> Vec<[f64; 2]> {
        let mut attr = pour_attr(PcbLayer::Copper(CopperLayer::Top), None);
        attr.corners = corners;
        attr.corner_size = size;
        let data = rectangle_with_pour(attr);
        let solved = solve(&data);
        let (mut out, mut warnings) = (Vec::new(), Vec::new());
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(out.len(), 1);
        out.remove(0).boundary.points
    }

    fn area(points: &[[f64; 2]]) -> f64 {
        let n = points.len();
        let twice: f64 = (0..n)
            .map(|i| {
                let ([x0, y0], [x1, y1]) = (points[i], points[(i + 1) % n]);
                x0 * y1 - x1 * y0
            })
            .sum();
        twice.abs() / 2.0
    }

    fn has_point(points: &[[f64; 2]], x: f64, y: f64) -> bool {
        points
            .iter()
            .any(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6)
    }

    /// #2598, decision 4 (Sharp default): a new pour bakes its outline as
    /// traced, and Rounded or Chamfer with no size does too.
    #[test]
    fn a_sharp_pour_bakes_its_outline_as_drawn() {
        let attr = pour_attr(PcbLayer::Copper(CopperLayer::Top), None);
        assert_eq!(attr.corners, PourCorners::Sharp);
        let sharp = baked_with(PourCorners::Sharp, 0.0);
        assert_eq!(sharp.len(), 4);
        assert!(has_point(&sharp, 0.0, 0.0) && has_point(&sharp, 5.0, 5.0));
        assert_eq!(baked_with(PourCorners::Rounded, 0.0), sharp);
        assert_eq!(baked_with(PourCorners::Chamfer, 0.0), sharp);
        assert_eq!(baked_with(PourCorners::Sharp, 1.0), sharp, "a size alone");
    }

    /// #2598: Chamfer cuts each corner back by the size along both edges.
    #[test]
    fn a_chamfered_pour_cuts_each_corner_by_its_size() {
        let points = baked_with(PourCorners::Chamfer, 1.0);
        for (x, y) in [(1.0, 0.0), (0.0, 1.0), (4.0, 5.0), (5.0, 4.0)] {
            assert!(has_point(&points, x, y), "({x}, {y}) in {points:?}");
        }
        assert!(!has_point(&points, 0.0, 0.0));
        assert!((area(&points) - 23.0).abs() < 1e-6, "{}", area(&points));
    }

    /// #2598: Rounded replaces each corner by an arc of the size, tangent
    /// to both edges, every point on the true circle, as on the board.
    #[test]
    fn a_rounded_pour_bakes_an_arc_of_its_radius_at_each_corner() {
        let points = baked_with(PourCorners::Rounded, 1.0);
        assert!(points.len() > 8, "{points:?}");
        assert!(has_point(&points, 1.0, 0.0) && has_point(&points, 0.0, 1.0));
        assert!(!has_point(&points, 0.0, 0.0));
        let near_origin = points.iter().filter(|p| p[0] < 1.0 && p[1] < 1.0);
        for p in near_origin {
            let r = (p[0] - 1.0).hypot(p[1] - 1.0);
            assert!((r - 1.0).abs() < 1e-5, "{p:?} is {r} from the centre");
        }
        let full = 25.0 - 4.0 * (1.0 - std::f64::consts::FRAC_PI_4);
        let got = area(&points);
        assert!(got < 25.0 && got > 23.0 && (got - full).abs() < 0.01, "{got}");
    }
}
