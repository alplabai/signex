//! The analytic Jacobian against central differences of `total_residual`,
//! for every constraint kind (and the implicit arc closure rows) at several
//! pseudo-random configurations.

use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::{Entity, EntityKind};
use crate::id::{ConstraintId, SketchEntityId};
use crate::plane::PlaneId;
use crate::sketch::SketchData;

use super::jacobian::jacobian;
use super::residual::{ResolvedParams, total_residual};
use super::state::pack;

/// Central-difference step. The truncation error is O(h^2) times a
/// third derivative of order 1, the rounding error ~eps/h: both near 1e-10.
const H: f64 = 1e-6;
/// Allowed |analytic - numeric|, relative to 1 + |value|.
const TOL: f64 = 1e-6;
const CONFIGS: usize = 6;

/// xorshift64*: a fixed seed keeps the configurations reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self, lo: f64, hi: f64) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let u = (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * u
    }
}

/// Fixture ids: two lines, two arcs, two circles and three loose points.
struct Ids {
    la: SketchEntityId,
    lb: SketchEntityId,
    a1: SketchEntityId,
    a2: SketchEntityId,
    c1: SketchEntityId,
    c2: SketchEntityId,
    q: [SketchEntityId; 3],
}

fn fixture(rng: &mut Rng) -> (SketchData, Ids) {
    let plane = PlaneId::new();
    let mut entities = Vec::new();
    let mut point = |rng: &mut Rng| {
        let id = SketchEntityId::new();
        let kind = EntityKind::Point {
            x: rng.next(-5.0, 5.0),
            y: rng.next(-5.0, 5.0),
        };
        entities.push(Entity::new(id, plane, kind));
        id
    };
    let p: Vec<_> = (0..15).map(|_| point(rng)).collect();
    let ids = Ids {
        la: SketchEntityId::new(),
        lb: SketchEntityId::new(),
        a1: SketchEntityId::new(),
        a2: SketchEntityId::new(),
        c1: SketchEntityId::new(),
        c2: SketchEntityId::new(),
        q: [p[12], p[13], p[14]],
    };
    let line = |id, start, end| Entity::new(id, plane, EntityKind::Line { start, end });
    let arc = |id, c, s, e| {
        Entity::new(
            id,
            plane,
            EntityKind::Arc {
                center: c,
                start: s,
                end: e,
                sweep_ccw: true,
            },
        )
    };
    let circle = |id, center, radius| Entity::new(id, plane, EntityKind::Circle { center, radius });
    entities.extend([
        line(ids.la, p[0], p[1]),
        line(ids.lb, p[2], p[3]),
        arc(ids.a1, p[4], p[5], p[6]),
        arc(ids.a2, p[7], p[8], p[9]),
        circle(ids.c1, p[10], rng.next(0.5, 3.0)),
        circle(ids.c2, p[11], rng.next(0.5, 3.0)),
    ]);
    let data = SketchData {
        entities,
        ..SketchData::default()
    };
    (data, ids)
}

fn kinds(ids: &Ids) -> Vec<ConstraintKind> {
    use ConstraintKind::*;
    let t = || DimTarget::Literal(1.7);
    let Ids {
        la,
        lb,
        a1,
        a2,
        c1,
        c2,
        q,
    } = *ids;
    vec![
        Coincident { p1: q[0], p2: q[1] },
        PointOnLine {
            point: q[0],
            line: la,
        },
        PointOnArc {
            point: q[0],
            arc: a1,
        },
        Horizontal { line: la },
        Vertical { line: lb },
        Parallel { l1: la, l2: lb },
        Perpendicular { l1: la, l2: lb },
        DistancePtPt {
            p1: q[0],
            p2: q[1],
            target: t(),
        },
        DistancePtLine {
            point: q[0],
            line: lb,
            target: t(),
        },
        DistancePtCircle {
            point: q[0],
            circle: c1,
            target: t(),
        },
        DistancePtCircle {
            point: q[0],
            circle: a1,
            target: t(),
        },
        Angle {
            l1: la,
            l2: lb,
            target: t(),
        },
        EqualLength { l1: la, l2: lb },
        EqualRadius { e1: c1, e2: c2 },
        EqualRadius { e1: c1, e2: a1 },
        EqualRadius { e1: a1, e2: a2 },
        TangentLineArc { line: la, arc: a1 },
        TangentLineArc { line: lb, arc: c1 },
        TangentArcArc {
            a1,
            a2,
            internal: false,
        },
        TangentArcArc {
            a1,
            a2,
            internal: true,
        },
        TangentArcArc {
            a1: c1,
            a2: c2,
            internal: false,
        },
        TangentArcArc {
            a1: c1,
            a2: a2,
            internal: true,
        },
        SymmetricAboutLine {
            p1: q[0],
            p2: q[1],
            line: la,
        },
        SymmetricAboutPoint {
            p1: q[0],
            p2: q[1],
            center: q[2],
        },
        // The same point in two roles: columns must add up.
        SymmetricAboutPoint {
            p1: q[0],
            p2: q[1],
            center: q[0],
        },
        Midpoint {
            point: q[0],
            line: lb,
        },
        Fixed { point: q[1] },
    ]
}

/// Max |analytic - central difference| over the Jacobian of `sketch`.
fn worst_error(sketch: &SketchData) -> f64 {
    let packed = pack(sketch);
    let params = ResolvedParams::new();
    let analytic = jacobian(sketch, &packed.vector, &packed.index, &params).expect("jacobian");
    let mut worst = 0.0_f64;
    let mut state = packed.vector.clone();
    for col in 0..state.len() {
        let saved = state[col];
        state[col] = saved + H;
        let plus = total_residual(sketch, &state, &packed.index, &params).expect("r+");
        state[col] = saved - H;
        let minus = total_residual(sketch, &state, &packed.index, &params).expect("r-");
        state[col] = saved;
        assert_eq!(analytic.len(), plus.len(), "row count");
        for (row, (p, m)) in plus.iter().zip(&minus).enumerate() {
            let numeric = (p - m) / (2.0 * H);
            let err = (analytic[row][col] - numeric).abs() / (1.0 + numeric.abs());
            worst = worst.max(err);
        }
    }
    worst
}

fn with_constraints(mut sketch: SketchData, kinds: Vec<ConstraintKind>) -> SketchData {
    sketch.constraints = kinds
        .into_iter()
        .map(|kind| Constraint {
            id: ConstraintId::new(),
            kind,
        })
        .collect();
    sketch
}

#[test]
fn every_constraint_kind_matches_central_differences() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for config in 0..CONFIGS {
        let (base, ids) = fixture(&mut rng);
        for kind in kinds(&ids) {
            let label = format!("{kind:?}");
            // Fixed alone leaves nothing to differentiate; pair it with a
            // distance so the fixed point's missing column is exercised.
            let mut set = vec![kind];
            if matches!(set[0], ConstraintKind::Fixed { .. }) {
                set.push(ConstraintKind::DistancePtPt {
                    p1: ids.q[1],
                    p2: ids.q[2],
                    target: DimTarget::Literal(1.0),
                });
            }
            let err = worst_error(&with_constraints(base.clone(), set));
            assert!(err < TOL, "config {config}, {label}: error {err:e}");
        }
    }
}

#[test]
fn a_sketch_with_every_kind_matches_central_differences() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for config in 0..CONFIGS {
        let (base, ids) = fixture(&mut rng);
        let err = worst_error(&with_constraints(base, kinds(&ids)));
        assert!(err < TOL, "config {config}: error {err:e}");
    }
}
