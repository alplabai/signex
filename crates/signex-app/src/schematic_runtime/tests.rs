use super::*;

fn empty_snapshot() -> SchematicRenderSnapshot {
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
        lib_symbols: HashMap::new(),
    }
}

#[test]
fn hit_test_wire_uses_segment_tolerance() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    snapshot.wires.push(signex_types::schematic::Wire {
        unknown_columns: Default::default(),
        uuid: wire_uuid,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        stroke_width: 0.15,
    });

    let hit = hit_test::hit_test(&snapshot, 5.0, 0.08);
    assert_eq!(hit, Some(SelectedItem::new(wire_uuid, SelectedKind::Wire)));
}

#[test]
fn hit_test_rect_mode_distinguishes_inside_and_touching() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    snapshot.wires.push(signex_types::schematic::Wire {
        unknown_columns: Default::default(),
        uuid: wire_uuid,
        start: Point::new(-4.0, 0.0),
        end: Point::new(4.0, 0.0),
        stroke_width: 0.15,
    });

    let rect = Aabb::new(0.0, -0.2, 2.0, 0.2);
    let inside = hit_test::hit_test_rect_mode(&snapshot, &rect, hit_test::SelectionMode::Inside);
    let touching =
        hit_test::hit_test_rect_mode(&snapshot, &rect, hit_test::SelectionMode::Touching);

    assert!(!inside.contains(&SelectedItem::new(wire_uuid, SelectedKind::Wire)));
    assert!(touching.contains(&SelectedItem::new(wire_uuid, SelectedKind::Wire)));
}

#[test]
fn hit_test_polygon_selects_wire_and_label_by_anchor() {
    let mut snapshot = empty_snapshot();
    let wire_uuid = uuid::Uuid::new_v4();
    let label_uuid = uuid::Uuid::new_v4();

    snapshot.wires.push(signex_types::schematic::Wire {
        unknown_columns: Default::default(),
        uuid: wire_uuid,
        start: Point::new(1.0, 1.0),
        end: Point::new(9.0, 1.0),
        stroke_width: 0.15,
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
    };
    let symbol = field_symbol(Some(prop));

    let drawn = snapshot::field_text_input(
        &symbol,
        "R1",
        symbol.ref_text.as_ref(),
        snapshot::FieldSlot::Reference,
        [1.0; 4],
    )
    .expect("visible field is drawn");
    assert_eq!(drawn.position, [70.0, 30.0]);
    assert_eq!(drawn.size_mm, 2.0);
    assert_eq!(drawn.rotation_rad, 0.0);
    assert_eq!(drawn.h_align, HAlign::Left);

    let mut snapshot = empty_snapshot();
    let uuid = symbol.uuid;
    snapshot.symbols.push(symbol);
    let hit = hit_test::hit_test(&snapshot, 70.0, 30.0);
    assert_eq!(
        hit,
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
    };
    assert!(
        snapshot::field_text_input(
            &symbol,
            "R1",
            Some(&hidden),
            snapshot::FieldSlot::Reference,
            [1.0; 4],
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
