//! Board cutout bake — turns BoardCutoutAttr-tagged closed profiles
//! into `Footprint::cutouts: Vec<FpCutout>` records.
//!
//! v0.14.1 records the polygon boundary; PCB outline subtraction
//! (the geometric op that combines cutouts with the board outline)
//! runs at PCB gerber-export time and is out of scope here.
//!
//! v0.15 — `edge_radius_mm` (corner fillet radius) and `through`
//! (full-depth vs partial-depth) now propagate from
//! `BoardCutoutAttr` into `FpCutout`. The corner radius expression
//! is evaluated to mm via the parameter table; eval failure falls
//! back to `0.0` (sharp corner) with a warning.

use std::collections::BTreeMap;
use std::collections::HashMap;

use signex_library::primitive::footprint::{FpCutout, Polygon};
use signex_sketch::SketchError;
use signex_sketch::entity::EntityKind;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;

use crate::profile::{TraceError, trace_closed_profile};

pub fn bake_cutouts(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<FpCutout>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let ctx = build_ctx(params_canonical);
    for entity in &sketch.entities {
        if entity.bake_skipped() {
            continue;
        }
        let attr = match entity.board_cutout.as_ref() {
            Some(a) => a,
            None => continue,
        };
        if !matches!(
            entity.kind,
            EntityKind::Line { .. } | EntityKind::Arc { .. }
        ) {
            warnings.push(format!(
                "entity {}: BoardCutoutAttr requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }

        match trace_closed_profile(sketch, solve, entity.id) {
            Ok(vertices) => {
                // v0.15 — evaluate the corner-radius expression into
                // mm. Empty expression → 0 (sharp corner); eval
                // failure → 0 + warning so the bake doesn't silently
                // drop the user's authoring intent.
                let edge_radius_mm = match opt_eval_mm(&attr.edge_radius_expr, &ctx) {
                    Ok(Some(v)) => v,
                    Ok(None) => 0.0,
                    Err(e) => {
                        warnings.push(format!(
                            "entity {}: BoardCutoutAttr.edge_radius_expr failed to evaluate ({e}); using 0",
                            entity.id
                        ));
                        0.0
                    }
                };
                out.push(FpCutout {
                    boundary: Polygon::new(vertices),
                    edge_radius_mm,
                    through: attr.through,
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

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::BoardCutoutAttr;
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
        bake_cutouts(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].boundary.points.len(), 4);
        assert_eq!(out[0].edge_radius_mm, 0.0);
        assert!(out[0].through);
        assert!(
            warnings.is_empty(),
            "default cutout should bake without warnings, got {warnings:?}"
        );
    }

    #[test]
    fn bake_cutout_edge_radius_evaluated() {
        let data = rectangle_with_cutout(BoardCutoutAttr {
            edge_radius_expr: Some("1mm".into()),
            through: true,
        });
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_cutouts(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!((out[0].edge_radius_mm - 1.0).abs() < 1e-9);
        assert!(
            warnings.is_empty(),
            "v0.15 — edge_radius is now baked, no warning expected; got {warnings:?}"
        );
    }

    #[test]
    fn bake_cutout_partial_depth_baked() {
        let data = rectangle_with_cutout(BoardCutoutAttr {
            edge_radius_expr: None,
            through: false,
        });
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_cutouts(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!(!out[0].through, "v0.15 — partial-depth flag now propagates");
        assert!(warnings.is_empty());
    }
}
