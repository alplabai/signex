//! Repairs applied to a footprint's sketch as its file is read.

use signex_sketch::{SketchData, snap_arc_ends_onto_circles};

/// Snap off-circle arc end points onto their circles (see
/// [`signex_sketch::snap_arc_ends_onto_circles`]) and report the adjusted
/// arcs once, so the Messages panel names what changed. This is the single
/// place a stored sketch is repaired; every consumer reads the result.
pub(super) fn repair_loaded_sketch(
    footprint_name: &str,
    sketch: Option<SketchData>,
) -> Option<SketchData> {
    let mut sketch = sketch?;
    let adjusted = snap_arc_ends_onto_circles(&mut sketch);
    if !adjusted.is_empty() {
        let arcs: Vec<String> = adjusted.iter().map(ToString::to_string).collect();
        tracing::warn!(
            target: "signex::sketch",
            "footprint \"{footprint_name}\": moved the end point of {} arc(s) onto its circle \
             (centre and start unchanged): {}",
            arcs.len(),
            arcs.join(", ")
        );
    }
    Some(sketch)
}

#[cfg(test)]
mod tests {
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_sketch::solver::state::point_xy;

    use super::super::{Footprint, FootprintFile};
    use super::*;

    fn off_circle_footprint() -> (Footprint, [SketchEntityId; 3]) {
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        let pid = plane.id;
        let mut sketch = SketchData::default();
        sketch.planes.push(plane);
        let mut point = |x, y| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, pid, EntityKind::Point { x, y }));
            id
        };
        let ids = [point(2.0, 3.0), point(7.0, 3.0), point(2.0, 10.0)];
        sketch.entities.push(Entity::new(
            SketchEntityId::new(),
            pid,
            EntityKind::Arc {
                center: ids[0],
                start: ids[1],
                end: ids[2],
                sweep_ccw: true,
            },
        ));
        let mut fp = Footprint::empty("arc");
        fp.sketch = Some(sketch);
        (fp, ids)
    }

    fn reload(fp: Footprint) -> Footprint {
        let text = FootprintFile::from_footprint(fp)
            .to_toml_string()
            .expect("serialise");
        FootprintFile::from_toml_str(&text)
            .expect("parse")
            .footprints
            .remove(0)
    }

    #[test]
    fn an_off_circle_arc_loads_with_centre_and_start_unchanged_and_end_on_the_circle() {
        let (fp, [c, s, e]) = off_circle_footprint();

        let loaded = reload(fp);

        let sketch = loaded.sketch.expect("sketch survives the round trip");
        let at = |id| {
            sketch
                .entities
                .iter()
                .find_map(|en| match en.kind {
                    EntityKind::Point { x, y } if en.id == id => Some((x, y)),
                    _ => None,
                })
                .expect("point")
        };
        assert_eq!(at(c), (2.0, 3.0));
        assert_eq!(at(s), (7.0, 3.0));
        let (ex, ey) = at(e);
        let reach = (ex - 2.0).hypot(ey - 3.0);
        assert!((reach - 5.0).abs() < 1e-9, "|E-C| = {reach}");
    }

    #[test]
    fn a_solve_after_loading_leaves_centre_and_start_unmoved() {
        let (fp, [c, s, _]) = off_circle_footprint();
        let sketch = reload(fp).sketch.expect("sketch");

        let out = Solver::default()
            .solve(&sketch, &ResolvedParams::new())
            .expect("a repaired arc alone is satisfiable");

        let at = |p| point_xy(p, &out.result.state, &out.result.index, &sketch).unwrap();
        assert_eq!(at(c), (2.0, 3.0));
        assert_eq!(at(s), (7.0, 3.0));
    }
}
