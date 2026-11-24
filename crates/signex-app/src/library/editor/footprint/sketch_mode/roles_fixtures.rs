//! A solved sketch with two closed profiles for the Bake as and Slot header
//! tests (mockup row 179): a 4 x 2 rectangle and a 12 x 3 slot.

use signex_sketch::Solver;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

/// The sketch, its solve, the rectangle's four lines and the slot's four
/// edges in walk order (line, arc, line, arc).
pub(crate) struct RectAndSlot {
    pub sketch: SketchData,
    pub solve: FullSolveOutput,
    pub rect: Vec<SketchEntityId>,
    pub slot: Vec<SketchEntityId>,
}

pub(crate) fn rect_and_slot() -> RectAndSlot {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let corners = [(-10.0, 0.0), (-6.0, 0.0), (-6.0, 2.0), (-10.0, 2.0)];
    let c: Vec<_> = corners
        .iter()
        .map(|&(x, y)| add(EntityKind::Point { x, y }))
        .collect();
    let rect = (0..4)
        .map(|i| {
            add(EntityKind::Line {
                start: c[i],
                end: c[(i + 1) % 4],
            })
        })
        .collect();
    let p0 = add(EntityKind::Point { x: 10.0, y: 0.0 });
    let p1 = add(EntityKind::Point { x: 19.0, y: 0.0 });
    let p2 = add(EntityKind::Point { x: 19.0, y: 3.0 });
    let p3 = add(EntityKind::Point { x: 10.0, y: 3.0 });
    let right = add(EntityKind::Point { x: 19.0, y: 1.5 });
    let left = add(EntityKind::Point { x: 10.0, y: 1.5 });
    let slot = vec![
        add(EntityKind::Line { start: p0, end: p1 }),
        add(EntityKind::Arc {
            center: right,
            start: p1,
            end: p2,
            sweep_ccw: true,
        }),
        add(EntityKind::Line { start: p2, end: p3 }),
        add(EntityKind::Arc {
            center: left,
            start: p3,
            end: p0,
            sweep_ccw: true,
        }),
    ];
    let params = resolve(&sketch.parameters).expect("no parameters");
    let solve = Solver::default()
        .solve(&sketch, &params)
        .expect("an unconstrained sketch solves");
    RectAndSlot {
        sketch,
        solve,
        rect,
        slot,
    }
}
