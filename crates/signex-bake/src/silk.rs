//! Silkscreen bake — turns SilkAttr-tagged sketch entities into
//! `FpGraphic` Line / Arc / Circle entries on the matching layer
//! (`F.SilkS` / `B.SilkS` typically).
//!
//! Phase B / Stage 3 of the v0.14 sketch-mode plan. Unlike courtyard /
//! mask / pour bakes, silk does NOT require closed-profile tracing —
//! each Line / Arc / Circle entity tagged with [`SilkAttr`] emits one
//! `FpGraphic` directly. Open paths are valid silkscreen geometry
//! (e.g. component outline reference marks).
//!
//! v0.14 scope:
//! - `EntityKind::Line` → `FpGraphicKind::Line { from, to }`
//! - `EntityKind::Arc`  → `FpGraphicKind::Arc { center, radius,
//!     start_deg, end_deg }`
//! - `EntityKind::Circle` → `FpGraphicKind::Circle { center, radius }`
//! - `EntityKind::Point` carrying a SilkAttr emits no graphic and
//!   triggers a warning (a Point is not a renderable silk primitive).
//! - Construction entities are skipped silently.
//!
//! Layer routing: SilkAttr.layer is used directly. `TopSilk` →
//! `silk_f`, `BottomSilk` → `silk_b`. Other layers (e.g. someone tags
//! a SilkAttr with TopAssembly) emit a warning and skip — silk
//! semantics only make sense on silk layers.
//!
//! Cleanroom: per-entity translation only; no third-party
//! footprint-generator source consulted.

use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_sketch::SketchError;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;
use signex_types::layer::SignexLayer;

/// Bake every SilkAttr-tagged non-construction entity into an
/// `FpGraphic` on `silk_f` or `silk_b`. Both Vecs are passed by mut-ref
/// and appended to (the caller decides whether to clear them first).
///
/// Returns `Ok(())` even when individual entities skip — those are
/// reported via the `warnings` Vec instead.
pub fn bake_silk(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    silk_f: &mut Vec<FpGraphic>,
    silk_b: &mut Vec<FpGraphic>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    for entity in &sketch.entities {
        if entity.bake_skipped() {
            continue;
        }
        let attr = match entity.silk.as_ref() {
            Some(a) => a,
            None => continue,
        };

        let target = match attr.layer {
            SignexLayer::TopSilk => &mut *silk_f,
            SignexLayer::BottomSilk => &mut *silk_b,
            other => {
                warnings.push(format!(
                    "entity {}: SilkAttr.layer = {:?} is not a silk layer; skipping bake (use TopSilk or BottomSilk)",
                    entity.id, other
                ));
                continue;
            }
        };

        match entity_to_graphic(entity, sketch, solve) {
            Ok(Some(g)) => target.push(g),
            Ok(None) => {
                warnings.push(format!(
                    "entity {}: SilkAttr on a Point is meaningless (no renderable primitive); skipping",
                    entity.id
                ));
            }
            Err(msg) => warnings.push(format!("entity {}: silk bake skipped — {}", entity.id, msg)),
        }
    }
    Ok(())
}

/// Translate a single edge entity to an `FpGraphic`. Returns
/// `Ok(None)` for non-edge entities (Points), `Err(...)` if a
/// referenced endpoint position can't be resolved.
fn entity_to_graphic(
    entity: &Entity,
    sketch: &SketchData,
    solve: &FullSolveOutput,
) -> Result<Option<FpGraphic>, String> {
    match entity.kind {
        EntityKind::Point { .. } => Ok(None),
        EntityKind::Line { start, end } => {
            let from = pos(sketch, solve, start)?;
            let to = pos(sketch, solve, end)?;
            Ok(Some(FpGraphic {
                kind: FpGraphicKind::Line { from, to },
                stroke_width: DEFAULT_SILK_STROKE_MM,
                filled: false,
            }))
        }
        EntityKind::Circle { center, radius } => {
            let c = pos(sketch, solve, center)?;
            Ok(Some(FpGraphic {
                kind: FpGraphicKind::Circle { center: c, radius },
                stroke_width: DEFAULT_SILK_STROKE_MM,
                filled: false,
            }))
        }
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => {
            let c = pos(sketch, solve, center)?;
            let s = pos(sketch, solve, start)?;
            let e = pos(sketch, solve, end)?;
            let radius = ((s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2)).sqrt();
            let mut start_deg = (s[1] - c[1]).atan2(s[0] - c[0]).to_degrees();
            let mut end_deg = (e[1] - c[1]).atan2(e[0] - c[0]).to_degrees();
            if start_deg < 0.0 {
                start_deg += 360.0;
            }
            if end_deg < 0.0 {
                end_deg += 360.0;
            }
            // CW arcs are encoded by start > end (renderer chooses the
            // short-arc direction otherwise). For now the bake just
            // records the raw degrees + a swap when sweep_ccw is false
            // and start < end, matching the FpGraphicKind::Arc
            // contract used by the v0.13 schematic renderer.
            if !sweep_ccw && start_deg < end_deg {
                std::mem::swap(&mut start_deg, &mut end_deg);
            }
            Ok(Some(FpGraphic {
                kind: FpGraphicKind::Arc {
                    center: c,
                    radius,
                    start_deg,
                    end_deg,
                },
                stroke_width: DEFAULT_SILK_STROKE_MM,
                filled: false,
            }))
        }
    }
}

/// Default silkscreen stroke width — IPC-7351 says 0.15 mm is the
/// fab-grade lower bound; we use 0.12 mm as the Signex default
/// (matches the v1 fixtures' `stroke_width = 0.12`).
const DEFAULT_SILK_STROKE_MM: f64 = 0.12;

fn pos(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    id: signex_sketch::id::SketchEntityId,
) -> Result<[f64; 2], String> {
    let (x, y) = point_xy(id, &solve.result.state, &solve.result.index, sketch)
        .ok_or_else(|| format!("missing endpoint Point {id}"))?;
    Ok([x, y])
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::SilkAttr;
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

    fn empty_sketch() -> (SketchData, PlaneId) {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        (data, plane)
    }

    #[test]
    fn bake_silk_line_to_top_silk() {
        let (mut data, plane) = empty_sketch();
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
        line.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
        data.entities.push(line);

        let solved = solve(&data);
        let mut silk_f = Vec::new();
        let mut silk_b = Vec::new();
        let mut warnings = Vec::new();
        bake_silk(&data, &solved, &mut silk_f, &mut silk_b, &mut warnings).unwrap();

        assert!(silk_b.is_empty());
        assert_eq!(silk_f.len(), 1);
        assert!(warnings.is_empty());
        match &silk_f[0].kind {
            FpGraphicKind::Line { from, to } => {
                assert_eq!(*from, [0.0, 0.0]);
                assert_eq!(*to, [5.0, 0.0]);
            }
            other => panic!("expected Line graphic, got {other:?}"),
        }
    }

    #[test]
    fn bake_silk_circle_to_bottom_silk() {
        let (mut data, plane) = empty_sketch();
        let pc = SketchEntityId::new();
        data.entities
            .push(Entity::new(pc, plane, EntityKind::Point { x: 1.0, y: 1.0 }));
        let mut circle = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Circle {
                center: pc,
                radius: 0.5,
            },
        );
        circle.silk = Some(SilkAttr {
            layer: SignexLayer::BottomSilk,
        });
        data.entities.push(circle);

        let solved = solve(&data);
        let mut silk_f = Vec::new();
        let mut silk_b = Vec::new();
        let mut warnings = Vec::new();
        bake_silk(&data, &solved, &mut silk_f, &mut silk_b, &mut warnings).unwrap();

        assert!(silk_f.is_empty());
        assert_eq!(silk_b.len(), 1);
        assert!(matches!(silk_b[0].kind, FpGraphicKind::Circle { .. }));
    }

    #[test]
    fn bake_silk_construction_skipped() {
        let (mut data, plane) = empty_sketch();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        line.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
        line.construction = true;
        data.entities.push(line);

        let solved = solve(&data);
        let mut silk_f = Vec::new();
        let mut silk_b = Vec::new();
        let mut warnings = Vec::new();
        bake_silk(&data, &solved, &mut silk_f, &mut silk_b, &mut warnings).unwrap();

        assert!(silk_f.is_empty(), "construction entity must not bake");
        assert!(warnings.is_empty(), "construction skip is silent");
    }

    #[test]
    fn bake_silk_wrong_layer_warns() {
        let (mut data, plane) = empty_sketch();
        let p1 = SketchEntityId::new();
        let p2 = SketchEntityId::new();
        data.entities
            .push(Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
        data.entities
            .push(Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }));
        let mut line = Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line { start: p1, end: p2 },
        );
        // Wrong: SilkAttr on a copper layer.
        line.silk = Some(SilkAttr {
            layer: SignexLayer::TopCopper,
        });
        data.entities.push(line);

        let solved = solve(&data);
        let mut silk_f = Vec::new();
        let mut silk_b = Vec::new();
        let mut warnings = Vec::new();
        bake_silk(&data, &solved, &mut silk_f, &mut silk_b, &mut warnings).unwrap();

        assert!(silk_f.is_empty());
        assert!(silk_b.is_empty());
        assert!(
            warnings.iter().any(|w| w.contains("not a silk layer")),
            "expected wrong-layer warning, got {warnings:?}"
        );
    }
}
