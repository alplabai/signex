//! Repair of a footprint's sketch as it is read from storage.
//!
//! Applied explicitly at each load site (the TOML container, the database
//! adapter, the library server's row read) rather than hidden in a serde
//! hook. The repair reports what it moved in [`Footprint::load_repairs`]; it
//! never logs, so previews, mounts and history reads stay quiet. The editor
//! that opens the footprint reports it once and marks itself dirty.

use signex_sketch::id::SketchEntityId;
use signex_sketch::snap_arc_ends_onto_circles;

use super::Footprint;

impl Footprint {
    /// Snap off-circle arc end points of this footprint's sketch onto their
    /// circles (see [`signex_sketch::snap_arc_ends_onto_circles`]) and record
    /// the adjusted arcs in [`Footprint::load_repairs`]. Call it on every
    /// footprint produced from stored bytes. Idempotent; linear in the size
    /// of the sketch.
    pub fn repair_stored_sketch(&mut self) {
        let Some(sketch) = self.sketch.as_mut() else {
            return;
        };
        let adjusted: Vec<SketchEntityId> = snap_arc_ends_onto_circles(sketch);
        self.load_repairs.extend(adjusted);
    }
}

#[cfg(test)]
mod tests {
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::solver::Solver;
    use signex_sketch::solver::residual::ResolvedParams;
    use signex_sketch::solver::state::point_xy;

    use super::super::{Footprint, FootprintFile};

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
        assert_eq!(loaded.load_repairs.len(), 1, "the load reports the arc");

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
    fn a_json_footprint_is_repaired_by_the_explicit_load_call_and_reports_it() {
        let (fp, [c, _, e]) = off_circle_footprint();
        let json = serde_json::to_string(&fp).expect("serialise");

        let mut back: Footprint = serde_json::from_str(&json).expect("parse");
        assert!(back.load_repairs.is_empty(), "serde alone repairs nothing");
        back.repair_stored_sketch();
        assert_eq!(back.load_repairs.len(), 1, "the one adjusted arc is named");
        back.repair_stored_sketch();
        assert_eq!(back.load_repairs.len(), 1, "a second call finds nothing");

        let sketch = back.sketch.expect("sketch");
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
        let (cx, cy) = at(c);
        let (ex, ey) = at(e);
        assert!(((ex - cx).hypot(ey - cy) - 5.0).abs() < 1e-9);
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
