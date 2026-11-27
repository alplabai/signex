//! mockup row 189 — where parameters are used, and renaming, deleting and minting one
//! without leaving a dangling reference.

use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadShape, PourAttr};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::expr::ExprError;
use signex_sketch::id::ConstraintId;
use signex_sketch::parameter::resolve;
use signex_sketch::parameter_refs::{
    AttributeField, DeleteParameterError, DeletePolicy, MakeParameterError, PadField,
    ParameterNameError, ParameterUse, PatternField, PatternKind, RowValue, check_new_name,
    delete_parameter, literal_of, parameter_from_dimension, rename_parameter, resolve_rows,
    unique_name, used_by, uses_by_parameter,
};
use signex_sketch::unit::Quantity;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::common::Sketch;

struct Fixture {
    sketch: Sketch,
    dim: ConstraintId,
    angle: ConstraintId,
    array: ArrayId,
}

/// `pitch` drives a distance, a linear pattern's spacing, a pad's size and
/// a pour's clearance; `pitch2` is a separate name; `span` reads `pitch`.
fn fixture() -> Fixture {
    let mut sketch = Sketch::new();
    let a = sketch.add_point(0.0, 0.0);
    let b = sketch.add_point(1.0, 0.0);
    let c = sketch.add_point(0.0, 1.0);
    let l1 = sketch.add_line(a, b);
    let l2 = sketch.add_line(a, c);
    let data = &mut sketch.data;
    data.parameters.insert("pitch", "0.65mm");
    data.parameters.insert("pitch2", "1mm");
    data.parameters.insert("rows", "4");
    data.parameters.insert("span", "= pitch * (rows - 1)");
    let dim = ConstraintId::new();
    data.constraints.push(Constraint {
        id: dim,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Expr("= pitch".into()),
        },
    });
    let angle = ConstraintId::new();
    data.constraints.push(Constraint {
        id: angle,
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(std::f64::consts::FRAC_PI_2),
        },
    });
    let array = ArrayId::new();
    data.arrays.push(Array {
        id: array,
        kind: ArrayKind::Linear {
            source: a,
            count_expr: "rows".into(),
            dx_expr: "pitch".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    let pad_entity = data.entities[0].id;
    data.entities[0].pad = Some(PadAttr {
        number: "1".into(),
        shape: PadShape::Rect,
        size_x_expr: "pitch2 * 0.5".into(),
        size_y_expr: "= pitch / 2".into(),
        ..PadAttr::default()
    });
    assert_eq!(pad_entity, a);
    data.entities[1].pour = Some(PourAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        net: None,
        fill_type: Default::default(),
        thermal_relief: Default::default(),
        clearance_expr: Some("pitch".into()),
        min_thickness_expr: None,
        priority: 0,
        corners: Default::default(),
        corner_size: 0.0,
    });
    Fixture {
        sketch,
        dim,
        angle,
        array,
    }
}

#[test]
fn used_by_finds_parameters_dimensions_patterns_and_attributes() {
    let f = fixture();
    let uses = used_by(&f.sketch.data, "pitch");
    let a = f.sketch.data.entities[0].id;
    let b = f.sketch.data.entities[1].id;
    assert_eq!(
        uses,
        vec![
            ParameterUse::Parameter {
                name: "span".into()
            },
            ParameterUse::Dimension {
                constraint: f.dim,
                angle: false
            },
            ParameterUse::Pattern {
                array: f.array,
                kind: PatternKind::Linear,
                field: PatternField::Dx
            },
            ParameterUse::Pad {
                entity: a,
                number: "1".into(),
                field: PadField::SizeY
            },
            ParameterUse::Attribute {
                entity: b,
                field: AttributeField::PourClearance
            },
        ]
    );
}

#[test]
fn a_longer_name_is_not_a_use() {
    let f = fixture();
    // `pitch2 * 0.5` reads pitch2 only.
    let uses = used_by(&f.sketch.data, "pitch2");
    assert_eq!(uses.len(), 1);
    assert!(matches!(
        uses[0],
        ParameterUse::Pad {
            field: PadField::SizeX,
            ..
        }
    ));
    assert!(used_by(&f.sketch.data, "span").is_empty());
}

#[test]
fn uses_by_parameter_matches_used_by_for_every_row() {
    let f = fixture();
    let all = uses_by_parameter(&f.sketch.data);
    for name in ["pitch", "pitch2", "rows", "span"] {
        assert_eq!(all[name], used_by(&f.sketch.data, name), "{name}");
    }
}

#[test]
fn rename_rewrites_every_reference_and_keeps_the_text_around_it() {
    let mut f = fixture();
    rename_parameter(&mut f.sketch.data, "pitch", "pad_pitch").expect("renamed");
    let data = &f.sketch.data;
    assert_eq!(data.parameters.get_raw("pitch"), None);
    assert_eq!(data.parameters.get_raw("pad_pitch"), Some("0.65mm"));
    assert_eq!(
        data.parameters.get_raw("span"),
        Some("= pad_pitch * (rows - 1)")
    );
    assert_eq!(data.parameters.get_raw("pitch2"), Some("1mm"));
    assert_eq!(
        data.constraints[0].kind.dim_target(),
        Some(&DimTarget::Expr("= pad_pitch".into()))
    );
    let pad = data.entities[0].pad.as_ref().unwrap();
    assert_eq!(pad.size_x_expr, "pitch2 * 0.5");
    assert_eq!(pad.size_y_expr, "= pad_pitch / 2");
    let pour = data.entities[1].pour.as_ref().unwrap();
    assert_eq!(pour.clearance_expr.as_deref(), Some("pad_pitch"));
    // Nothing reads the old name any more; the values are unchanged.
    assert!(used_by(data, "pitch").is_empty());
    assert!((resolve(&data.parameters).unwrap()["span"] - 1.95).abs() < 1e-9);
}

#[test]
fn rename_refuses_a_taken_reserved_or_malformed_name() {
    let mut f = fixture();
    let before = f.sketch.data.clone();
    let refused = [
        ("pitch2", ParameterNameError::Taken("pitch2".into())),
        ("pi", ParameterNameError::Reserved("pi".into())),
        ("i", ParameterNameError::Reserved("i".into())),
        ("2x", ParameterNameError::NotAName("2x".into())),
        ("a.b", ParameterNameError::NotAName("a.b".into())),
        ("", ParameterNameError::Empty),
    ];
    for (name, error) in refused {
        assert_eq!(
            rename_parameter(&mut f.sketch.data, "pitch", name),
            Err(error),
            "{name}"
        );
    }
    assert_eq!(
        rename_parameter(&mut f.sketch.data, "nope", "x"),
        Err(ParameterNameError::Missing("nope".into()))
    );
    assert_eq!(f.sketch.data, before, "a refused rename changes nothing");
    assert_eq!(
        rename_parameter(&mut f.sketch.data, "pitch", "pitch"),
        Ok(())
    );
    assert_eq!(f.sketch.data, before);
}

#[test]
fn delete_refuses_a_used_parameter_and_names_the_uses() {
    let mut f = fixture();
    let before = f.sketch.data.clone();
    match delete_parameter(&mut f.sketch.data, "pitch", DeletePolicy::Refuse) {
        Err(DeleteParameterError::InUse(uses)) => assert_eq!(uses.len(), 5),
        other => panic!("expected InUse, got {other:?}"),
    }
    assert_eq!(f.sketch.data, before);
}

#[test]
fn delete_with_inline_puts_the_value_in_every_use() {
    let mut f = fixture();
    let before = resolve(&f.sketch.data.parameters).unwrap();
    let changed =
        delete_parameter(&mut f.sketch.data, "pitch", DeletePolicy::Inline).expect("deleted");
    assert_eq!(changed, 5);
    let data = &f.sketch.data;
    assert_eq!(data.parameters.get_raw("pitch"), None);
    assert_eq!(
        data.parameters.get_raw("span"),
        Some("= 0.65mm * (rows - 1)")
    );
    assert_eq!(
        data.constraints[0].kind.dim_target(),
        Some(&DimTarget::Expr("= 0.65mm".into()))
    );
    let pad = data.entities[0].pad.as_ref().unwrap();
    assert_eq!(pad.size_y_expr, "= 0.65mm / 2");
    assert_eq!(pad.size_x_expr, "pitch2 * 0.5", "pitch2 is not pitch");
    let after = resolve(&data.parameters).unwrap();
    assert_eq!(after["span"], before["span"]);
}

#[test]
fn delete_of_an_unused_parameter_needs_no_inline() {
    let mut f = fixture();
    assert_eq!(
        delete_parameter(&mut f.sketch.data, "span", DeletePolicy::Refuse),
        Ok(0)
    );
    assert_eq!(f.sketch.data.parameters.get_raw("span"), None);
    assert_eq!(
        delete_parameter(&mut f.sketch.data, "span", DeletePolicy::Inline),
        Err(DeleteParameterError::Missing("span".into()))
    );
}

#[test]
fn delete_of_a_parameter_without_a_value_inlines_its_expression() {
    let mut f = fixture();
    f.sketch.data.parameters.insert("w", "= bodyy + 1mm");
    f.sketch.data.parameters.insert("h", "2 * w");
    delete_parameter(&mut f.sketch.data, "w", DeletePolicy::Inline).expect("deleted");
    assert_eq!(
        f.sketch.data.parameters.get_raw("h"),
        Some("2 * (bodyy + 1mm)")
    );
}

#[test]
fn each_row_resolves_on_its_own() {
    let mut f = fixture();
    f.sketch.data.parameters.insert("bad", "= bodyy - 0.1mm");
    f.sketch.data.parameters.insert("reads_bad", "= bad * 2");
    let rows = resolve_rows(&f.sketch.data.parameters);
    assert_eq!(rows["pitch"], RowValue::Value(Quantity::length(0.65)));
    assert_eq!(rows["rows"], RowValue::Value(Quantity::count(4.0)));
    assert_eq!(
        rows["bad"],
        RowValue::Error(ExprError::Unknown("bodyy".into()))
    );
    assert_eq!(rows["reads_bad"], RowValue::Blocked { by: "bad".into() });
    match &rows["span"] {
        RowValue::Value(q) => assert!((q.value - 1.95).abs() < 1e-9),
        other => panic!("span keeps its value: {other:?}"),
    }
}

#[test]
fn a_cycle_marks_its_rows_and_a_parse_error_its_own() {
    let mut f = fixture();
    f.sketch.data.parameters.insert("a", "= b");
    f.sketch.data.parameters.insert("b", "= a");
    f.sketch.data.parameters.insert("c", "2 *");
    let rows = resolve_rows(&f.sketch.data.parameters);
    assert!(matches!(rows["a"], RowValue::Error(ExprError::Cycle(_))));
    assert!(matches!(rows["b"], RowValue::Error(ExprError::Cycle(_))));
    assert!(matches!(
        rows["c"],
        RowValue::Error(ExprError::Parse { .. })
    ));
}

#[test]
fn a_dimension_becomes_a_parameter_with_its_value_and_unit() {
    let mut f = fixture();
    let name = parameter_from_dimension(&mut f.sketch.data, f.angle).expect("made");
    assert_eq!(name, "angle_1");
    let data = &f.sketch.data;
    assert_eq!(data.parameters.get_raw("angle_1"), Some("90deg"));
    assert_eq!(
        data.constraints[1].kind.dim_target(),
        Some(&DimTarget::Expr("angle_1".into()))
    );
    assert!(
        (resolve(&data.parameters).unwrap()["angle_1"] - std::f64::consts::FRAC_PI_2).abs() < 1e-9
    );
}

#[test]
fn a_literal_distance_becomes_a_length_parameter() {
    let mut f = fixture();
    if let ConstraintKind::DistancePtPt { target, .. } = &mut f.sketch.data.constraints[0].kind {
        *target = DimTarget::Literal(1.27);
    }
    f.sketch.data.parameters.insert("dim_1", "3mm");
    let name = parameter_from_dimension(&mut f.sketch.data, f.dim).expect("made");
    assert_eq!(name, "dim_2", "the first free name");
    assert_eq!(f.sketch.data.parameters.get_raw("dim_2"), Some("1.27mm"));
}

#[test]
fn a_dimension_that_reads_one_parameter_keeps_it() {
    let mut f = fixture();
    let before = f.sketch.data.clone();
    assert_eq!(
        parameter_from_dimension(&mut f.sketch.data, f.dim),
        Ok("pitch".into())
    );
    assert_eq!(f.sketch.data, before);
}

#[test]
fn only_a_dimension_can_become_a_parameter() {
    let mut f = fixture();
    let a = f.sketch.data.entities[0].id;
    let fixed = ConstraintId::new();
    f.sketch.data.constraints.push(Constraint {
        id: fixed,
        kind: ConstraintKind::Fixed { point: a },
    });
    assert_eq!(
        parameter_from_dimension(&mut f.sketch.data, fixed),
        Err(MakeParameterError::NotADimension)
    );
    assert_eq!(
        parameter_from_dimension(&mut f.sketch.data, ConstraintId::new()),
        Err(MakeParameterError::NoConstraint)
    );
}

#[test]
fn names_and_literals() {
    let f = fixture();
    let table = &f.sketch.data.parameters;
    assert_eq!(unique_name(table, "param"), "param_1");
    assert_eq!(check_new_name(table, "fresh"), Ok(()));
    assert_eq!(
        check_new_name(table, "rows"),
        Err(ParameterNameError::Taken("rows".into()))
    );
    assert_eq!(literal_of(Quantity::length(0.65)), "0.65mm");
    assert_eq!(literal_of(Quantity::length(-0.0)), "0mm");
    assert_eq!(literal_of(Quantity::count(4.0)), "4");
    assert_eq!(literal_of(Quantity::angle(std::f64::consts::PI)), "180deg");
}
