//! #187 / #191 (B5) - the sketch canvas's values and pattern instances:
//! one suppression helper on the array type, used by the canvas instance
//! click and the Properties pattern form, round-trips a grid and a polar
//! array and leaves a linear one alone; a pattern value written on the
//! drawing lands in the right expression; the conflict badge counts.

use signex_app::library::editor::footprint::state::canvas_text::{COUNT_SLOT, SketchCanvasText};
use signex_app::library::editor::footprint::state::sketch_overlay::PatternField;
use signex_sketch::array::{ArrayKind, ArrayLayout, GridDepopulation};
use signex_sketch::id::SketchEntityId;

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

#[test]
fn instance_slots_sit_where_the_bake_expands_them() {
    let grid = ArrayLayout::Grid {
        nx: 3,
        ny: 2,
        step: (1.0, 0.5),
    };
    assert_eq!(grid.slot_count(), 6);
    assert_eq!(grid.slots()[0], (0, 0), "the source's slot first");
    assert_eq!(grid.position((10.0, 20.0), 2, 1), (12.0, 20.5));

    let polar = ArrayLayout::Polar {
        count: 4,
        sweep_rad: std::f64::consts::TAU,
        center: (0.0, 0.0),
    };
    let (x, y) = polar.position((1.0, 0.0), 1, 0);
    assert!(
        x.abs() < 1e-9 && (y - 1.0).abs() < 1e-9,
        "a quarter turn: {x}, {y}"
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
    let text = SketchCanvasText {
        conflict_badge_one: "1 conflict · see Properties".into(),
        conflict_badge_many: format!("{COUNT_SLOT} conflicts · see Properties"),
        ..SketchCanvasText::default()
    };
    assert_eq!(text.conflict_badge(1), "1 conflict · see Properties");
    assert_eq!(text.conflict_badge(4), "4 conflicts · see Properties");
}
