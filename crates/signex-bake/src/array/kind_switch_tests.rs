//! #2596: a pattern switches between Linear, Grid and Polar with its count
//! (DECISIONS 2026-11-27, decision 1), and keeps the suppressed instances
//! and pad overrides whose index still has a place (decision 3).

use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::{InstanceOverride, PadAttrOverride, SketchData};

use super::{KindSwitchRefused, KindSwitched, KindTarget, PatternKind, switch_pattern_kind};

const SOURCE: SketchEntityId = SketchEntityId(uuid::Uuid::from_u128(0x5a));
const CENTRE: SketchEntityId = SketchEntityId(uuid::Uuid::from_u128(0xce));

fn sketch_with(kind: ArrayKind) -> (SketchData, ArrayId) {
    let array = Array {
        id: ArrayId::new(),
        kind,
        numbering: NumberingScheme::default(),
    };
    let id = array.id;
    let sketch = SketchData {
        arrays: vec![array],
        ..SketchData::default()
    };
    (sketch, id)
}

fn linear(count: &str, dx: &str, dy: &str, suppressed: Vec<(u32, u32)>) -> ArrayKind {
    ArrayKind::Linear {
        source: SOURCE,
        count_expr: count.into(),
        dx_expr: dx.into(),
        dy_expr: dy.into(),
        depopulation: depopulation("", suppressed),
    }
}

fn grid(nx: &str, ny: &str, suppressed: Vec<(u32, u32)>) -> ArrayKind {
    ArrayKind::Grid {
        source: SOURCE,
        nx_expr: nx.into(),
        ny_expr: ny.into(),
        dx_expr: "1.27mm".into(),
        dy_expr: "2mm".into(),
        depopulation: depopulation("", suppressed),
    }
}

fn polar(count: &str, mask: &str, suppressed: Vec<(u32, u32)>) -> ArrayKind {
    ArrayKind::Polar {
        source: SOURCE,
        center: CENTRE,
        count_expr: count.into(),
        sweep_angle_expr: "90deg".into(),
        depopulation: depopulation(mask, suppressed),
    }
}

fn depopulation(mask: &str, suppressed: Vec<(u32, u32)>) -> Option<GridDepopulation> {
    (!mask.is_empty() || !suppressed.is_empty()).then(|| GridDepopulation {
        mask_expr: mask.into(),
        suppressed_instances: suppressed,
    })
}

fn kind(sketch: &SketchData, id: ArrayId) -> &ArrayKind {
    &sketch.arrays.iter().find(|a| a.id == id).unwrap().kind
}

fn switched(start: ArrayKind, target: KindTarget) -> (ArrayKind, KindSwitched) {
    let (mut sketch, id) = sketch_with(start);
    let dropped = switch_pattern_kind(&mut sketch, id, target).expect("a switch");
    (kind(&sketch, id).clone(), dropped)
}

const POLAR: KindTarget = KindTarget::Polar { centre: CENTRE };

#[test]
fn linear_to_grid_is_one_row_of_the_count() {
    let (got, dropped) = switched(linear("8", "2.54mm", "0mm", vec![(3, 0)]), KindTarget::Grid);
    assert_eq!(
        got,
        ArrayKind::Grid {
            source: SOURCE,
            nx_expr: "8".into(),
            ny_expr: "1".into(),
            dx_expr: "2.54mm".into(),
            dy_expr: "0mm".into(),
            depopulation: depopulation("", vec![(3, 0)]),
        }
    );
    assert_eq!(dropped, KindSwitched::default());
}

/// A linear pattern that steps along Y only becomes a column, so its
/// instances do not stack on a zero column pitch.
#[test]
fn a_vertical_linear_becomes_a_grid_column() {
    let (got, _) = switched(linear("4", "0mm", "1mm", vec![(2, 0)]), KindTarget::Grid);
    let ArrayKind::Grid {
        nx_expr,
        ny_expr,
        depopulation: d,
        ..
    } = got
    else {
        panic!("a grid");
    };
    assert_eq!((nx_expr.as_str(), ny_expr.as_str()), ("1", "4"));
    assert_eq!(d.unwrap().suppressed_instances, vec![(0, 2)]);
}

#[test]
fn linear_to_polar_keeps_the_count_over_a_full_turn() {
    let (got, _) = switched(linear("6", "1mm", "0mm", vec![(5, 0)]), POLAR);
    assert_eq!(
        got,
        ArrayKind::Polar {
            source: SOURCE,
            center: CENTRE,
            count_expr: "6".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: depopulation("", vec![(5, 0)]),
        }
    );
}

/// A 2 × 3 grid walks row by row: cell (1, 2) is the sixth instance.
#[test]
fn grid_to_linear_keeps_the_instance_count_in_walk_order() {
    let (got, dropped) = switched(grid("2", "3", vec![(1, 0), (1, 2)]), KindTarget::Linear);
    assert_eq!(
        got,
        ArrayKind::Linear {
            source: SOURCE,
            count_expr: "6".into(),
            dx_expr: "1.27mm".into(),
            dy_expr: "0mm".into(),
            depopulation: depopulation("", vec![(1, 0), (5, 0)]),
        }
    );
    assert_eq!(dropped, KindSwitched::default());
}

#[test]
fn a_one_row_grid_keeps_its_count_expression() {
    let (got, _) = switched(grid("pins", "1", vec![]), KindTarget::Linear);
    let ArrayKind::Linear { count_expr, .. } = got else {
        panic!("a linear pattern");
    };
    assert_eq!(count_expr, "pins");
}

/// A grid one column wide walks down the column.
#[test]
fn a_one_column_grid_steps_along_its_rows() {
    let (got, _) = switched(grid("1", "4", vec![(0, 3)]), KindTarget::Linear);
    let ArrayKind::Linear {
        count_expr,
        dx_expr,
        dy_expr,
        depopulation: d,
        ..
    } = got
    else {
        panic!("a linear pattern");
    };
    assert_eq!(
        (count_expr.as_str(), dx_expr.as_str(), dy_expr.as_str()),
        ("4", "0mm", "2mm")
    );
    assert_eq!(d.unwrap().suppressed_instances, vec![(3, 0)]);
}

#[test]
fn grid_to_polar_keeps_the_instance_count_over_a_full_turn() {
    let (got, _) = switched(grid("2", "2", vec![(0, 1)]), POLAR);
    assert_eq!(
        got,
        ArrayKind::Polar {
            source: SOURCE,
            center: CENTRE,
            count_expr: "4".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: depopulation("", vec![(2, 0)]),
        }
    );
}

/// Polar has no spacing: the switch gives the pitch of a new Rectangular
/// Pattern along X, and keeps the mask expression.
#[test]
fn polar_to_linear_and_grid_keep_the_count() {
    let (got, _) = switched(polar("5", "i != 1", vec![(4, 0)]), KindTarget::Linear);
    assert_eq!(
        got,
        ArrayKind::Linear {
            source: SOURCE,
            count_expr: "5".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
            depopulation: depopulation("i != 1", vec![(4, 0)]),
        }
    );
    let (got, _) = switched(polar("5", "", vec![]), KindTarget::Grid);
    assert_eq!(
        got,
        ArrayKind::Grid {
            source: SOURCE,
            nx_expr: "5".into(),
            ny_expr: "1".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        }
    );
}

/// A grid whose size does not resolve keeps its count as a product; the
/// entries of its first row still map, the others are dropped.
#[test]
fn an_unresolved_grid_keeps_its_first_row_only() {
    let (got, dropped) = switched(
        grid("cols", "rows", vec![(1, 0), (0, 1)]),
        KindTarget::Linear,
    );
    let ArrayKind::Linear {
        count_expr,
        depopulation: d,
        ..
    } = got
    else {
        panic!("a linear pattern");
    };
    assert_eq!(count_expr, "(cols) * (rows)");
    assert_eq!(d.unwrap().suppressed_instances, vec![(1, 0)]);
    assert_eq!(dropped.suppressed_dropped, vec![(0, 1)]);
}

/// Decision 3: an override moves with its instance; one past the last row
/// (a mirrored instance) or past the count has no place and is dropped.
#[test]
fn overrides_follow_their_instance_and_the_rest_are_dropped() {
    let (mut sketch, id) = sketch_with(grid("2", "2", vec![(0, 2)]));
    let other = ArrayId::new();
    let net = |name: &str| PadAttrOverride {
        net: Some(name.into()),
        ..PadAttrOverride::default()
    };
    for (array, i, j, name) in [
        (id, 1, 1, "A"),
        (id, 0, 2, "B"),
        (id, 5, 0, "C"),
        (other, 1, 1, "D"),
    ] {
        sketch.pad_overrides.push(InstanceOverride {
            array,
            i,
            j,
            pad: net(name),
        });
    }
    let dropped = switch_pattern_kind(&mut sketch, id, POLAR).unwrap();
    assert_eq!(dropped.suppressed_dropped, vec![(0, 2)]);
    assert_eq!(dropped.overrides_dropped, vec![(0, 2), (5, 0)]);
    assert_eq!(sketch.override_for(id, 3, 0), Some(&net("A")));
    assert_eq!(
        sketch.override_for(other, 1, 1),
        Some(&net("D")),
        "another pattern's stays"
    );
    assert_eq!(sketch.pad_overrides.len(), 2);
    assert!(
        kind(&sketch, id).depopulation().is_none(),
        "an emptied list with no mask leaves no depopulation"
    );
}

/// Switching to a kind and back gives the pattern it started as.
#[test]
fn linear_to_grid_and_back_is_the_same_pattern() {
    let start = linear("8", "2.54mm", "0mm", vec![(3, 0)]);
    let (mut sketch, id) = sketch_with(start.clone());
    switch_pattern_kind(&mut sketch, id, KindTarget::Grid).unwrap();
    switch_pattern_kind(&mut sketch, id, KindTarget::Linear).unwrap();
    assert_eq!(kind(&sketch, id), &start);
}

#[test]
fn the_same_kind_or_a_missing_pattern_changes_nothing() {
    let (mut sketch, id) = sketch_with(grid("2", "2", vec![]));
    let before = sketch.clone();
    assert_eq!(
        switch_pattern_kind(&mut sketch, id, KindTarget::Grid),
        Err(KindSwitchRefused::SameKind)
    );
    assert_eq!(
        switch_pattern_kind(&mut sketch, ArrayId::new(), KindTarget::Linear),
        Err(KindSwitchRefused::NoSuchPattern)
    );
    assert_eq!(sketch, before);
    assert_eq!(PatternKind::of(kind(&sketch, id)), PatternKind::Grid);
}

/// `remove_unused_point` takes a bare Point nothing names, and keeps one a
/// Line, a pattern or its own pad attribute uses.
#[test]
fn only_an_unused_bare_point_is_removed() {
    use signex_sketch::attr::PadAttr;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::plane::PlaneId;

    use super::remove_unused_point;

    let plane = PlaneId::new();
    let point = |id| Entity::new(id, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    let (free, on_line, padded, end) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    let (mut sketch, _) = sketch_with(polar("4", "", vec![]));
    let mut pad = point(padded);
    pad.pad = Some(PadAttr::default());
    sketch.entities.extend([
        point(free),
        point(on_line),
        pad,
        point(end),
        point(CENTRE),
        Entity::new(
            SketchEntityId::new(),
            plane,
            EntityKind::Line {
                start: on_line,
                end,
            },
        ),
    ]);
    assert!(!remove_unused_point(&mut sketch, on_line));
    assert!(!remove_unused_point(&mut sketch, padded));
    assert!(
        !remove_unused_point(&mut sketch, CENTRE),
        "the pattern's centre"
    );
    assert!(remove_unused_point(&mut sketch, free));
    assert!(!sketch.entities.iter().any(|e| e.id == free));
    assert!(!remove_unused_point(&mut sketch, free), "already gone");
}
