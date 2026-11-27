//! The Properties pattern preview walks an array the way the bake does:
//! every kept instance has the number and position of the baked pad.

use std::collections::HashMap;

use signex_bake::array::{ArrayPreview, preview_array};
use signex_bake::bake_arrays;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::attr::PadAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

/// A sketch with one pad point at `(x, y)` and `kind` built on it.
fn sketch_with(
    at: (f64, f64),
    numbering: NumberingScheme,
    kind: impl FnOnce(SketchEntityId) -> ArrayKind,
) -> SketchData {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: at.0, y: at.1 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    data.entities.push(point);
    data.arrays.push(Array {
        id: ArrayId::new(),
        kind: kind(source),
        numbering,
    });
    data
}

/// The kept preview instances as `(number, x, y)`, positions from the
/// source at `at`, beside the baked pads.
fn preview_and_bake(data: &SketchData, at: (f64, f64)) -> (Vec<(String, f64, f64)>, usize) {
    let ArrayPreview::Instances(all) =
        preview_array(&data.arrays[0], data, Some([at.0, at.1]), None, 256)
    else {
        panic!("the array resolves");
    };
    let kept: Vec<_> = all
        .iter()
        .filter(|c| !c.is_skipped)
        .map(|c| {
            (
                c.number.clone(),
                at.0 + c.offset_mm[0],
                at.1 + c.offset_mm[1],
            )
        })
        .collect();
    let solve = Solver::default()
        .solve(data, &ResolvedParams::new())
        .expect("the fixture solves");
    let mut baked = Vec::new();
    let mut warnings = Vec::new();
    bake_arrays(data, &solve, &HashMap::new(), &mut baked, &mut warnings).expect("bake ok");
    assert_eq!(baked.len(), kept.len(), "{warnings:?}");
    for (pad, (number, x, y)) in baked.iter().zip(&kept) {
        assert_eq!(&pad.number, number);
        assert!((pad.position[0] - x).abs() < 1e-9 && (pad.position[1] - y).abs() < 1e-9);
    }
    (kept, all.len())
}

#[test]
fn a_depopulated_bga_grid_previews_the_baked_pads() {
    let at = (0.5, 0.25);
    let data = sketch_with(
        at,
        NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'H',
            start_col: 1,
        },
        |source| ArrayKind::Grid {
            source,
            nx_expr: "3".into(),
            ny_expr: "2".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0.8mm".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: "i != 1 || j != 0".into(),
                suppressed_instances: vec![(2, 1)],
            }),
        },
    );
    let (kept, all) = preview_and_bake(&data, at);
    assert_eq!(all, 6, "dropped cells are previewed too");
    let numbers: Vec<&str> = kept.iter().map(|(n, ..)| n.as_str()).collect();
    assert_eq!(numbers, vec!["H1", "H3", "J1", "J2"], "I is skipped");
}

#[test]
fn a_linear_pattern_previews_the_baked_pads() {
    let at = (0.0, 0.0);
    let data = sketch_with(
        at,
        NumberingScheme::LinearIncrement {
            start_expr: "10".into(),
            step_expr: "2".into(),
            sweep: Default::default(),
        },
        |source| ArrayKind::Linear {
            source,
            count_expr: "3".into(),
            dx_expr: "1.27mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
    );
    let (kept, _) = preview_and_bake(&data, at);
    let numbers: Vec<&str> = kept.iter().map(|(n, ..)| n.as_str()).collect();
    assert_eq!(numbers, vec!["10", "12", "14"]);
}

#[test]
fn a_count_that_does_not_evaluate_previews_nothing() {
    let data = sketch_with((0.0, 0.0), NumberingScheme::default(), |source| {
        ArrayKind::Linear {
            source,
            count_expr: "missing_parameter".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        }
    });
    let shown = preview_array(&data.arrays[0], &data, Some([0.0, 0.0]), None, 256);
    assert_eq!(shown, ArrayPreview::Unresolved);
    assert_eq!(shown.kept(), None);
}

#[test]
fn more_instances_than_the_limit_are_counted_not_walked() {
    let data = sketch_with((0.0, 0.0), NumberingScheme::default(), |source| {
        ArrayKind::Linear {
            source,
            count_expr: "300".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        }
    });
    let shown = preview_array(&data.arrays[0], &data, Some([0.0, 0.0]), None, 256);
    assert_eq!(shown, ArrayPreview::TooMany(300));
}

/// #2688: a linear pattern's suppressed instance and the one its mask drops
/// are previewed as dropped and baked as nothing.
#[test]
fn a_depopulated_linear_pattern_previews_the_baked_pads() {
    let at = (0.5, 0.25);
    let data = sketch_with(at, NumberingScheme::default(), |source| ArrayKind::Linear {
        source,
        count_expr: "5".into(),
        dx_expr: "1.27mm".into(),
        dy_expr: "0mm".into(),
        depopulation: Some(GridDepopulation {
            mask_expr: "i != 4".into(),
            suppressed_instances: vec![(1, 0)],
        }),
    });
    let (kept, all) = preview_and_bake(&data, at);
    assert_eq!(all, 5, "dropped instances are previewed too");
    let numbers: Vec<&str> = kept.iter().map(|(n, ..)| n.as_str()).collect();
    assert_eq!(numbers, vec!["1", "3", "4"]);
}
