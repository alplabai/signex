//! Misfiled: these two exercise `signex-sketch` state directly with no `Signex`/`app.update` involved. True home is `crates/signex-sketch/tests/`; kept here as-is per the split (see issue #432).

// ─────────────────────────────────────────────────────────────────
// v0.23 — sketch-mode pattern dispatchers (signex-sketch state side)
// ─────────────────────────────────────────────────────────────────

#[test]
fn array_kind_residual_count_is_one_per_kind_for_distance_pt_circle() {
    // Spot-check the new ConstraintKind variant integrates with
    // the residual_count machinery the panel relies on.
    use signex_sketch::constraint::{ConstraintKind, DimTarget};
    use signex_sketch::id::SketchEntityId;

    let kind = ConstraintKind::DistancePtCircle {
        point: SketchEntityId::new(),
        circle: SketchEntityId::new(),
        target: DimTarget::Literal(1.0),
    };
    assert_eq!(kind.residual_count(), 1);
}

#[test]
fn grid_depopulation_round_trips_suppressed_instances_through_app_layer() {
    // App layer never authors GridDepopulation directly — but
    // .snxfpt files load through signex-library and into the
    // FootprintEditorState's primitive. This test pins the schema:
    // empty mask + non-empty suppression list survives a TOML
    // round trip via signex-sketch.
    use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
    use signex_sketch::id::SketchEntityId;

    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "3".into(),
            ny_expr: "3".into(),
            dx_expr: "1mm".into(),
            dy_expr: "1mm".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: vec![(0, 0), (1, 1)],
            }),
        },
        numbering: NumberingScheme::default(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}
