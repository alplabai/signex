//! mockup row 187 / mockup row 191 - the sketch canvas's values and pattern instances:
//! one suppression helper on the array type, used by the canvas instance
//! click and the Properties pattern form, round-trips a grid and a polar
//! array and leaves a linear one alone; the canvas's instance slots sit
//! where the bake puts the pads; a pattern value written on the drawing
//! lands in the right expression; the conflict badge counts.

use std::collections::HashMap;

use signex_app::library::editor::footprint::PatternField;
use signex_app::library::editor::footprint::SketchCanvasText;
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::Solver;

fn grid() -> ArrayKind {
    ArrayKind::Grid {
        source: SketchEntityId::new(),
        nx_expr: "3".into(),
        ny_expr: "2".into(),
        dx_expr: "1mm".into(),
        dy_expr: "1mm".into(),
        depopulation: None,
    }
}

fn polar() -> ArrayKind {
    ArrayKind::Polar {
        source: SketchEntityId::new(),
        center: SketchEntityId::new(),
        count_expr: "8".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    }
}

fn linear() -> ArrayKind {
    ArrayKind::Linear {
        source: SketchEntityId::new(),
        count_expr: "4".into(),
        dx_expr: "0.65mm".into(),
        dy_expr: "0mm".into(),
    }
}

#[test]
fn suppressing_and_restoring_an_instance_round_trips_a_grid_and_a_polar_array() {
    for start in [grid(), polar()] {
        let mut kind = start.clone();
        assert!(kind.has_suppression_model());

        assert!(kind.toggle_instance_suppressed(2, 0), "suppress");
        assert!(kind.is_instance_suppressed(2, 0));
        assert!(
            !kind.set_instance_suppressed(2, 0, true),
            "already suppressed"
        );

        assert!(kind.toggle_instance_suppressed(2, 0), "restore");
        assert!(!kind.is_instance_suppressed(2, 0));
        assert_eq!(kind, start, "a blank mask and an empty list leave no trace");
    }
}

#[test]
fn restoring_keeps_a_mask_the_user_wrote() {
    let mut kind = grid();
    if let ArrayKind::Grid { depopulation, .. } = &mut kind {
        *depopulation = Some(GridDepopulation {
            mask_expr: "i != 1".into(),
            suppressed_instances: Vec::new(),
        });
    }
    kind.set_instance_suppressed(0, 1, true);
    kind.set_instance_suppressed(0, 1, false);
    let ArrayKind::Grid {
        depopulation: Some(d),
        ..
    } = &kind
    else {
        panic!("the mask is kept: {kind:?}");
    };
    assert_eq!(d.mask_expr, "i != 1");
    assert!(d.suppressed_instances.is_empty());
}

#[test]
fn a_linear_pattern_has_no_suppression_and_never_changes() {
    let mut kind = linear();
    let before = kind.clone();
    assert!(!kind.has_suppression_model());
    assert!(!kind.toggle_instance_suppressed(1, 0));
    assert!(!kind.set_instance_suppressed(1, 0, true));
    assert!(!kind.is_instance_suppressed(1, 0));
    assert_eq!(kind, before);
}

/// A pad source at (10, 20) and a 3 × 2 grid of it whose x pitch grows
/// with the column (`i`), instance (2, 0) suppressed and column 1 masked.
fn baked_grid() -> (SketchData, ArrayId) {
    let plane = PlaneId::new();
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: 10.0, y: 20.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.3mm".into(),
        size_y_expr: "0.3mm".into(),
        ..PadAttr::default()
    });
    let array = ArrayId::new();
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![point],
        arrays: vec![Array {
            id: array,
            kind: ArrayKind::Grid {
                source,
                nx_expr: "4".into(),
                ny_expr: "2".into(),
                dx_expr: "1mm + i * 0.5mm".into(),
                dy_expr: "0.5mm".into(),
                depopulation: Some(GridDepopulation {
                    mask_expr: "i != 1".into(),
                    suppressed_instances: vec![(2, 0)],
                }),
            },
            numbering: NumberingScheme::default(),
        }],
        ..SketchData::default()
    };
    (sketch, array)
}

#[test]
fn instance_slots_sit_where_the_bake_expands_them() {
    let (sketch, array) = baked_grid();
    let solved = Solver::default()
        .solve(&sketch, &HashMap::new())
        .expect("the grid solves");
    let (mut pads, mut warnings) = (Vec::new(), Vec::new());
    signex_bake::bake_pads(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();
    signex_bake::bake_arrays(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();

    let slots = signex_bake::pattern_slots(&sketch, &solved, array, (10.0, 20.0), 100)
        .expect("the grid lays out");
    assert_eq!((slots[0].i, slots[0].j), (0, 0), "the source's slot first");
    assert!(slots.iter().all(|s| s.i != 1), "a masked slot has no ring");
    let shown: Vec<_> = slots.iter().filter(|s| !s.suppressed).collect();
    assert_eq!(
        shown.len(),
        pads.len(),
        "{slots:?} vs {pads:?}, {warnings:?}"
    );
    for slot in &shown {
        assert!(
            pads.iter().any(|p| (p.position[0] - slot.at.0).abs() < 1e-9
                && (p.position[1] - slot.at.1).abs() < 1e-9),
            "no pad under slot {slot:?}: {pads:?}"
        );
    }
    let held = slots
        .iter()
        .find(|s| (s.i, s.j) == (2, 0))
        .expect("the suppressed slot keeps its ring");
    assert!(held.suppressed);
    assert!(
        (held.at.0 - 14.0).abs() < 1e-9,
        "the per-instance pitch: 2 × (1 + 2 × 0.5) mm, {held:?}"
    );
}

#[test]
fn a_pattern_value_written_on_the_drawing_lands_in_its_expression() {
    let mut kind = linear();
    assert_eq!(PatternField::Count.expr(&kind), Some("4"));
    assert!(PatternField::Count.set_expr(&mut kind, "rows"));
    assert!(
        !PatternField::Count.set_expr(&mut kind, "rows"),
        "no change"
    );
    assert!(PatternField::SpacingX.set_expr(&mut kind, "pitch"));
    assert!(matches!(
        &kind,
        ArrayKind::Linear { count_expr, dx_expr, .. } if count_expr == "rows" && dx_expr == "pitch"
    ));
    assert!(
        !PatternField::Sweep.set_expr(&mut kind, "90deg"),
        "a linear pattern has no sweep"
    );
    assert_eq!(
        PatternField::of(&polar()),
        [PatternField::Count, PatternField::Sweep]
    );
}

#[test]
fn the_conflict_badge_counts_its_conflicts() {
    // The count reaches the catalog as a number, so it picks the plural form.
    let i18n = signex_i18n::I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()));
    let mut text = SketchCanvasText::default();
    assert!(text.set_conflict_count(&i18n, 1));
    assert_eq!(text.conflict_badge, "1 conflict · see Properties");
    assert!(text.set_conflict_count(&i18n, 4));
    assert_eq!(text.conflict_badge, "4 conflicts · see Properties");
    assert!(
        !text.set_conflict_count(&i18n, 4),
        "the same count renders once"
    );
    assert!(text.set_conflict_count(&i18n, 0));
    assert!(text.conflict_badge.is_empty(), "no conflict, no badge");
}
