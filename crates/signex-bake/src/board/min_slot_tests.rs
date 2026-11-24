//! mockup row 179d — the min slot check: a Slot narrower than the fab's
//! min slot warns and names its place; one at or above it passes.

use signex_sketch::Solver;
use signex_sketch::attr::BoardCutoutAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::sketch::board_attr::BoardSlotAttr;

use super::super::checks::{
    CheckState, FabLimits, Finding, NotChecked, OutlineCheck, OutlineChecks, outline_checks_with,
    outline_checks_with_limits,
};
use crate::board::bake_board;

struct Sketch {
    data: SketchData,
    plane: PlaneId,
}

impl Sketch {
    fn new() -> Self {
        let plane = PlaneId::new();
        let mut data = SketchData::default();
        data.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        Sketch { data, plane }
    }

    fn add(&mut self, kind: EntityKind) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data.entities.push(Entity::new(id, self.plane, kind));
        id
    }

    fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        self.add(EntityKind::Point { x, y })
    }

    fn get(&mut self, id: SketchEntityId) -> &mut Entity {
        self.data.entities.iter_mut().find(|e| e.id == id).unwrap()
    }

    /// A slot `width` across along X from (`x`, `y`), 5 mm between its
    /// arc centres: bottom line, right arc, top line, left arc. Returns
    /// the four edges, untagged.
    fn slot_edges(&mut self, x: f64, y: f64, width: f64) -> Vec<SketchEntityId> {
        let (r, s) = (width / 2.0, 5.0);
        let (a, b) = (self.point(x, y), self.point(x + s, y));
        let (c, d) = (self.point(x + s, y + width), self.point(x, y + width));
        let (right, left) = (self.point(x + s, y + r), self.point(x, y + r));
        let arc = |center, start, end| EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw: true,
        };
        let bottom = self.add(EntityKind::Line { start: a, end: b });
        let right_arc = self.add(arc(right, b, c));
        let top = self.add(EntityKind::Line { start: c, end: d });
        let left_arc = self.add(arc(left, d, a));
        vec![bottom, right_arc, top, left_arc]
    }

    /// [`Self::slot_edges`] tagged as a Slot cutout on its first edge.
    fn slot(&mut self, x: f64, y: f64, width: f64) -> SketchEntityId {
        let edges = self.slot_edges(x, y, width);
        let seed = edges[0];
        self.get(seed).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        self.get(seed).board_slot = Some(BoardSlotAttr::default());
        seed
    }

    fn checks(&self, min_slot: Option<f64>) -> OutlineChecks {
        let params = resolve(&self.data.parameters).unwrap();
        let solve = Solver::default().solve(&self.data, &params).unwrap();
        let fab = FabLimits {
            router_tool_diameter: None,
            min_slot,
        };
        outline_checks_with_limits(&self.data, &solve, &solve.params, fab)
    }
}

#[test]
fn a_slot_under_the_min_slot_warns_and_names_its_place() {
    let mut s = Sketch::new();
    let seed = s.slot(10.0, 10.0, 0.6);

    let checks = s.checks(Some(0.8));

    let CheckState::Failed(found) = &checks.min_slot else {
        panic!("expected a failure, got {:?}", checks.min_slot);
    };
    assert_eq!(found.len(), 1, "{found:?}");
    let Finding::NarrowSlot {
        entity,
        width_mm,
        min_slot_mm,
    } = found[0]
    else {
        panic!("a narrow slot: {found:?}");
    };
    assert_eq!(entity, seed);
    assert!((width_mm - 0.6).abs() < 1e-6, "{width_mm}");
    assert_eq!(min_slot_mm, 0.8);
    assert_eq!(found[0].place(), [seed], "a click selects the slot");
    let failures: Vec<_> = checks.failures().map(|(check, _)| check).collect();
    assert_eq!(failures, [OutlineCheck::MinSlot]);
}

#[test]
fn a_slot_at_or_above_the_min_slot_passes() {
    for width in [0.8, 1.2] {
        let mut s = Sketch::new();
        s.slot(10.0, 10.0, width);
        assert_eq!(s.checks(Some(0.8)).min_slot, CheckState::Passed, "{width}");
    }
}

#[test]
fn every_narrow_slot_is_named_once() {
    let mut s = Sketch::new();
    let narrow = s.slot(0.0, 0.0, 0.5);
    s.slot(0.0, 10.0, 0.8);
    let other = s.slot(0.0, 20.0, 0.4);
    // A second edge of the same loop tagged too: still one finding.
    let edges = s.slot_edges(0.0, 30.0, 0.3);
    for &edge in &edges[..2] {
        s.get(edge).board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
    }
    s.get(edges[0]).board_slot = Some(BoardSlotAttr::default());

    let CheckState::Failed(found) = s.checks(Some(0.6)).min_slot else {
        panic!("narrow slots fail");
    };
    let places: Vec<SketchEntityId> = found.iter().map(|f| f.place()[0]).collect();
    assert_eq!(places.len(), 3, "{found:?}");
    assert!(places.contains(&narrow) && places.contains(&other));
}

#[test]
fn without_a_min_slot_the_check_says_why() {
    let mut s = Sketch::new();
    s.slot(10.0, 10.0, 0.3);
    assert_eq!(
        s.checks(None).min_slot,
        CheckState::NotChecked(NotChecked::NoMinSlot)
    );
    for bad in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            s.checks(Some(bad)).min_slot,
            CheckState::NotChecked(NotChecked::NoMinSlot),
            "{bad}"
        );
    }
    let params = resolve(&s.data.parameters).unwrap();
    let solve = Solver::default().solve(&s.data, &params).unwrap();
    let router_only = outline_checks_with(&s.data, &solve, &solve.params, Some(2.0));
    assert_eq!(
        router_only.min_slot,
        CheckState::NotChecked(NotChecked::NoMinSlot)
    );
}

#[test]
fn a_plain_cutout_is_no_slot_and_is_not_checked() {
    let mut s = Sketch::new();
    let edges = s.slot_edges(10.0, 10.0, 0.3);
    s.get(edges[0]).board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });
    assert_eq!(s.checks(Some(0.8)).min_slot, CheckState::Passed);
}

#[test]
fn a_narrow_slot_warns_only_and_still_bakes() {
    let mut s = Sketch::new();
    let seed = s.slot(10.0, 10.0, 0.6);
    assert!(matches!(
        s.checks(Some(0.8)).min_slot,
        CheckState::Failed(_)
    ));
    let cutouts = bake_board(&s.data, &[]).unwrap().cutouts;
    assert_eq!(cutouts.len(), 1);
    assert_eq!(cutouts[0].uuid, seed.0);
}
