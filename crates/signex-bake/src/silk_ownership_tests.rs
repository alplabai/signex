//! #1718 — a silk re-bake keeps hand-drawn silk; legacy files are
//! classified once on open.

use super::*;
use signex_sketch::SketchData;
use signex_sketch::attr::SilkAttr;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::ConstraintId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::layer::SignexLayer;

fn line(from: [f64; 2], to: [f64; 2], stroke_width: f64) -> FpGraphic {
    FpGraphic::drawn(FpGraphicKind::Line { from, to }, stroke_width, false)
}

fn text(content: &str) -> FpGraphic {
    FpGraphic::drawn(
        FpGraphicKind::Text {
            position: [0.0, 2.0],
            content: content.into(),
            size: 1.0,
            frame: None,
        },
        0.0,
        false,
    )
}

fn marked(mut g: FpGraphic) -> FpGraphic {
    g.sketch_entity = Some(SketchEntityId::new());
    g
}

/// A footprint whose sketch has one top-silk line from (0, 0) to (5, 0).
/// Returns the footprint and the line's two point ids.
fn silk_line_footprint() -> (Footprint, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
    let mut silk = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line { start: p1, end: p2 },
    );
    silk.silk = Some(SilkAttr {
        layer: SignexLayer::TopSilk,
    });
    let mut fp = Footprint::empty("legacy");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }),
            silk,
        ],
        ..SketchData::default()
    });
    (fp, p1, p2)
}

#[test]
fn a_merge_replaces_marked_entries_and_keeps_hand_entries_in_order() {
    let hand_a = text("A");
    let hand_b = line([9.0, 9.0], [10.0, 9.0], 0.15);
    let stale = marked(line([0.0, 0.0], [1.0, 0.0], 0.12));
    let mut layer = vec![stale.clone(), hand_a.clone(), stale, hand_b.clone()];
    let fresh = marked(line([0.0, 0.0], [2.0, 0.0], 0.12));

    let remap = merge_silk_layer(&mut layer, vec![fresh.clone()]);

    assert_eq!(layer, vec![fresh, hand_a, hand_b]);
    assert_eq!(remap, vec![None, Some(1), None, Some(2)]);
}

#[test]
fn an_empty_merge_removes_only_marked_entries() {
    let hand = text("REF");
    let mut layer = vec![marked(line([0.0, 0.0], [1.0, 0.0], 0.12)), hand.clone()];
    let remap = merge_silk_layer(&mut layer, Vec::new());
    assert_eq!(layer, vec![hand]);
    assert_eq!(remap, vec![None, Some(0)]);
}

#[test]
fn a_marked_entry_is_remapped_to_the_rebake_of_its_entity() {
    let old = marked(line([0.0, 0.0], [1.0, 0.0], 0.12));
    let mut moved = line([0.0, 0.0], [3.0, 0.0], 0.12);
    moved.sketch_entity = old.sketch_entity;
    let mut layer = vec![text("A"), old];
    let remap = merge_silk_layer(
        &mut layer,
        vec![marked(line([5.0; 2], [6.0; 2], 0.12)), moved],
    );
    assert_eq!(remap, vec![Some(2), Some(1)]);
}

#[test]
fn settling_drops_a_legacy_silk_token() {
    let (mut fp, _, _) = silk_line_footprint();
    let sketch = fp.sketch.as_mut().unwrap();
    sketch.record_baked(BakedKind::SilkF, true);
    sketch.record_baked(BakedKind::Courtyard, true);
    settle_baked_silk(&mut fp, Vec::new(), Vec::new());
    let sketch = fp.sketch.as_ref().unwrap();
    assert!(!sketch.was_baked(BakedKind::SilkF));
    assert!(sketch.was_baked(BakedKind::Courtyard));
}

#[test]
fn legacy_recovery_marks_the_baked_line_and_keeps_hand_entries() {
    let (mut fp, _, _) = silk_line_footprint();
    // As an older build saved it: the bake output, unmarked, plus a hand
    // text and a hand line with the bake's own stroke width elsewhere.
    fp.silk_f = vec![
        text("REF"),
        line([0.0, 0.0], [5.0, 0.0], 0.12),
        line([0.0, 3.0], [5.0, 3.0], 0.12),
    ];
    let outcome = recover_legacy_silk(&mut fp, &Solver::default());
    assert_eq!(outcome, LegacySilk::Marked(1));
    let flags: Vec<bool> = fp.silk_f.iter().map(FpGraphic::is_baked).collect();
    assert_eq!(flags, vec![false, true, false]);
}

#[test]
fn legacy_recovery_uses_each_bake_output_once() {
    let (mut fp, _, _) = silk_line_footprint();
    // A hand copy drawn exactly over the baked line: one of the two is the
    // bake's, the other stays hand-drawn.
    fp.silk_f = vec![
        line([0.0, 0.0], [5.0, 0.0], 0.12),
        line([0.0, 0.0], [5.0, 0.0], 0.12),
    ];
    assert_eq!(
        recover_legacy_silk(&mut fp, &Solver::default()),
        LegacySilk::Marked(1)
    );
    assert_eq!(fp.silk_f.iter().filter(|g| g.is_baked()).count(), 1);
}

#[test]
fn legacy_recovery_needs_the_same_stroke_and_fill() {
    let (mut fp, _, _) = silk_line_footprint();
    let mut filled = line([0.0, 0.0], [5.0, 0.0], 0.12);
    filled.filled = true;
    fp.silk_f = vec![line([0.0, 0.0], [5.0, 0.0], 0.15), filled];
    assert_eq!(
        recover_legacy_silk(&mut fp, &Solver::default()),
        LegacySilk::Marked(0)
    );
    assert!(fp.silk_f.iter().all(|g| !g.is_baked()));
}

#[test]
fn legacy_recovery_on_an_unsolvable_sketch_marks_and_removes_nothing() {
    let (mut fp, p1, p2) = silk_line_footprint();
    let conflict = |mm: f64| Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        },
    };
    let sketch = fp.sketch.as_mut().unwrap();
    sketch.constraints = vec![conflict(1.0), conflict(2.0)];
    fp.silk_f = vec![line([0.0, 0.0], [5.0, 0.0], 0.12), text("REF")];
    let before = fp.silk_f.clone();

    let outcome = recover_legacy_silk(&mut fp, &Solver::default());

    assert!(matches!(outcome, LegacySilk::Unsolved(_)), "{outcome:?}");
    assert_eq!(fp.silk_f, before);
}

#[test]
fn a_marked_layer_needs_no_recovery() {
    let (mut fp, _, _) = silk_line_footprint();
    fp.silk_f = vec![marked(line([0.0, 0.0], [5.0, 0.0], 0.12))];
    assert!(!needs_legacy_silk_recovery(&fp));
    assert_eq!(
        recover_legacy_silk(&mut fp, &Solver::default()),
        LegacySilk::NotNeeded
    );
}
