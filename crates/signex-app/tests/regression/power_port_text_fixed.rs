//! Maintainer decision 2026-11-07 (`docs/DECISIONS.md`): a power port's
//! net-name text is fixed to the port. A click on the text takes the whole
//! port, the halo covers body and text, the text follows every move, and a
//! stored custom offset from an older file is kept on disk but not drawn.

use crate::field_edit_support::{app_with, doc, empty_sheet};
use iced::{Point as ScreenPoint, Rectangle, Size};
use signex_app::app::{Message, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::schematic_runtime::selection::selection_bounds;
use signex_app::schematic_runtime::{
    ScreenTransform, WireWidths, build_renderer_snapshot, hit_test,
};
use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, power_port_value_field,
};

const AT: (f64, f64) = (50.8, 50.8);

fn vcc_port() -> Symbol {
    let at = Point::new(AT.0, AT.1);
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "power:VCC".into(),
        reference: "#PWR01".into(),
        value: "VCC".into(),
        position: at,
        unit: 1,
        is_power: true,
        val_text: Some(power_port_value_field("power:VCC", at)),
        ..Default::default()
    }
}

fn app_with_port(port: Symbol) -> Signex {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![port];
    app_with(sheet)
}

/// Where the port's net name is drawn on the canvas, world mm.
fn drawn_text_at(sheet: &SchematicSheet) -> [f32; 2] {
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 1.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
    let bounds = Rectangle::new(ScreenPoint::ORIGIN, Size::new(800.0, 600.0));
    let snap =
        build_renderer_snapshot(sheet, &transform, &colors, bounds, None, Default::default());
    snap.reference_value_texts
        .iter()
        .find(|t| t.content == "VCC")
        .expect("the net name is drawn")
        .position
}

fn near(a: [f32; 2], x: f64, y: f64) -> bool {
    (f64::from(a[0]) - x).abs() < 1e-3 && (f64::from(a[1]) - y).abs() < 1e-3
}

#[test]
fn a_click_on_the_net_name_takes_the_whole_port_and_the_halo_covers_it() {
    let app = app_with_port(vcc_port());
    let sheet = doc(&app);
    let port = SelectedItem::new(sheet.symbols[0].uuid, SelectedKind::Symbol);
    let [tx, ty] = drawn_text_at(sheet);
    let (tx, ty) = (f64::from(tx), f64::from(ty));
    // The text is bottom-aligned at its anchor above the bar.
    let on_text = (tx, ty - 0.5);
    assert_eq!(
        hit_test::hit_test(sheet, &Default::default(), on_text.0, on_text.1),
        Some(port)
    );
    let halo = selection_bounds(sheet, &[port], &Default::default(), WireWidths::default())
        .expect("the port is boxed");
    assert!(
        halo.contains(on_text.0, on_text.1),
        "{halo:?} misses the text"
    );
    assert!(
        halo.contains(AT.0, AT.1),
        "{halo:?} misses the connection point"
    );
}

#[test]
fn a_stored_text_offset_loads_and_is_kept_on_disk_but_is_not_drawn() {
    let mut legacy = vcc_port();
    let custom = Point::new(AT.0 + 20.32, AT.1 - 10.16);
    legacy.val_text.as_mut().expect("value field").position = custom;
    let mut sheet = empty_sheet();
    sheet.symbols = vec![legacy];
    let text = SnxSchematic::new(sheet).write_string().expect("writes");
    let loaded = SnxSchematic::parse(&text).expect("loads").sheet;
    assert_eq!(
        loaded.symbols[0].val_text.as_ref().map(|t| t.position),
        Some(custom),
        "the stored offset round-trips"
    );
    let rewritten = SnxSchematic::new(loaded.clone())
        .write_string()
        .expect("writes");
    assert_eq!(rewritten, text, "loading changes nothing on disk");

    let fixed = power_port_value_field("power:VCC", Point::new(AT.0, AT.1)).position;
    assert!(near(drawn_text_at(&loaded), fixed.x, fixed.y));
    assert_ne!(
        hit_test::hit_test(&loaded, &Default::default(), custom.x, custom.y - 0.5),
        Some(SelectedItem::new(
            loaded.symbols[0].uuid,
            SelectedKind::SymbolValField
        )),
        "the stored spot is no longer a separate text"
    );
}

#[test]
fn a_selected_net_name_drags_the_port_and_the_text_follows() {
    let mut app = app_with_port(vcc_port());
    app.ui_state.snap_enabled = false;
    let id = doc(&app).symbols[0].uuid;
    let before = drawn_text_at(doc(&app));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::SymbolValField)];
    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 5.08,
        dy: 2.54,
        connections: Default::default(),
    }));
    let sheet = doc(&app);
    assert_eq!(
        sheet.symbols[0].position,
        Point::new(AT.0 + 5.08, AT.1 + 2.54)
    );
    let after = drawn_text_at(sheet);
    assert!(near(
        after,
        f64::from(before[0]) + 5.08,
        f64::from(before[1]) + 2.54
    ));
}

#[test]
fn a_turned_port_draws_its_text_past_the_turned_head() {
    let mut port = vcc_port();
    port.rotation = 90.0;
    let app = app_with_port(port);
    // Turned a quarter counter-clockwise, the head points left.
    let [tx, ty] = drawn_text_at(doc(&app));
    assert!(f64::from(tx) < AT.0 - 2.54, "text at x {tx}");
    assert!((f64::from(ty) - AT.1).abs() < 1e-3, "text at y {ty}");
}

#[test]
fn a_double_click_on_the_port_edits_its_net_name() {
    let mut app = app_with_port(vcc_port());
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: AT.0,
        world_y: AT.1 - 1.27,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
    let editing = app
        .interaction_state
        .editing_text
        .as_ref()
        .map(|e| (e.kind, e.text.clone()));
    assert_eq!(editing, Some((SelectedKind::SymbolValField, "VCC".into())));
}
