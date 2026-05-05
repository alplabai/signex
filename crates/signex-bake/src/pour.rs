//! Pour bake — turns PourAttr-tagged closed sketch profiles into
//! `Footprint::pours: Vec<FpPour>` records.
//!
//! Phase B / Stage 3 of the v0.14 sketch-mode plan. v0.14 records the
//! polygon boundary + all PourAttr metadata (layer, net, fill_type,
//! thermal_relief, clearance, min_thickness, priority). Actual fill
//! generation (polygon offset + raster fill + thermal-relief geometry)
//! lands in v0.15.

use std::collections::BTreeMap;

use signex_library::primitive::footprint::{
    FpPour, LayerId, NetRef, Polygon, PourFillType as LibPourFill, ThermalReliefStyle as LibThermal,
};
use signex_sketch::SketchError;
use signex_sketch::attr::PourFillType as SkPourFill;
use signex_sketch::entity::EntityKind;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;
use std::collections::HashMap;

use crate::profile::{TraceError, trace_closed_profile};

/// Bake every PourAttr-tagged closed profile into `out`.
pub fn bake_pours(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<FpPour>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let ctx = build_ctx(params_canonical);
    for entity in &sketch.entities {
        if entity.construction {
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
                "entity {}: PourAttr requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }

        let vertices = match trace_closed_profile(sketch, solve, entity.id) {
            Ok(v) => v,
            Err(TraceError::OpenChain) => {
                warnings.push(format!(
                    "entity {}: PourAttr profile is not closed (open chain); skipping",
                    entity.id
                ));
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
                    "entity {}: PourAttr.clearance_expr failed to evaluate ({e}); using 0",
                    entity.id
                ));
                0.0
            }
        };
        let min_thickness = match opt_eval_mm(&attr.min_thickness_expr, &ctx) {
            Ok(v) => v.unwrap_or(0.0),
            Err(e) => {
                warnings.push(format!(
                    "entity {}: PourAttr.min_thickness_expr failed to evaluate ({e}); using 0",
                    entity.id
                ));
                0.0
            }
        };

        out.push(FpPour {
            boundary: Polygon::new(vertices),
            layer: LayerId::new(attr.layer.altium_label()),
            net: NetRef(attr.net.clone()),
            fill_type: map_fill(attr.fill_type),
            thermal_relief: map_thermal(&attr.thermal_relief),
            clearance,
            min_thickness,
            priority: attr.priority.min(u8::MAX as u32) as u8,
        });
    }
    Ok(())
}

fn build_ctx(params_canonical: &HashMap<String, f64>) -> EvalContext {
    let mut params: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, value) in params_canonical {
        params.insert(name.clone(), ExprNode::Literal(Quantity::length(*value)));
    }
    EvalContext {
        params,
        array_index: None,
    }
}

fn opt_eval_mm(expr: &Option<String>, ctx: &EvalContext) -> Result<Option<f64>, String> {
    let s = match expr.as_deref() {
        Some(s) => s.trim(),
        None => return Ok(None),
    };
    let body = s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s);
    let ast = parse(body).map_err(|e| format!("parse: {e:?}"))?;
    let q = eval(&ast, ctx).map_err(|e| format!("eval: {e:?}"))?;
    let mm = q.as_mm().map_err(|e| format!("unit: {e:?}"))?;
    Ok(Some(mm))
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
            layer: SignexLayer::TopCopper,
            net: Some("GND".into()),
            fill_type: PourFillType::Solid,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: Some("0.2mm".into()),
            min_thickness_expr: Some("0.15mm".into()),
            priority: 3,
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
            layer: SignexLayer::TopCopper,
            net: None,
            fill_type: PourFillType::Hatched,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
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
            layer: SignexLayer::BottomCopper,
            net: None,
            fill_type: PourFillType::Outline,
            thermal_relief: ThermalRelief::default(),
            clearance_expr: None,
            min_thickness_expr: None,
            priority: 0,
        };
        let data = rectangle_with_pour(attr);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_pours(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out[0].fill_type, LibPourFill::None);
        assert_eq!(out[0].layer.as_str(), "Bottom Layer");
    }
}
