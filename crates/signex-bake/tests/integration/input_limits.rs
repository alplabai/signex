//! #678 — the bake must refuse non-finite pad dimensions and cap array
//! instance counts instead of emitting NaN pads or hanging the editor.

use std::collections::HashMap;

use signex_bake::array::MAX_ARRAY_INSTANCES;
use signex_bake::{bake_arrays, bake_pads};
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

fn pad(w: &str, h: &str) -> PadAttr {
    PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: w.into(),
        size_y_expr: h.into(),
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

/// A sketch with one pad-carrying point at the origin (plus the plane).
fn sketch_with_pad(w: &str, h: &str) -> (SketchData, SketchEntityId, PlaneId) {
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let plane_id = plane.id;
    let mut data = SketchData::default();
    data.planes.push(plane);
    let id = SketchEntityId::new();
    let mut entity = Entity::new(id, plane_id, EntityKind::Point { x: 0.0, y: 0.0 });
    entity.pad = Some(pad(w, h));
    data.entities.push(entity);
    (data, id, plane_id)
}

fn solved(data: &SketchData) -> signex_sketch::solver::FullSolveOutput {
    Solver::default()
        .solve(data, &ResolvedParams::new())
        .expect("fixture solves")
}

fn bake_array(kind: ArrayKind, data: &mut SketchData) -> (usize, Vec<String>) {
    data.arrays.push(Array {
        id: ArrayId::new(),
        kind,
        numbering: NumberingScheme::default(),
    });
    let solve = solved(data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    bake_arrays(data, &solve, &HashMap::new(), &mut out, &mut warnings).expect("bake returns Ok");
    (out.len(), warnings)
}

fn assert_capped(baked: usize, warnings: &[String]) {
    assert_eq!(baked, 0, "an over-cap array must bake nothing");
    assert!(
        warnings.iter().any(|w| w.contains("instances")),
        "the cap must be reported, got {warnings:?}"
    );
}

#[test]
fn pad_size_from_nan_expression_is_an_error_not_a_nan_pad() {
    let (data, _, _) = sketch_with_pad("2mm * (0-1)^0.5", "1mm");
    let solve = solved(&data);
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    let result = bake_pads(&data, &solve, &HashMap::new(), &mut out, &mut warnings);
    assert!(result.is_err(), "NaN pad size must not bake: {out:?}");
}

#[test]
fn grid_array_over_the_cap_is_skipped_with_a_warning() {
    let (mut data, src, _) = sketch_with_pad("1mm", "1mm");
    // 200 x 200 = 40_000 instances, past MAX_ARRAY_INSTANCES.
    let (baked, warnings) = bake_array(
        ArrayKind::Grid {
            source: src,
            nx_expr: "200".into(),
            ny_expr: "200".into(),
            dx_expr: "1mm".into(),
            dy_expr: "1mm".into(),
            depopulation: None,
        },
        &mut data,
    );
    const { assert!(200 * 200 > MAX_ARRAY_INSTANCES) };
    assert_capped(baked, &warnings);
}

#[test]
fn grid_array_with_a_product_that_overflows_is_skipped() {
    let (mut data, src, _) = sketch_with_pad("1mm", "1mm");
    let (baked, warnings) = bake_array(
        ArrayKind::Grid {
            source: src,
            nx_expr: "9000000000000000000".into(),
            ny_expr: "9000000000000000000".into(),
            dx_expr: "1mm".into(),
            dy_expr: "1mm".into(),
            depopulation: None,
        },
        &mut data,
    );
    assert_capped(baked, &warnings);
}

#[test]
fn linear_array_over_the_cap_is_skipped_with_a_warning() {
    let (mut data, src, _) = sketch_with_pad("1mm", "1mm");
    let (baked, warnings) = bake_array(
        ArrayKind::Linear {
            source: src,
            count_expr: "1000000".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
        },
        &mut data,
    );
    assert_capped(baked, &warnings);
}

#[test]
fn polar_array_over_the_cap_is_skipped_with_a_warning() {
    let (mut data, src, plane) = sketch_with_pad("1mm", "1mm");
    let centre = SketchEntityId::new();
    data.entities.push(Entity::new(
        centre,
        plane,
        EntityKind::Point { x: 5.0, y: 0.0 },
    ));
    let (baked, warnings) = bake_array(
        ArrayKind::Polar {
            source: src,
            center: centre,
            count_expr: "1000000".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        },
        &mut data,
    );
    assert_capped(baked, &warnings);
}

#[test]
fn array_at_the_cap_still_bakes() {
    let (mut data, src, _) = sketch_with_pad("1mm", "1mm");
    let (baked, warnings) = bake_array(
        ArrayKind::Linear {
            source: src,
            count_expr: "8".into(),
            dx_expr: "1mm".into(),
            dy_expr: "0mm".into(),
        },
        &mut data,
    );
    assert_eq!(baked, 8, "warnings: {warnings:?}");
}
