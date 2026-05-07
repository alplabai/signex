use signex_sketch::SketchData;
use signex_sketch::array::{
    Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme, bga_row_letter,
};
use signex_sketch::attr::{
    BoardCutoutAttr, ChamferedCorners, CustomPadShape, DrillSpec, KeepoutAttr, KeepoutKinds,
    MaskOpeningAttr, PadAttr, PadKind, PadShape, PadSide, PasteApertureAttr, PasteAperturePattern,
    PourAttr, PourFillType, ThermalRelief, VScoreHintAttr, VScoreSide,
};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::layer::SignexLayer;
use uuid::Uuid;

#[test]
fn entity_id_round_trip() {
    let id = SketchEntityId(Uuid::new_v4());
    let s = serde_json::to_string(&id).unwrap();
    let back: SketchEntityId = serde_json::from_str(&s).unwrap();
    assert_eq!(id, back);
}

#[test]
fn constraint_id_round_trip() {
    let id = ConstraintId(Uuid::new_v4());
    let s = serde_json::to_string(&id).unwrap();
    let back: ConstraintId = serde_json::from_str(&s).unwrap();
    assert_eq!(id, back);
}

#[test]
fn plane_board_top_round_trip() {
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let s = toml::to_string(&p).unwrap();
    let back: Plane = toml::from_str(&s).unwrap();
    assert_eq!(p.kind, back.kind);
}

#[test]
fn plane_body_top_round_trip() {
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BodyTop {
            offset_z_expr: "= body_h".to_string(),
        },
    };
    let s = toml::to_string(&p).unwrap();
    let back: Plane = toml::from_str(&s).unwrap();
    assert_eq!(p.kind, back.kind);
}

#[test]
fn point_entity_round_trip() {
    let pt_id = SketchEntityId::new();
    let plane_id = PlaneId::new();
    let e = Entity::new(pt_id, plane_id, EntityKind::Point { x: 1.5, y: 2.5 });
    let s = toml::to_string(&e).unwrap();
    let back: Entity = toml::from_str(&s).unwrap();
    assert_eq!(e, back);
}

#[test]
fn line_entity_round_trip() {
    let plane_id = PlaneId::new();
    let p1 = SketchEntityId::new();
    let p2 = SketchEntityId::new();
    let mut e = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Line { start: p1, end: p2 },
    );
    e.construction = true;
    let s = toml::to_string(&e).unwrap();
    let back: Entity = toml::from_str(&s).unwrap();
    assert_eq!(e, back);
}

#[test]
fn arc_entity_round_trip() {
    let plane_id = PlaneId::new();
    let e = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Arc {
            center: SketchEntityId::new(),
            start: SketchEntityId::new(),
            end: SketchEntityId::new(),
            sweep_ccw: true,
        },
    );
    let s = toml::to_string(&e).unwrap();
    let back: Entity = toml::from_str(&s).unwrap();
    assert_eq!(e, back);
}

#[test]
fn circle_entity_round_trip() {
    let plane_id = PlaneId::new();
    let e = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Circle {
            center: SketchEntityId::new(),
            radius: 0.75,
        },
    );
    let s = toml::to_string(&e).unwrap();
    let back: Entity = toml::from_str(&s).unwrap();
    assert_eq!(e, back);
}

// ─── PadAttr round-trips ───

fn smd_rect_pad(num: &str) -> PadAttr {
    PadAttr {
        number: num.into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.25mm".into(),
        size_y_expr: "0.65mm".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: None,
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

#[test]
fn pad_attr_round_trip_rect() {
    let a = smd_rect_pad("1");
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_round_rect_corner_radius_round_trip() {
    let mut a = smd_rect_pad("1");
    a.shape = PadShape::RoundRect {
        radius_ratio_expr: "0.25".into(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_chamfered_round_trip() {
    let mut a = smd_rect_pad("1");
    a.shape = PadShape::Chamfered {
        chamfer_ratio_expr: "0.2".into(),
        corners: ChamferedCorners {
            top_left: true,
            top_right: true,
            ..Default::default()
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_custom_static_round_trip() {
    let mut a = smd_rect_pad("1");
    a.shape = PadShape::Custom(CustomPadShape::StaticPoints {
        points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]],
    });
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_with_rotation_and_offset_round_trip() {
    let mut a = smd_rect_pad("1");
    a.rotation_expr = Some("= leg_angle".into());
    a.offset_x_expr = Some("0.1mm".into());
    a.offset_y_expr = Some("= -row_offset".into());
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_tht_with_drill_round_trip() {
    let a = PadAttr {
        number: "1".into(),
        kind: PadKind::Tht,
        side: PadSide::All,
        shape: PadShape::Round,
        size_x_expr: "1.6mm".into(),
        size_y_expr: "1.6mm".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: Some(DrillSpec {
            diameter_expr: "0.8mm".into(),
            slot_length_expr: None,
            plated: true,
        }),
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_npt_mounting_hole_round_trip() {
    let a = PadAttr {
        number: "MH1".into(),
        kind: PadKind::NptHole,
        side: PadSide::All,
        shape: PadShape::Round,
        size_x_expr: "3.2mm".into(),
        size_y_expr: "3.2mm".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: Some(DrillSpec {
            diameter_expr: "3.2mm".into(),
            slot_length_expr: None,
            plated: false,
        }),
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_with_mask_paste_overrides_round_trip() {
    let mut a = smd_rect_pad("1");
    a.mask_margin_expr = Some("0.05mm".into());
    a.paste_margin_expr = Some("-0.025mm".into());
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pad_attr_thermal_grid_round_trip() {
    let mut a = smd_rect_pad("EP");
    a.size_x_expr = "= thermal_w".into();
    a.size_y_expr = "= thermal_h".into();
    a.mask_margin_expr = Some("0mm".into());
    a.paste_apertures = PasteAperturePattern::Grid {
        nx_expr: "3".into(),
        ny_expr: "3".into(),
        coverage_expr: "0.6".into(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn standalone_mask_opening_round_trip() {
    let a = MaskOpeningAttr {
        layer: SignexLayer::TopSolderMask,
    };
    let s = toml::to_string(&a).unwrap();
    let back: MaskOpeningAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn standalone_paste_aperture_round_trip() {
    let a = PasteApertureAttr {
        layer: SignexLayer::TopPaste,
    };
    let s = toml::to_string(&a).unwrap();
    let back: PasteApertureAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn fiducial_pad_round_trip() {
    let a = PadAttr {
        number: "FID1".into(),
        kind: PadKind::Fiducial,
        side: PadSide::Top,
        shape: PadShape::Round,
        size_x_expr: "1.0mm".into(),
        size_y_expr: "1.0mm".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: None,
        mask_margin_expr: Some("1.0mm".into()),
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pour_attr_round_trip_default() {
    let a = PourAttr {
        layer: SignexLayer::TopCopper,
        net: Some("GND".into()),
        fill_type: PourFillType::Solid,
        thermal_relief: ThermalRelief::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
    };
    let s = toml::to_string(&a).unwrap();
    let back: PourAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn pour_attr_hatched_with_overrides_round_trip() {
    let a = PourAttr {
        layer: SignexLayer::TopCopper,
        net: Some("GND".into()),
        fill_type: PourFillType::Hatched,
        thermal_relief: ThermalRelief {
            enabled: true,
            gap_expr: "= relief_gap".into(),
            spoke_width_expr: "0.3mm".into(),
            spoke_count: 2,
        },
        clearance_expr: Some("0.2mm".into()),
        min_thickness_expr: Some("0.15mm".into()),
        priority: 5,
    };
    let s = toml::to_string(&a).unwrap();
    let back: PourAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn keepout_attr_no_copper_round_trip() {
    let a = KeepoutAttr {
        layer: SignexLayer::TopCopper,
        kinds: KeepoutKinds::ALL_COPPER,
    };
    let s = toml::to_string(&a).unwrap();
    let back: KeepoutAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn keepout_attr_antenna_preset_round_trip() {
    let a = KeepoutAttr {
        layer: SignexLayer::TopCopper,
        kinds: KeepoutKinds::ANTENNA,
    };
    let s = toml::to_string(&a).unwrap();
    let back: KeepoutAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn keepout_attr_routing_only_round_trip() {
    let a = KeepoutAttr {
        layer: SignexLayer::TopCopper,
        kinds: KeepoutKinds::NO_ROUTING,
    };
    let s = toml::to_string(&a).unwrap();
    let back: KeepoutAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn board_cutout_through_round_trip() {
    let a = BoardCutoutAttr {
        edge_radius_expr: Some("0.8mm".into()),
        through: true,
    };
    let s = toml::to_string(&a).unwrap();
    let back: BoardCutoutAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn board_cutout_sharp_round_trip() {
    let a = BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    };
    let s = toml::to_string(&a).unwrap();
    let back: BoardCutoutAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn castellated_pad_round_trip() {
    let a = PadAttr {
        number: "1".into(),
        kind: PadKind::Castellated,
        side: PadSide::All,
        shape: PadShape::RoundRect {
            radius_ratio_expr: "0.25".into(),
        },
        size_x_expr: "1.5mm".into(),
        size_y_expr: "1.0mm".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: Some(DrillSpec {
            diameter_expr: "0.6mm".into(),
            slot_length_expr: None,
            plated: true,
        }),
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    };
    let s = toml::to_string(&a).unwrap();
    let back: PadAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn v_score_hint_default_round_trip() {
    let a = VScoreHintAttr {
        depth_fraction_expr: "0.333".into(),
        min_web_expr: None,
        side: VScoreSide::Both,
    };
    let s = toml::to_string(&a).unwrap();
    let back: VScoreHintAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn v_score_hint_with_overrides_round_trip() {
    let a = VScoreHintAttr {
        depth_fraction_expr: "= v_depth".into(),
        min_web_expr: Some("0.5mm".into()),
        side: VScoreSide::Top,
    };
    let s = toml::to_string(&a).unwrap();
    let back: VScoreHintAttr = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

// ─── Array round-trips ───

#[test]
fn linear_array_round_trip() {
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: SketchEntityId::new(),
            count_expr: "= pin_count / 4".into(),
            dx_expr: "= pad_pitch".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn grid_array_with_bga_numbering_round_trip() {
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "16".into(),
            ny_expr: "16".into(),
            dx_expr: "= ball_pitch".into(),
            dy_expr: "= ball_pitch".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'A',
            start_col: 1,
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn polar_array_round_trip() {
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source: SketchEntityId::new(),
            center: SketchEntityId::new(),
            count_expr: "8".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn polar_array_with_depopulation_round_trip() {
    // v0.22 Phase B5 — Polar gains depopulation parity with Grid.
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source: SketchEntityId::new(),
            center: SketchEntityId::new(),
            count_expr: "8".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: "i != 3".into(),
                suppressed_instances: Vec::new(),
            }),
        },
        numbering: NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn grid_array_with_corner_depopulation_round_trip() {
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "16".into(),
            ny_expr: "16".into(),
            dx_expr: "= ball_pitch".into(),
            dy_expr: "= ball_pitch".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: "!(i == 0 && j == 0) && !(i == nx-1 && j == 0) \
                            && !(i == 0 && j == ny-1) && !(i == nx-1 && j == ny-1)"
                    .into(),
                suppressed_instances: Vec::new(),
            }),
        },
        numbering: NumberingScheme::BgaRowCol {
            skip_letters: true,
            start_row: 'A',
            start_col: 1,
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn grid_array_with_suppressed_instances_round_trip() {
    // v0.23 — explicit per-instance suppression list survives the
    // round trip alongside any mask expression. Empty mask + non-empty
    // suppression list is a valid combination (Properties-panel
    // checkbox-only authoring path).
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "4".into(),
            ny_expr: "4".into(),
            dx_expr: "5mm".into(),
            dy_expr: "5mm".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: vec![(0, 0), (3, 3), (1, 2)],
            }),
        },
        numbering: NumberingScheme::default(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn polar_array_with_suppressed_instances_round_trip() {
    // v0.23 — Polar mirrors Grid's suppression list; entries use
    // `j = 0` since Polar is a 1-D array.
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source: SketchEntityId::new(),
            center: SketchEntityId::new(),
            count_expr: "8".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: vec![(2, 0), (5, 0)],
            }),
        },
        numbering: NumberingScheme::default(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn grid_depopulation_default_suppressed_instances_back_compat() {
    // v0.23 — round-trip an old-format depopulation TOML (mask_expr
    // only) to ensure `#[serde(default)]` keeps existing on-disk
    // arrays loadable.
    let toml_str = r#"
        [[entries]]
        kind = "Grid"
        source = "00000000-0000-0000-0000-000000000001"
        nx_expr = "2"
        ny_expr = "2"
        dx_expr = "1mm"
        dy_expr = "1mm"
        [entries.depopulation]
        mask_expr = "i != 0"
    "#;
    #[derive(serde::Deserialize)]
    struct Wrapper {
        entries: Vec<ArrayKind>,
    }
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    let kind = &w.entries[0];
    if let ArrayKind::Grid { depopulation, .. } = kind {
        let d = depopulation.as_ref().expect("depop present");
        assert_eq!(d.mask_expr, "i != 0");
        assert!(d.suppressed_instances.is_empty());
    } else {
        panic!("expected Grid kind");
    }
}

#[test]
fn explicit_numbering_round_trip() {
    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: SketchEntityId::new(),
            count_expr: "5".into(),
            dx_expr: "= pad_pitch".into(),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::Explicit {
            names: vec![
                "GND".into(),
                "VBUS".into(),
                "D-".into(),
                "D+".into(),
                "ID".into(),
            ],
        },
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

#[test]
fn bga_letters_basic() {
    assert_eq!(bga_row_letter(0, true, 'A'), "A");
    assert_eq!(bga_row_letter(1, true, 'A'), "B");
    assert_eq!(bga_row_letter(7, true, 'A'), "H");
    // Index 8 would be 'I' which is skipped → 'J'
    assert_eq!(bga_row_letter(8, true, 'A'), "J");
    // 20 letters in the skipped alphabet → index 20 = "AA"
    assert_eq!(bga_row_letter(20, true, 'A'), "AA");
}

#[test]
fn bga_letters_no_skip() {
    assert_eq!(bga_row_letter(8, false, 'A'), "I");
    assert_eq!(bga_row_letter(25, false, 'A'), "Z");
    assert_eq!(bga_row_letter(26, false, 'A'), "AA");
}

// ─── SketchData round-trip ───

#[test]
fn empty_sketch_round_trip() {
    let s = SketchData::default();
    let toml_s = toml::to_string(&s).unwrap();
    let back: SketchData = toml::from_str(&toml_s).unwrap();
    assert_eq!(s, back);
}

#[test]
fn populated_sketch_round_trip() {
    let plane_id = PlaneId::new();
    let p1 = SketchEntityId::new();
    let p2 = SketchEntityId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane_id,
        kind: PlaneKind::BoardTop,
    });
    data.entities.push(Entity::new(
        p1,
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    data.entities.push(Entity::new(
        p2,
        plane_id,
        EntityKind::Point { x: 1.0, y: 0.0 },
    ));
    data.entities.push(Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Line { start: p1, end: p2 },
    ));
    data.parameters.0.insert("pad_pitch".into(), "0.5mm".into());
    let s = toml::to_string(&data).unwrap();
    let back: SketchData = toml::from_str(&s).unwrap();
    assert_eq!(data, back);
}

#[test]
fn distance_pt_circle_constraint_round_trip() {
    // v0.23 — the new parametric DistancePtCircle constraint must
    // round-trip through TOML cleanly, including its DimTarget. Both
    // literal and Expr targets are exercised.
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};

    let mut data = SketchData::default();
    let plane_id = PlaneId::new();
    data.planes.push(Plane {
        id: plane_id,
        kind: PlaneKind::BoardTop,
    });
    let centre_id = SketchEntityId::new();
    let circle_id = SketchEntityId::new();
    let anchor_id = SketchEntityId::new();
    data.entities.push(Entity::new(
        centre_id,
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    data.entities.push(Entity::new(
        circle_id,
        plane_id,
        EntityKind::Circle {
            center: centre_id,
            radius: 5.0,
        },
    ));
    data.entities.push(Entity::new(
        anchor_id,
        plane_id,
        EntityKind::Point { x: 7.0, y: 0.0 },
    ));
    // Literal-target constraint
    data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: anchor_id,
            circle: circle_id,
            target: DimTarget::Literal(2.0),
        },
    });
    // Expr-target constraint (parametric)
    data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: anchor_id,
            circle: circle_id,
            target: DimTarget::Expr("offset_dist".into()),
        },
    });
    data.parameters
        .0
        .insert("offset_dist".into(), "0.5mm".into());

    let s = toml::to_string(&data).unwrap();
    let back: SketchData = toml::from_str(&s).unwrap();
    assert_eq!(data, back);
}
