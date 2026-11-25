//! The canvas's instance slots come from the bake's walk: same steps,
//! sweep, suppression list and mask.

use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::{FullSolveOutput, Solver};

use super::{PatternSlot, pattern_slots};

/// A source point at (10, 20) and a centre at the origin, with `kind`
/// built from them as the sketch's one array.
fn sketch_with(
    kind: impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind,
) -> (SketchData, ArrayId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let (source, center) = (SketchEntityId::new(), SketchEntityId::new());
    for (id, x, y) in [(source, 10.0, 20.0), (center, 0.0, 0.0)] {
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
    }
    let id = ArrayId::new();
    sketch.arrays.push(Array {
        id,
        kind: kind(source, center),
        numbering: NumberingScheme::default(),
    });
    (sketch, id)
}

fn solve(sketch: &SketchData) -> FullSolveOutput {
    Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("the fixture solves")
}

fn grid(depopulation: Option<GridDepopulation>) -> (SketchData, ArrayId) {
    sketch_with(|source, _| ArrayKind::Grid {
        source,
        nx_expr: "3".into(),
        ny_expr: "2".into(),
        dx_expr: "1mm".into(),
        dy_expr: "0.5mm".into(),
        depopulation: depopulation.clone(),
    })
}

fn slot(slots: &[PatternSlot], i: u32, j: u32) -> PatternSlot {
    *slots
        .iter()
        .find(|s| (s.i, s.j) == (i, j))
        .unwrap_or_else(|| panic!("no slot ({i}, {j}): {slots:?}"))
}

#[test]
fn grid_slots_sit_where_the_bake_expands_them() {
    let (sketch, id) = grid(None);
    let slots = pattern_slots(&sketch, &solve(&sketch), id, (10.0, 20.0), 100).expect("slots");
    assert_eq!(slots.len(), 6);
    assert_eq!((slots[0].i, slots[0].j), (0, 0), "the source's slot first");
    let far = slot(&slots, 2, 1);
    assert!(
        (far.at.0 - 12.0).abs() < 1e-9 && (far.at.1 - 20.5).abs() < 1e-9,
        "{far:?}"
    );
    assert!(slots.iter().all(|s| !s.suppressed));
}

#[test]
fn a_listed_slot_is_kept_suppressed_and_a_masked_slot_is_left_out() {
    let (sketch, id) = grid(Some(GridDepopulation {
        mask_expr: "i != 1".into(),
        suppressed_instances: vec![(2, 0)],
    }));
    let slots = pattern_slots(&sketch, &solve(&sketch), id, (10.0, 20.0), 100).expect("slots");
    assert!(slot(&slots, 2, 0).suppressed, "the list holds it back");
    assert!(!slot(&slots, 2, 1).suppressed);
    assert!(
        slots.iter().all(|s| s.i != 1),
        "the mask drops column 1: {slots:?}"
    );
    assert_eq!(slots.len(), 4);
}

#[test]
fn a_polar_slot_turns_by_its_share_of_the_sweep_in_degrees() {
    let (sketch, id) = sketch_with(|source, center| ArrayKind::Polar {
        source,
        center,
        count_expr: "4".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    });
    let slots = pattern_slots(&sketch, &solve(&sketch), id, (10.0, 20.0), 100).expect("slots");
    let quarter = slot(&slots, 1, 0);
    assert!(
        (quarter.at.0 + 20.0).abs() < 1e-9 && (quarter.at.1 - 10.0).abs() < 1e-9,
        "a quarter turn: {quarter:?}"
    );
}

#[test]
fn more_slots_than_the_limit_lay_out_nothing() {
    let (sketch, id) = grid(None);
    assert_eq!(
        pattern_slots(&sketch, &solve(&sketch), id, (10.0, 20.0), 5),
        None
    );
}
