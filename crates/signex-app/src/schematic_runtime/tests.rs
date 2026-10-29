use super::*;

pub(super) fn empty_snapshot() -> SchematicRenderSnapshot {
    SchematicRenderSnapshot {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.0.0".into(),
        paper_size: "A4".into(),
        root_sheet_page: "1".into(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: Default::default(),
        net_colors: HashMap::new(),
        drawing_placements: Default::default(),
        frame: Default::default(),
        font_family: None,
    }
}

#[test]
fn hit_test_wire_uses_segment_tolerance() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    snapshot.wires.push(signex_types::schematic::Wire {
        uuid: wire_uuid,
        end: Point::new(10.0, 0.0),
        stroke_width: 0.15,
        ..Default::default()
    });

    let hit = hit_test::hit_test(&snapshot, &Default::default(), 5.0, 0.08);
    assert_eq!(hit, Some(SelectedItem::new(wire_uuid, SelectedKind::Wire)));
}

#[test]
fn hit_test_picks_the_right_wire_and_bus_among_many() {
    let mut snapshot = empty_snapshot();
    let mut wires = Vec::new();
    for i in 0..5 {
        let uuid = uuid::Uuid::new_v4();
        wires.push(uuid);
        snapshot.wires.push(signex_types::schematic::Wire {
            uuid,
            start: Point::new(0.0, i as f64 * 10.0),
            end: Point::new(10.0, i as f64 * 10.0),
            stroke_width: 0.15,
            ..Default::default()
        });
    }
    let bus_uuid = uuid::Uuid::new_v4();
    snapshot.buses.push(signex_types::schematic::Bus {
        uuid: bus_uuid,
        start: Point::new(50.0, 0.0),
        end: Point::new(60.0, 0.0),
    });

    for (i, uuid) in wires.iter().enumerate() {
        let hit = hit_test::hit_test(&snapshot, &Default::default(), 5.0, i as f64 * 10.0);
        assert_eq!(hit, Some(SelectedItem::new(*uuid, SelectedKind::Wire)));
    }
    let hit = hit_test::hit_test(&snapshot, &Default::default(), 55.0, 0.0);
    assert_eq!(hit, Some(SelectedItem::new(bus_uuid, SelectedKind::Bus)));
    assert_eq!(
        hit_test::hit_test(&snapshot, &Default::default(), 5.0, 5.0),
        None
    );
}

#[test]
fn hit_test_rect_mode_distinguishes_inside_and_touching() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    snapshot.wires.push(signex_types::schematic::Wire {
        uuid: wire_uuid,
        start: Point::new(-4.0, 0.0),
        end: Point::new(4.0, 0.0),
        stroke_width: 0.15,
        ..Default::default()
    });

    let rect = Aabb::new(0.0, -0.2, 2.0, 0.2);
    let inside = hit_test::hit_test_rect_mode(
        &snapshot,
        &Default::default(),
        &rect,
        hit_test::SelectionMode::Inside,
    );
    let touching = hit_test::hit_test_rect_mode(
        &snapshot,
        &Default::default(),
        &rect,
        hit_test::SelectionMode::Touching,
    );

    assert!(!inside.contains(&SelectedItem::new(wire_uuid, SelectedKind::Wire)));
    assert!(touching.contains(&SelectedItem::new(wire_uuid, SelectedKind::Wire)));
}

#[test]
fn hit_test_polygon_selects_wire_and_label_by_anchor() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    let label_uuid = uuid::Uuid::new_v4();

    snapshot.wires.push(signex_types::schematic::Wire {
        uuid: wire_uuid,
        start: Point::new(1.0, 1.0),
        end: Point::new(9.0, 1.0),
        stroke_width: 0.15,
        ..Default::default()
    });
    snapshot.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: label_uuid,
        text: "NET_MAIN".into(),
        position: Point::new(4.0, 4.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
    });

    let polygon = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)];
    let hits = hit_test::hit_test_polygon(&snapshot, &polygon);

    assert!(hits.contains(&SelectedItem::new(wire_uuid, SelectedKind::Wire)));
    assert!(hits.contains(&SelectedItem::new(label_uuid, SelectedKind::Label)));
}

fn field_symbol(ref_text: Option<TextProp>) -> Symbol {
    Symbol {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".into(),
        reference: "R1".into(),
        value: "10k".into(),
        footprint: String::new(),
        datasheet: String::new(),
        position: Point::new(50.0, 50.0),
        rotation: 90.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        body_style: Default::default(),
        is_power: false,
        ref_text,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: Vec::new(),
        pin_uuids: HashMap::new(),
        instances: Vec::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    }
}

#[test]
fn reference_field_draws_and_hit_tests_at_stored_text_prop() {
    let prop = TextProp {
        position: Point::new(70.0, 30.0),
        rotation: 0.0,
        font_size: 2.0,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        hidden: false,
        font_family: None,
    };
    let symbol = field_symbol(Some(prop));

    let drawn = snapshot::field_text_input(
        &symbol,
        "R1",
        symbol.ref_text.as_ref(),
        snapshot::FieldSlot::Reference,
        [1.0; 4],
        None,
    )
    .expect("visible field is drawn");
    assert_eq!(drawn.position, [70.0, 30.0]);
    assert_eq!(drawn.size_mm, 2.0);
    // Field rotation is relative to the symbol: symbol 90 + prop 0 is vertical.
    assert!((drawn.rotation_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(drawn.h_align, HAlign::Left);

    let mut snapshot = empty_snapshot();
    let uuid = symbol.uuid;
    snapshot.symbols.push(symbol);
    // Left/Top anchored: the box extends right and down from the anchor.
    let hit = hit_test::hit_test(&snapshot, &Default::default(), 70.1, 30.1);
    assert_eq!(
        hit,
        Some(SelectedItem::new(uuid, SelectedKind::SymbolRefField))
    );
}

fn drawn_style(
    sym_rot: f64,
    prop_rot: f64,
    mirror_x: bool,
    mirror_y: bool,
) -> (f32, HAlign, VAlign) {
    let prop = TextProp {
        position: Point::new(0.0, 0.0),
        rotation: prop_rot,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        hidden: false,
        font_family: None,
    };
    let mut symbol = field_symbol(Some(prop));
    symbol.rotation = sym_rot;
    symbol.mirror_x = mirror_x;
    symbol.mirror_y = mirror_y;
    let t = snapshot::field_text_input(
        &symbol,
        "R1",
        symbol.ref_text.as_ref(),
        snapshot::FieldSlot::Reference,
        [1.0; 4],
        None,
    )
    .unwrap();
    (t.rotation_rad.to_degrees().round(), t.h_align, t.v_align)
}

#[test]
fn field_rotation_is_relative_to_symbol() {
    // Autoplace stores prop 90 on a 90-degree symbol so it reads horizontally.
    let (rot, h, _) = drawn_style(90.0, 90.0, false, false);
    assert_eq!(rot, 0.0);
    assert_eq!(h, HAlign::Right);
    // 180: folded to a readable horizontal glyph, justification flipped.
    let (rot, h, _) = drawn_style(180.0, 0.0, false, false);
    assert_eq!(rot, 0.0);
    assert_eq!(h, HAlign::Right);
    // Mirrors flip justification without rotating.
    let (rot, h, v) = drawn_style(0.0, 0.0, true, true);
    assert_eq!(rot, 0.0);
    assert_eq!((h, v), (HAlign::Right, VAlign::Bottom));
}

#[test]
fn dragged_reference_field_draw_and_hit_test_follow_new_position() {
    let prop = TextProp {
        position: Point::new(70.0, 30.0),
        rotation: 0.0,
        font_size: 2.0,
        justify_h: HAlign::Center,
        justify_v: VAlign::Center,
        hidden: false,
        font_family: None,
    };
    let mut symbol = field_symbol(Some(prop));
    symbol.rotation = 0.0;
    let uuid = symbol.uuid;
    let mut sheet = empty_snapshot();
    sheet.symbols.push(symbol);

    let mut engine = signex_engine::Engine::new(sheet).unwrap();
    engine
        .execute(signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(uuid, SelectedKind::SymbolRefField)],
            dx: 10.0,
            dy: 5.0,
            keep_wires: false,
        })
        .unwrap();
    let moved = engine.document();

    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
    };
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let snap =
        build_renderer_snapshot(moved, &transform, &colors, bounds, None, Default::default());
    let text = snap
        .reference_value_texts
        .iter()
        .find(|t| t.content == "R1")
        .expect("reference text is drawn");
    assert_eq!(text.position, [80.0, 35.0]);

    let new_hit = hit_test::hit_test(moved, &Default::default(), 80.0, 35.0);
    assert_eq!(
        new_hit,
        Some(SelectedItem::new(uuid, SelectedKind::SymbolRefField))
    );
    assert_ne!(
        hit_test::hit_test(moved, &Default::default(), 70.0, 30.0),
        Some(SelectedItem::new(uuid, SelectedKind::SymbolRefField))
    );
}

#[test]
fn field_without_text_prop_keeps_default_offset_and_hidden_field_is_skipped() {
    let symbol = field_symbol(None);
    let fallback = snapshot::field_text_input(
        &symbol,
        "R1",
        None,
        snapshot::FieldSlot::Reference,
        [1.0; 4],
        None,
    )
    .unwrap();
    assert_eq!(fallback.position, [50.0, 46.5]);

    let hidden = TextProp {
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Center,
        justify_v: VAlign::Center,
        hidden: true,
        font_family: None,
    };
    assert!(
        snapshot::field_text_input(
            &symbol,
            "R1",
            Some(&hidden),
            snapshot::FieldSlot::Reference,
            [1.0; 4],
            None,
        )
        .is_none()
    );
}

#[test]
fn long_text_stays_visible_while_anchor_is_just_offscreen() {
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let anchor = iced::Point::new(-23.0, 300.0);
    let flag = Aabb::new(-5.0, -1.0, 5.0, 1.0);
    assert!(!point_visible(anchor, bounds, 22.0));
    assert!(text_anchor_visible(anchor, bounds, 22.0, &flag, 48.0));
    let far = iced::Point::new(-2000.0, 300.0);
    assert!(!text_anchor_visible(far, bounds, 22.0, &flag, 48.0));
}

fn ref_field_hit(rotation: f64, h: HAlign, v: VAlign, hidden: bool, x: f64, y: f64) -> bool {
    let prop = TextProp {
        position: Point::new(70.0, 30.0),
        rotation,
        font_size: 2.0,
        justify_h: h,
        justify_v: v,
        hidden,
        font_family: None,
    };
    let mut symbol = field_symbol(Some(prop));
    symbol.rotation = 0.0;
    let uuid = symbol.uuid;
    let mut sheet = empty_snapshot();
    sheet.symbols.push(symbol);
    hit_test::hit_test(&sheet, &Default::default(), x, y)
        == Some(SelectedItem::new(uuid, SelectedKind::SymbolRefField))
}

#[test]
fn rotated_right_justified_field_hit_box_follows_rotated_text() {
    // Drawn at x in [69,71], y in [27.6,30.0].
    assert!(ref_field_hit(
        90.0,
        HAlign::Right,
        VAlign::Center,
        false,
        70.6,
        27.8
    ));
}

#[test]
fn rotated_270_field_hit_box_follows_rotated_text() {
    // 270 draws at 90 with the vertical alignment folded: Top -> Bottom.
    // Drawn at x in [70,72], y in [27.6,30.0].
    assert!(ref_field_hit(
        270.0,
        HAlign::Right,
        VAlign::Top,
        false,
        71.0,
        28.0
    ));
}

#[test]
fn rotated_left_justified_field_hit_box_follows_rotated_text() {
    // Drawn at x in [69,71], y in [30.0,32.4].
    assert!(ref_field_hit(
        90.0,
        HAlign::Left,
        VAlign::Center,
        false,
        70.5,
        31.0
    ));
}

#[test]
fn hidden_field_has_no_hit_box() {
    assert!(ref_field_hit(
        0.0,
        HAlign::Center,
        VAlign::Center,
        false,
        70.0,
        30.0
    ));
    assert!(!ref_field_hit(
        0.0,
        HAlign::Center,
        VAlign::Center,
        true,
        70.0,
        30.0
    ));
}

fn anchored_label(label_type: LabelType, rotation: f64) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        text: "DATA_BUS_0".into(),
        position: Point::new(20.0, 30.0),
        rotation,
        label_type,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        font_family: None,
    }
}

/// Extent of `bbox` along the unit axis `(dx, dy)`, measured from `anchor`.
fn extent_along(bbox: &Aabb, anchor: (f64, f64), (dx, dy): (f64, f64)) -> (f64, f64) {
    let along = |x: f64, y: f64| (x - anchor.0) * dx + (y - anchor.1) * dy;
    let corners = [
        along(bbox.min_x, bbox.min_y),
        along(bbox.min_x, bbox.max_y),
        along(bbox.max_x, bbox.min_y),
        along(bbox.max_x, bbox.max_y),
    ];
    (
        corners.iter().cloned().fold(f64::INFINITY, f64::min),
        corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    )
}

#[test]
fn label_box_starts_at_anchor_and_extends_in_label_direction() {
    for label_type in [LabelType::Net, LabelType::Global, LabelType::Hierarchical] {
        for rotation in [0.0_f64, 90.0, 180.0, 270.0] {
            let label = anchored_label(label_type, rotation);
            let anchor = (label.position.x, label.position.y);
            let bbox = label_aabb(&label, None);
            let (sin, cos) = rotation.to_radians().sin_cos();
            let dir = (cos.round(), sin.round());
            let (back, forward) = extent_along(&bbox, anchor, dir);
            let what = format!("{label_type:?} @ {rotation}");

            // The label reaches well past the anchor in its direction...
            assert!(forward > 5.0, "{what}: forward extent {forward}");
            // ...and only a flag's arrow tip may sit behind the anchor.
            let max_back = if label::is_flag(&label) { 1.0 } else { 0.0 };
            assert!(back >= -max_back - 1e-9, "{what}: back extent {back}");
        }
    }
}

#[test]
fn flag_text_runs_away_from_the_connection_point() {
    for label_type in [LabelType::Global, LabelType::Hierarchical] {
        for rotation in [0.0_f64, 90.0, 180.0, 270.0] {
            let label = anchored_label(label_type, rotation);
            let anchor = (label.position.x, label.position.y);
            let (tx, ty) = label::flag_text_anchor(&label, None);
            let (sin, cos) = rotation.to_radians().sin_cos();
            let along = (tx - anchor.0) * cos.round() + (ty - anchor.1) * sin.round();
            assert!(
                along > 0.0,
                "{label_type:?} @ {rotation}: text start {along}"
            );
            // The text start sits inside the hit box, so click and draw agree.
            let bbox = label_aabb(&label, None);
            assert!(tx >= bbox.min_x && tx <= bbox.max_x && ty >= bbox.min_y && ty <= bbox.max_y);
        }
    }
}

/// Unit vector along which `h_align` text at `rotation_deg` runs from its
/// anchor: forward for left-aligned text, backward for right-aligned.
fn text_run(rotation_deg: f64, h_align: HAlign) -> (f64, f64) {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let sign = if h_align == HAlign::Right { -1.0 } else { 1.0 };
    ((sign * cos).round(), (sign * sin).round())
}

#[test]
fn flag_text_reads_left_to_right_or_bottom_to_top() {
    for label_type in [LabelType::Global, LabelType::Hierarchical] {
        for rotation in [0.0_f64, 90.0, 180.0, 270.0, -90.0, 450.0] {
            let label = anchored_label(label_type, rotation);
            let (_, text_rotation, _) = label::flag_text(&label, None);
            let upright = text_rotation.rem_euclid(360.0);
            assert!(
                upright.abs() < 1e-9 || (upright - 270.0).abs() < 1e-9,
                "{label_type:?} @ {rotation}: text at {upright}°"
            );
        }
    }
}

#[test]
fn readable_flag_text_still_covers_the_flag_body() {
    for label_type in [LabelType::Global, LabelType::Hierarchical] {
        for rotation in [0.0_f64, 90.0, 180.0, 270.0] {
            let label = anchored_label(label_type, rotation);
            let (anchor, text_rotation, h_align) = label::flag_text(&label, None);
            assert_eq!(anchor, label::flag_text_anchor(&label, None));
            let (sin, cos) = rotation.to_radians().sin_cos();
            assert_eq!(
                text_run(text_rotation, h_align),
                (cos.round(), sin.round()),
                "{label_type:?} @ {rotation}: the text runs along the flag, away from the tip"
            );
        }
    }
}

#[test]
fn canvas_draws_flag_text_with_the_readable_frame() {
    let mut sheet = empty_snapshot();
    sheet.labels.push(anchored_label(LabelType::Global, 180.0));
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
    };
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let snap = build_renderer_snapshot(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        Default::default(),
    );
    let text = snap
        .labels
        .iter()
        .find(|t| t.content == "DATA_BUS_0")
        .expect("flag text is drawn");
    assert!(text.rotation_rad.abs() < 1e-6, "upright, not upside down");
    assert_eq!(text.h_align, HAlign::Right);
}

#[test]
fn rotated_global_label_is_hit_on_its_flag_not_on_empty_space() {
    let mut snapshot = empty_snapshot();
    let label = anchored_label(LabelType::Global, 90.0);
    let uuid = label.uuid;
    snapshot.labels.push(label);
    // Rotated 90 degrees the flag runs down (+y) from (20, 30).
    let hit = hit_test::hit_test(&snapshot, &Default::default(), 20.0, 38.0);
    assert_eq!(hit, Some(SelectedItem::new(uuid, SelectedKind::Label)));
    assert_eq!(
        hit_test::hit_test(&snapshot, &Default::default(), 28.0, 30.0),
        None
    );
}

#[test]
fn selection_only_update_leaves_content_untouched() {
    let mut doc = empty_snapshot();
    let mut cache = SchematicRenderCache::from_sheet_at(&doc, 1);
    doc.wires.push(signex_types::schematic::Wire::default());
    // A selection change reaches the cache as NONE: no collection is re-cloned.
    cache.update_from_sheet(&doc, RenderInvalidation::NONE, 1, 2);
    assert!(cache.snapshot().wires.is_empty());
}

#[test]
fn document_edit_refreshes_only_the_edited_collection() {
    let mut doc = empty_snapshot();
    let mut cache = SchematicRenderCache::from_sheet_at(&doc, 1);
    doc.wires.push(signex_types::schematic::Wire::default());
    doc.paper_size = "A3".into();
    doc.frame.vertical_zones = 2;
    cache.update_from_sheet(&doc, RenderInvalidation::WIRES, 1, 2);
    assert_eq!(cache.snapshot().wires.len(), 1);
    assert_eq!(cache.snapshot().paper_size, "A4");
    cache.update_from_sheet(&doc, RenderInvalidation::PAPER, 2, 3);
    assert_eq!(cache.snapshot().paper_size, "A3");
    assert_eq!(cache.snapshot().frame.vertical_zones, 2);
}

#[test]
fn full_or_foreign_document_update_reclones_everything() {
    let mut doc = empty_snapshot();
    let mut cache = SchematicRenderCache::from_sheet_at(&doc, 1);
    doc.wires.push(signex_types::schematic::Wire::default());
    cache.update_from_sheet(&doc, RenderInvalidation::FULL, 1, 2);
    assert_eq!(cache.snapshot().wires.len(), 1);

    doc.uuid = uuid::Uuid::new_v4();
    doc.wires.push(signex_types::schematic::Wire::default());
    cache.update_from_sheet(&doc, RenderInvalidation::NONE, 2, 3);
    assert_eq!(cache.snapshot().wires.len(), 2);
}

#[test]
fn stale_snapshot_reclones_instead_of_partial_copy() {
    let mut doc = empty_snapshot();
    let mut cache = SchematicRenderCache::from_sheet_at(&doc, 1);
    // The engine moved to revision 2 without this cache being refreshed...
    doc.paper_size = "A3".into();
    // ...so the next edit (base 2 -> 3) must not trust a WIRES-only copy.
    doc.wires.push(signex_types::schematic::Wire::default());
    cache.update_from_sheet(&doc, RenderInvalidation::WIRES, 2, 3);
    assert_eq!(cache.snapshot().wires.len(), 1);
    assert_eq!(cache.snapshot().paper_size, "A3");
}

/// #1200 — a designator drawn longer than the stored `reference` (a
/// placement's R101 over R1) is hit, and selected by a rectangle, across the
/// whole drawn text, not only the stored one's width.
#[test]
fn a_drawn_designator_is_hit_across_its_drawn_width() {
    let mut snapshot = empty_snapshot();
    let id = uuid::Uuid::from_u128(7);
    let prop = signex_types::schematic::TextProp {
        position: Point::new(50.0, 50.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
        hidden: false,
    };
    snapshot.symbols.push(Symbol {
        uuid: id,
        reference: "R1".into(),
        position: Point::new(40.0, 40.0),
        ref_text: Some(prop.clone()),
        ..Default::default()
    });
    let symbol = snapshot.symbols[0].clone();
    let short = text_prop_aabb("R1", &prop, &symbol, None);
    let long = text_prop_aabb("R101", &prop, &symbol, None);
    assert!(long.max_x > short.max_x + 0.5, "{short:?} vs {long:?}");
    // Past the stored text's box (and its hit tolerance), inside the drawn one.
    let x = (short.max_x + 0.3 + long.max_x) / 2.0;
    let y = (long.min_y + long.max_y) / 2.0;
    let field = SelectedItem::new(id, SelectedKind::SymbolRefField);
    let drawn = Designators::from([(id, "R101".to_string())]);

    assert_eq!(hit_test::hit_test(&snapshot, &drawn, x, y), Some(field));
    assert_ne!(
        hit_test::hit_test(&snapshot, &Designators::new(), x, y),
        Some(field)
    );
    let around_long = Aabb::new(
        long.min_x - 0.1,
        long.min_y - 0.1,
        long.max_x + 0.1,
        long.max_y + 0.1,
    );
    let inside = hit_test::SelectionMode::Inside;
    assert!(hit_test::hit_test_rect_mode(&snapshot, &drawn, &around_long, inside).contains(&field));
    let around_short = Aabb::new(
        short.min_x - 0.1,
        short.min_y - 0.1,
        short.max_x + 0.1,
        short.max_y + 0.1,
    );
    assert!(
        !hit_test::hit_test_rect_mode(&snapshot, &drawn, &around_short, inside).contains(&field),
        "the drawn text no longer fits the stored text's box"
    );
}
