use super::*;

fn empty_snapshot() -> SchematicRenderSnapshot {
    SchematicRenderSnapshot {
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
        uuid: wire_uuid,
        start: Point::new(1.0, 1.0),
        end: Point::new(9.0, 1.0),
        stroke_width: 0.15,
    });
    snapshot.labels.push(Label {
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
