use super::*;
use signex_types::schematic::{Graphic, LibGraphic, LibSymbol};

/// Sheet with one symbol placed at `(x, y)` whose library body is a
/// `w` x `h` mm rectangle centred on the origin.
fn sheet_with_body(w: f64, h: f64, x: f64, y: f64, rotation: f64) -> (SchematicSheet, Symbol) {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:Body" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-w / 2.0, -h / 2.0),
            end: Point::new(w / 2.0, h / 2.0),
            width: 0.0,
            fill: FillType::Background,
        },
    });
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: lib.id.clone(),
        position: Point::new(x, y),
        rotation,
        ..Symbol::default()
    };
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert(lib.id.clone(), lib);
    sheet.symbols.push(symbol.clone());
    (sheet, symbol)
}

fn approx(b: Aabb, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> bool {
    [
        (b.min_x, min_x),
        (b.min_y, min_y),
        (b.max_x, max_x),
        (b.max_y, max_y),
    ]
    .iter()
    .all(|(got, want)| (got - want).abs() < 1e-6)
}

#[test]
fn large_and_small_symbols_get_distinct_boxes() {
    let (ic_sheet, ic) = sheet_with_body(20.0, 40.0, 50.0, 50.0, 0.0);
    let (r_sheet, r) = sheet_with_body(1.6, 0.8, 50.0, 50.0, 0.0);
    let ic_box = symbol_bounds(&ic_sheet, &ic);
    let r_box = symbol_bounds(&r_sheet, &r);
    assert!(approx(ic_box, 40.0, 30.0, 60.0, 70.0), "{ic_box:?}");
    assert!(approx(r_box, 49.2, 49.6, 50.8, 50.4), "{r_box:?}");
}

#[test]
fn rotated_symbol_box_is_transformed() {
    let (sheet, sym) = sheet_with_body(4.0, 20.0, 0.0, 0.0, 90.0);
    let bbox = symbol_bounds(&sheet, &sym);
    assert!(approx(bbox, -10.0, -2.0, 10.0, 2.0), "{bbox:?}");
}

#[test]
fn click_inside_the_real_body_selects_and_outside_does_not() {
    let (sheet, sym) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    let hit = SelectedItem::new(sym.uuid, SelectedKind::Symbol);
    // 15 mm above the origin: outside the old 8 x 5.6 mm placeholder box,
    // inside the real 20 x 40 mm body.
    assert_eq!(
        hit_test::hit_test(&sheet, &Default::default(), 0.0, 15.0),
        Some(hit)
    );
    assert_eq!(
        hit_test::hit_test(&sheet, &Default::default(), 0.0, 25.0),
        None
    );
}

#[test]
fn small_body_is_not_hit_where_the_placeholder_box_was() {
    let (sheet, _) = sheet_with_body(1.6, 0.8, 0.0, 0.0, 0.0);
    assert_eq!(
        hit_test::hit_test(&sheet, &Default::default(), 3.0, 0.0),
        None
    );
}

#[test]
fn missing_library_symbol_falls_back_to_the_placeholder_box() {
    let (mut sheet, sym) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    sheet.lib_symbols.clear();
    let (primitives, bbox) = symbol_geometry(&sheet, &sym);
    assert!(primitives.is_empty());
    assert!(approx(bbox, -4.0, -2.8, 4.0, 2.8), "{bbox:?}");
}

#[test]
fn symbols_without_geometry_lists_each_missing_library_id_once() {
    let (mut sheet, sym) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    assert!(symbols_without_geometry(&sheet).is_empty());
    let mut twin = sym.clone();
    twin.uuid = uuid::Uuid::new_v4();
    sheet.symbols.push(twin);
    sheet.lib_symbols.clear();
    assert_eq!(symbols_without_geometry(&sheet), vec!["L:Body".to_string()]);
}

/// The box of one item, through the batch entry point the overlay uses.
fn item_aabb(sheet: &SchematicRenderSnapshot, item: &SelectedItem) -> Option<Aabb> {
    selected_aabbs(sheet, &[*item], &Default::default())[0]
}

#[test]
fn selected_symbol_box_matches_its_body_without_lowering_the_sheet() {
    let (sheet, sym) = sheet_with_body(20.0, 40.0, 5.0, 6.0, 0.0);
    let item = SelectedItem::new(sym.uuid, SelectedKind::Symbol);
    let bbox = item_aabb(&sheet, &item).expect("symbol box");
    assert!(approx(bbox, -5.0, -14.0, 15.0, 26.0), "{bbox:?}");
    let missing = SelectedItem::new(uuid::Uuid::new_v4(), SelectedKind::Symbol);
    assert!(item_aabb(&sheet, &missing).is_none());
}

#[test]
fn symbol_fields_are_still_found_without_symbol_bodies() {
    let (mut sheet, sym) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    sheet.symbols[0].reference = "U1".into();
    sheet.symbols[0].ref_text = Some(TextProp {
        position: Point::new(0.0, -30.0),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        hidden: false,
        font_family: None,
    });
    let item = SelectedItem::new(sym.uuid, SelectedKind::SymbolRefField);
    assert!(item_aabb(&sheet, &item).is_some());
}

#[test]
fn filled_library_arc_draws_a_filled_polygon() {
    let (mut sheet, _) = sheet_with_body(2.0, 2.0, 0.0, 0.0, 0.0);
    sheet.lib_symbols.get_mut("L:Body").unwrap().graphics[0].graphic = Graphic::Arc {
        start: Point::new(-5.0, 0.0),
        mid: Point::new(0.0, 5.0),
        end: Point::new(5.0, 0.0),
        width: 0.0,
        fill: FillType::Background,
    };
    let transform = ScreenTransform {
        offset_x: 400.0,
        offset_y: 400.0,
        scale: 10.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 800.0));
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let snap = build_renderer_snapshot(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        Default::default(),
    );
    assert!(
        snap.polygons.iter().any(|p| p.fill_color[3] > 0.0),
        "a filled arc must reach the renderer as a filled polygon"
    );
}

#[test]
fn snapshot_draws_the_library_body_not_the_placeholder() {
    let (sheet, _) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    let transform = ScreenTransform {
        offset_x: 400.0,
        offset_y: 400.0,
        scale: 10.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 800.0));
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let snap = build_renderer_snapshot(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        Default::default(),
    );
    let body = snap.polygons.first().expect("symbol body polygon");
    let span = |axis: usize| {
        let v = body.vertices.iter().map(|p| p[axis]);
        v.clone().fold(f32::MIN, f32::max) - v.fold(f32::MAX, f32::min)
    };
    assert!((span(0) - 20.0).abs() < 1e-3, "width {}", span(0));
    assert!((span(1) - 40.0).abs() < 1e-3, "height {}", span(1));
}

#[test]
fn snapshot_marks_a_missing_library_entry_with_its_lib_id() {
    let (mut sheet, _) = sheet_with_body(20.0, 40.0, 0.0, 0.0, 0.0);
    sheet.lib_symbols.clear();
    let transform = ScreenTransform {
        offset_x: 400.0,
        offset_y: 400.0,
        scale: 10.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 800.0));
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let snap = build_renderer_snapshot(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        Default::default(),
    );
    let label = snap
        .parameter_texts
        .iter()
        .find(|t| t.content == "L:Body")
        .expect("placeholder labelled with the missing lib_id");
    assert_eq!(label.color, to_rgba(to_iced(&colors.no_connect)));
    assert_eq!(snap.wires.len(), 2, "the box is crossed out");
    let body = snap.polygons.first().expect("placeholder box");
    assert_eq!(
        body.stroke_color,
        Some(to_rgba(to_iced(&colors.no_connect))),
        "drawn in the missing colour, not the body colour"
    );
}

fn placed_power_port(lib_id: &str) -> (SchematicSheet, Symbol) {
    let symbol = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: lib_id.to_string(),
        position: Point::new(50.0, 50.0),
        is_power: true,
        ..Symbol::default()
    };
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(symbol.clone());
    (sheet, symbol)
}

/// The Active Bar places a power port with no library entry. It must still be
/// drawn and hit-tested as its own shape, not as the 8 x 5.6 mm placeholder.
#[test]
fn a_power_port_without_a_library_entry_gets_its_own_body_not_the_box() {
    let (sheet, sym) = placed_power_port("power:GND");
    let (primitives, bbox) = symbol_geometry(&sheet, &sym);
    assert!(!primitives.is_empty(), "no body lowered");
    assert!(bbox.width() < 3.0 && bbox.height() < 4.0, "{bbox:?}");
    assert!(
        symbols_without_geometry(&sheet).is_empty(),
        "reported as a placeholder"
    );
    // Ground hangs below the connection point (world Y-down).
    assert!(bbox.max_y > 50.0 && bbox.min_y <= 50.0, "{bbox:?}");
}

#[test]
fn a_bar_power_port_rises_above_the_connection_point() {
    let (sheet, sym) = placed_power_port("power:bar");
    let bbox = symbol_bounds(&sheet, &sym);
    assert!(bbox.min_y < 50.0 && bbox.max_y >= 50.0, "{bbox:?}");
    assert!(
        bbox.width() < 3.0 && bbox.height() < 4.0,
        "placeholder box: {bbox:?}"
    );
}

#[test]
fn the_power_port_placement_ghost_draws_the_same_geometry_as_the_placed_port() {
    let (_, sym) = placed_power_port("power:arrow");
    let (ghost, ghost_box) = symbol_geometry_of(None, &sym);
    let mut with_sym = SchematicSheet::default();
    with_sym.symbols.push(sym.clone());
    let (placed, placed_box) = symbol_geometry(&with_sym, &sym);
    assert!(!ghost.is_empty(), "ghost drew no body");
    assert_eq!(ghost, placed);
    assert!(approx(
        ghost_box,
        placed_box.min_x,
        placed_box.min_y,
        placed_box.max_x,
        placed_box.max_y
    ));
}

#[test]
fn a_non_power_symbol_without_a_library_entry_still_gets_the_placeholder() {
    let sym = Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "missing".into(),
        position: Point::new(10.0, 10.0),
        ..Symbol::default()
    };
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(sym.clone());
    assert!(approx(symbol_bounds(&sheet, &sym), 6.0, 7.2, 14.0, 12.8));
}
