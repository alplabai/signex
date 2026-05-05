//! 3D-extrude bake — closed profile on a `PlaneKind::BodyTop` plane
//! becomes the `body_3d.outline` polygon (driving the procedural 3D
//! render with the actual body shape rather than the default fab
//! outline convex hull).
//!
//! Phase B / Stage 5 of the v0.14.1 sketch-mode plan. Per the v3
//! library schema:
//! ```text
//! struct Body3D {
//!     shape: BodyShape,           // Extrude / Dome / Cylinder / ...
//!     height_mm: f32,
//!     offset_z_mm: f32,           // <-- bake sets from plane's offset_z_expr
//!     outline: Option<Polygon>,   // <-- bake sets from closed profile
//!     ...
//! }
//! ```
//!
//! v0.14.1 scope:
//! - Find the first `PlaneKind::BodyTop` plane.
//! - Find the first non-construction Line / Arc on that plane.
//! - Trace a closed profile through the walker.
//! - Set `body_3d.outline = Some(Polygon)` and
//!   `body_3d.offset_z_mm = eval(plane.offset_z_expr)`.
//! - `height_mm` stays at whatever the caller had pre-set (Body3D
//!   defaults to 1.0 mm; user-edited values are preserved).
//! - Multiple BodyTop planes / multiple closed profiles per plane:
//!   first wins; subsequent emit warnings.

use std::collections::BTreeMap;
use std::collections::HashMap;

use signex_library::primitive::footprint::{Body3D, Polygon};
use signex_sketch::SketchError;
use signex_sketch::entity::EntityKind;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;

use crate::profile::{TraceError, trace_closed_profile};

pub fn bake_body3d(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    body_3d: &mut Body3D,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let body_top_planes: Vec<&signex_sketch::plane::Plane> = sketch
        .planes
        .iter()
        .filter(|p| matches!(p.kind, PlaneKind::BodyTop { .. }))
        .collect();
    if body_top_planes.is_empty() {
        return Ok(()); // no BodyTop plane → nothing to bake
    }
    if body_top_planes.len() > 1 {
        warnings.push(format!(
            "{} BodyTop planes found; only the first contributes to body_3d (multi-plane stack-up lands in v0.15)",
            body_top_planes.len()
        ));
    }
    let body_plane = body_top_planes[0];
    let plane_id = body_plane.id;
    let offset_z_expr = match &body_plane.kind {
        PlaneKind::BodyTop { offset_z_expr } => offset_z_expr.clone(),
        _ => unreachable!("filtered to BodyTop above"),
    };

    let seed = match find_seed_on_plane(sketch, plane_id) {
        Some(id) => id,
        None => return Ok(()), // BodyTop plane exists but no edges on it
    };

    match trace_closed_profile(sketch, solve, seed) {
        Ok(vertices) => {
            body_3d.outline = Some(Polygon::new(vertices));
            // Evaluate offset_z_expr to mm; on failure, leave whatever
            // the caller had set and warn.
            let ctx = build_ctx(params_canonical);
            match eval_mm(&offset_z_expr, &ctx) {
                Ok(z_mm) => {
                    // MD-9: guard against NaN / Inf / overflow before
                    // narrowing to f32. A typo like `=1e40mm` would
                    // otherwise produce ±Inf with no warning.
                    if !z_mm.is_finite() {
                        warnings.push(format!(
                            "BodyTop plane offset_z_expr `{offset_z_expr}` evaluated to non-finite {z_mm}; keeping prior offset_z_mm = {}",
                            body_3d.offset_z_mm
                        ));
                    } else if z_mm.abs() > f32::MAX as f64 {
                        warnings.push(format!(
                            "BodyTop plane offset_z_expr `{offset_z_expr}` evaluated to {z_mm} which exceeds f32 range; keeping prior offset_z_mm = {}",
                            body_3d.offset_z_mm
                        ));
                    } else {
                        body_3d.offset_z_mm = z_mm as f32;
                    }
                }
                Err(e) => warnings.push(format!(
                    "BodyTop plane offset_z_expr `{offset_z_expr}` failed to evaluate ({e}); keeping prior offset_z_mm = {}",
                    body_3d.offset_z_mm
                )),
            }
        }
        Err(TraceError::OpenChain) => warnings.push(
            "Body3D extrude: profile on BodyTop plane is not closed (open chain); keeping prior outline".into(),
        ),
        Err(TraceError::Branching) => warnings.push(
            "Body3D extrude: profile on BodyTop plane branches at a vertex; keeping prior outline".into(),
        ),
        Err(other) => warnings.push(format!(
            "Body3D extrude: trace failed ({other:?}); keeping prior outline"
        )),
    }
    Ok(())
}

fn find_seed_on_plane(sketch: &SketchData, plane_id: PlaneId) -> Option<SketchEntityId> {
    sketch
        .entities
        .iter()
        .find(|e| {
            e.plane == plane_id
                && !e.construction
                && matches!(e.kind, EntityKind::Line { .. } | EntityKind::Arc { .. })
        })
        .map(|e| e.id)
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

fn eval_mm(expr: &str, ctx: &EvalContext) -> Result<f64, String> {
    let s = expr.trim();
    let body = s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s);
    let ast = parse(body).map_err(|e| format!("parse: {e:?}"))?;
    let q = eval(&ast, ctx).map_err(|e| format!("eval: {e:?}"))?;
    q.as_mm().map_err(|e| format!("unit: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::primitive::footprint::Body3D;
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

    fn sketch_with_body_top_rectangle(offset_z_expr: &str) -> SketchData {
        let board = PlaneId::new();
        let body = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: board,
            kind: PlaneKind::BoardTop,
        });
        data.planes.push(Plane {
            id: body,
            kind: PlaneKind::BodyTop {
                offset_z_expr: offset_z_expr.into(),
            },
        });

        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let p3 = SketchEntityId::new();
        let p4 = SketchEntityId::new();
        data.entities.push(Entity::new(
            p1,
            body,
            EntityKind::Point { x: -1.0, y: -1.0 },
        ));
        data.entities
            .push(Entity::new(p2, body, EntityKind::Point { x: 1.0, y: -1.0 }));
        data.entities
            .push(Entity::new(p3, body, EntityKind::Point { x: 1.0, y: 1.0 }));
        data.entities
            .push(Entity::new(p4, body, EntityKind::Point { x: -1.0, y: 1.0 }));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            body,
            EntityKind::Line { start: p1, end: p2 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            body,
            EntityKind::Line { start: p2, end: p3 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            body,
            EntityKind::Line { start: p3, end: p4 },
        ));
        data.entities.push(Entity::new(
            SketchEntityId::new(),
            body,
            EntityKind::Line { start: p4, end: p1 },
        ));
        data
    }

    #[test]
    fn bake_body3d_no_body_top_plane_is_noop() {
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        });
        let solved = solve(&data);
        let mut body = Body3D::default();
        let original = body.clone();
        let mut warnings = Vec::new();
        bake_body3d(&data, &solved, &HashMap::new(), &mut body, &mut warnings).unwrap();
        assert_eq!(body, original, "no BodyTop plane → body_3d unchanged");
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_body3d_rectangle_outline() {
        let data = sketch_with_body_top_rectangle("0.5mm");
        let solved = solve(&data);
        let mut body = Body3D::default();
        let mut warnings = Vec::new();
        bake_body3d(&data, &solved, &HashMap::new(), &mut body, &mut warnings).unwrap();
        let outline = body.outline.as_ref().expect("outline baked");
        assert_eq!(outline.points.len(), 4);
        assert!((body.offset_z_mm - 0.5).abs() < 1e-6);
    }

    #[test]
    fn bake_body3d_offset_z_eval_failure_keeps_prior() {
        // BodyTop with a deliberately broken expression — outline still
        // bakes, offset_z stays at the prior value.
        let data = sketch_with_body_top_rectangle("(((bad");
        let solved = solve(&data);
        let mut body = Body3D::default();
        body.offset_z_mm = 7.5; // sentinel
        let mut warnings = Vec::new();
        bake_body3d(&data, &solved, &HashMap::new(), &mut body, &mut warnings).unwrap();
        assert!(body.outline.is_some(), "outline should still bake");
        assert_eq!(body.offset_z_mm, 7.5, "offset_z stays put when expr fails");
        assert!(
            warnings.iter().any(|w| w.contains("offset_z_expr")),
            "expected offset_z parse warning, got {warnings:?}"
        );
    }

    #[test]
    fn bake_body3d_no_edges_on_plane_is_noop() {
        // BodyTop plane exists but has no Lines/Arcs on it.
        let board = PlaneId::new();
        let body = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: board,
            kind: PlaneKind::BoardTop,
        });
        data.planes.push(Plane {
            id: body,
            kind: PlaneKind::BodyTop {
                offset_z_expr: "0.5mm".into(),
            },
        });
        let solved = solve(&data);
        let mut body3d = Body3D::default();
        let original = body3d.clone();
        let mut warnings = Vec::new();
        bake_body3d(&data, &solved, &HashMap::new(), &mut body3d, &mut warnings).unwrap();
        assert_eq!(body3d, original);
    }
}
