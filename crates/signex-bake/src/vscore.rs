//! V-score bake — turns VScoreHintAttr-tagged Line entities into
//! `Footprint::v_scores: Vec<FpVScore>` records.
//!
//! Phase B / Stage 4 of the v0.14.1 sketch-mode plan. A V-score is a
//! single straight scoring line on the PCB surface — it's NOT a
//! closed profile, so the walker is not used. Each tagged Line emits
//! one record.
//!
//! The sketch-side `VScoreHintAttr` carries `depth_fraction_expr`
//! (depth as a fraction of board thickness, evaluated against the
//! parameter table) and an optional `min_web_expr`. v0.14.1 evaluates
//! both into mm; the depth_fraction is multiplied by a nominal
//! [`NOMINAL_BOARD_THICKNESS_MM`] to get an absolute depth (the lib
//! field is mm, not a fraction). Real fab houses will substitute the
//! actual board thickness at panelisation time.

use std::collections::BTreeMap;

use signex_library::primitive::footprint::FpVScore;
use signex_sketch::entity::EntityKind;
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{eval, EvalContext};
use signex_sketch::expr::parse::parse;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::state::point_xy;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::Quantity;
use signex_sketch::SketchError;
use std::collections::HashMap;

/// Nominal board thickness used to convert `depth_fraction` to mm.
/// 1.6 mm is the IPC-A-600 default board thickness for FR-4.
const NOMINAL_BOARD_THICKNESS_MM: f64 = 1.6;

pub fn bake_v_scores(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<FpVScore>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let ctx = build_ctx(params_canonical);
    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        let attr = match entity.v_score.as_ref() {
            Some(a) => a,
            None => continue,
        };
        let (start, end) = match entity.kind {
            EntityKind::Line { start, end } => (start, end),
            _ => {
                warnings.push(format!(
                    "entity {}: VScoreHintAttr requires a Line entity (Arcs / Circles ignored — V-scores are straight cuts); skipping",
                    entity.id
                ));
                continue;
            }
        };

        let from = match point_xy(start, &solve.result.state, &solve.result.index, sketch) {
            Some(p) => [p.0, p.1],
            None => {
                warnings.push(format!(
                    "entity {}: VScoreHintAttr start endpoint missing; skipping",
                    entity.id
                ));
                continue;
            }
        };
        let to = match point_xy(end, &solve.result.state, &solve.result.index, sketch) {
            Some(p) => [p.0, p.1],
            None => {
                warnings.push(format!(
                    "entity {}: VScoreHintAttr end endpoint missing; skipping",
                    entity.id
                ));
                continue;
            }
        };

        let depth = match eval_dimensionless(&attr.depth_fraction_expr, &ctx) {
            Ok(frac) => frac.clamp(0.0, 1.0) * NOMINAL_BOARD_THICKNESS_MM,
            Err(e) => {
                warnings.push(format!(
                    "entity {}: VScoreHintAttr.depth_fraction_expr failed to evaluate ({e}); using nominal 1/3",
                    entity.id
                ));
                NOMINAL_BOARD_THICKNESS_MM / 3.0
            }
        };

        if attr.min_web_expr.is_some() {
            warnings.push(format!(
                "entity {}: VScoreHintAttr.min_web_expr ignored — no min-web lib field in v0.14.1",
                entity.id
            ));
        }

        out.push(FpVScore {
            line: [from, to],
            depth,
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

fn eval_dimensionless(expr: &str, ctx: &EvalContext) -> Result<f64, String> {
    let s = expr.trim();
    let body = s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s);
    let ast = parse(body).map_err(|e| format!("parse: {e:?}"))?;
    let q = eval(&ast, ctx).map_err(|e| format!("eval: {e:?}"))?;
    q.as_count().map_err(|e| format!("unit: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::{VScoreHintAttr, VScoreSide};
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

    #[test]
    fn bake_v_score_horizontal_line() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 50.0, y: 0.0 }));
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        line.v_score = Some(VScoreHintAttr {
            depth_fraction_expr: "0.5".into(),
            min_web_expr: None,
            side: VScoreSide::Both,
        });
        data.entities.push(line);

        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_v_scores(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].line[0], [0.0, 0.0]);
        assert_eq!(out[0].line[1], [50.0, 0.0]);
        // 0.5 fraction * 1.6 mm = 0.8 mm depth.
        assert!((out[0].depth - 0.8).abs() < 1e-9);
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_v_score_clamps_depth_fraction() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }));
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        // Out-of-range depth fraction.
        line.v_score = Some(VScoreHintAttr {
            depth_fraction_expr: "1.5".into(),
            min_web_expr: None,
            side: VScoreSide::Top,
        });
        data.entities.push(line);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_v_scores(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        // Clamped to 1.0 → 1.6 mm.
        assert!((out[0].depth - 1.6).abs() < 1e-9);
    }

    #[test]
    fn bake_v_score_min_web_warns() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }));
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        line.v_score = Some(VScoreHintAttr {
            depth_fraction_expr: "0.333".into(),
            min_web_expr: Some("0.4mm".into()),
            side: VScoreSide::Both,
        });
        data.entities.push(line);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_v_scores(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert!(warnings.iter().any(|w| w.contains("min_web_expr")));
    }

    #[test]
    fn bake_v_score_arc_skipped() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        let pc = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 0.5, y: 0.0 }));
        let mut arc = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Arc {
                center: pc,
                start: p1,
                end: p2,
                sweep_ccw: true,
            },
        );
        arc.v_score = Some(VScoreHintAttr {
            depth_fraction_expr: "0.333".into(),
            min_web_expr: None,
            side: VScoreSide::Both,
        });
        data.entities.push(arc);
        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_v_scores(&data, &solved, &HashMap::new(), &mut out, &mut warnings).unwrap();
        assert!(out.is_empty(), "v-score on arc must skip");
        assert!(warnings.iter().any(|w| w.contains("requires a Line")));
    }
}
