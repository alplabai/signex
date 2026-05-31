//! Mask + paste-aperture bake — turns MaskOpeningAttr,
//! MaskExcludeAttr, and PasteApertureAttr-tagged closed profiles into
//! `Footprint::mask_openings`, `mask_excludes`, and `paste_apertures`.
//!
//! Phase B / Stage 3 of the v0.14 sketch-mode plan. All three follow
//! the same recipe: walker → polygon, attr.layer → LayerId, append to
//! the corresponding output Vec.
//!
//! v0.14 scope: Lines only (walker limitation). Arcs / Circles in a
//! profile surface a warning and skip. Construction entities skipped.

use signex_library::primitive::footprint::{
    FpMaskExclude, FpMaskOpening, FpPasteAperture, LayerId, Polygon,
};
use signex_sketch::SketchError;
use signex_sketch::entity::EntityKind;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::layer::SignexLayer;

use crate::profile::{TraceError, trace_closed_profile};

/// Bake every MaskOpeningAttr-tagged closed profile.
pub fn bake_mask_openings(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<FpMaskOpening>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    bake_layered::<_, _>(
        sketch,
        solve,
        out,
        warnings,
        |e| e.mask_opening.as_ref().map(|a| a.layer),
        |boundary, layer| FpMaskOpening { boundary, layer },
        "MaskOpeningAttr",
    )
}

/// Bake every MaskExcludeAttr-tagged closed profile.
pub fn bake_mask_excludes(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<FpMaskExclude>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    bake_layered::<_, _>(
        sketch,
        solve,
        out,
        warnings,
        |e| e.mask_exclude.as_ref().map(|a| a.layer),
        |boundary, layer| FpMaskExclude { boundary, layer },
        "MaskExcludeAttr",
    )
}

/// Bake every PasteApertureAttr-tagged closed profile.
pub fn bake_paste_apertures(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<FpPasteAperture>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    bake_layered::<_, _>(
        sketch,
        solve,
        out,
        warnings,
        |e| e.paste_aperture.as_ref().map(|a| a.layer),
        |boundary, layer| FpPasteAperture { boundary, layer },
        "PasteApertureAttr",
    )
}

/// Generic helper: for every non-construction Line entity where
/// `attr_extract` returns Some(layer), trace the closed profile and
/// push `make(boundary, layer_id)` onto `out`. Skipped entities
/// produce a warning that names the attr (`attr_name`).
fn bake_layered<O, F>(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    out: &mut Vec<O>,
    warnings: &mut Vec<String>,
    attr_extract: impl Fn(&signex_sketch::entity::Entity) -> Option<SignexLayer>,
    make: F,
    attr_name: &'static str,
) -> Result<(), SketchError>
where
    F: Fn(Polygon, LayerId) -> O,
{
    let mut visited_seeds: std::collections::HashSet<signex_sketch::id::SketchEntityId> =
        std::collections::HashSet::new();
    for entity in &sketch.entities {
        if entity.bake_skipped() {
            continue;
        }
        let layer = match attr_extract(entity) {
            Some(l) => l,
            None => continue,
        };
        if !matches!(
            entity.kind,
            EntityKind::Line { .. } | EntityKind::Arc { .. }
        ) {
            warnings.push(format!(
                "entity {}: {attr_name} requires a Line or Arc seed (Circles land in v0.14.2); skipping",
                entity.id
            ));
            continue;
        }
        if visited_seeds.contains(&entity.id) {
            continue;
        }

        match trace_closed_profile(sketch, solve, entity.id) {
            Ok(vertices) => {
                let layer_id = LayerId::new(layer.altium_label());
                out.push(make(Polygon::new(vertices), layer_id));
                visited_seeds.insert(entity.id);
            }
            Err(TraceError::OpenChain) => warnings.push(format!(
                "entity {}: {attr_name} profile is not closed (open chain); skipping",
                entity.id
            )),
            Err(TraceError::Branching) => warnings.push(format!(
                "entity {}: {attr_name} profile branches at a vertex; skipping",
                entity.id
            )),
            Err(other) => warnings.push(format!(
                "entity {}: {attr_name} trace failed ({other:?}); skipping",
                entity.id
            )),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::attr::{MaskExcludeAttr, MaskOpeningAttr, PasteApertureAttr};
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

    fn rectangle(plane: PlaneId, data: &mut SketchData) -> SketchEntityId {
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
        let l2 = SketchEntityId::new();
        let l3 = SketchEntityId::new();
        let l4 = SketchEntityId::new();
        data.entities.push(Entity::new(
            l1,
            plane,
            EntityKind::Line { start: p1, end: p2 },
        ));
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
        l1
    }

    #[test]
    fn bake_mask_opening_rectangle() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let l1 = rectangle(plane, &mut data);
        let l1_entity = data.entities.iter_mut().find(|e| e.id == l1).unwrap();
        l1_entity.mask_opening = Some(MaskOpeningAttr {
            layer: SignexLayer::TopSolderMask,
        });

        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_mask_openings(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].layer.as_str(), "Top Solder");
        assert_eq!(out[0].boundary.points.len(), 4);
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_mask_exclude_rectangle() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let l1 = rectangle(plane, &mut data);
        let l1_entity = data.entities.iter_mut().find(|e| e.id == l1).unwrap();
        l1_entity.mask_exclude = Some(MaskExcludeAttr {
            layer: SignexLayer::BottomSolderMask,
        });

        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_mask_excludes(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].layer.as_str(), "Bottom Solder");
        assert!(warnings.is_empty());
    }

    #[test]
    fn bake_paste_aperture_rectangle() {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let l1 = rectangle(plane, &mut data);
        let l1_entity = data.entities.iter_mut().find(|e| e.id == l1).unwrap();
        l1_entity.paste_aperture = Some(PasteApertureAttr {
            layer: SignexLayer::TopPaste,
        });

        let solved = solve(&data);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        bake_paste_apertures(&data, &solved, &mut out, &mut warnings).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].layer.as_str(), "Top Paste");
        assert_eq!(out[0].boundary.points.len(), 4);
        assert!(warnings.is_empty());
    }
}
