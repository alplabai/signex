//! #1961 — a placed stock IC draws its pin numbers and pin names. The sheet
//! renderer used to emit only the body and bare pin lines.
//!
//! Real `Stock.snxlib`, the real place message, then the same renderer
//! snapshot the canvas builds.

use std::path::PathBuf;

use iced::{Point, Rectangle, Size};
use signex_app::app::{Message, Signex};
use signex_app::library::LibraryMessage;
use signex_app::schematic_runtime::{ScreenTransform, build_renderer_snapshot};
use signex_types::schematic::{HAlign, VAlign};
use signex_types::theme::{ThemeId, canvas_colors};

use crate::library_place_symbol::{PLACE_AT, app_with_empty_sheet, click, doc, run};

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn app_with_placed_stock_ic() -> Signex {
    let mut app = app_with_empty_sheet();
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    let (_, ic) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == "STOCK-IC_Generic")
        .expect("Stock lists the generic IC");
    let table = app
        .library
        .table_of_row(&stock_file(), ic.row_id)
        .expect("the IC's table");
    let mut pending = vec![Message::Library(LibraryMessage::ComponentsPanelPlace {
        library_path: stock_file(),
        table,
        row_id: signex_library::RowId(ic.row_id),
    })];
    while let Some(next) = pending.pop() {
        let task = app.update(next);
        pending.extend(run(task));
    }
    click(&mut app, PLACE_AT);
    app
}

#[test]
fn a_placed_stock_ic_renders_its_pin_numbers_and_names() {
    let app = app_with_placed_stock_ic();
    let sheet = doc(&app);
    let sym = &sheet.symbols[0];
    let lib = sheet.body_of(sym).expect("embedded body");
    assert_eq!(lib.pins.len(), 8, "the generic IC has 8 pins");

    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 10.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(4000.0, 4000.0));
    let colors = canvas_colors(ThemeId::GraphiteDark);
    let snap =
        build_renderer_snapshot(sheet, &transform, &colors, bounds, None, Default::default());

    assert_eq!(snap.pin_texts.len(), 16, "8 numbers and 8 names");
    assert_eq!(
        (sym.rotation, sym.mirror_x, sym.mirror_y),
        (0.0, false, false)
    );
    let rgba = |c: &signex_types::theme::Color| [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0);
    let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let (sx, sy) = (sym.position.x as f32, sym.position.y as f32);
    let (reach, along) = (2.54 + 0.508, 2.54 * 0.18);

    for lp in &lib.pins {
        let pin = &lp.pin;
        // Pins left of the body point right (into it), those right of it
        // point left. Library y is up, the sheet's is down.
        let dir: f32 = if pin.position.x < 0.0 { 1.0 } else { -1.0 };
        let tip = (sx + pin.position.x as f32, sy - pin.position.y as f32);
        let find = |content: &str| {
            let hits: Vec<_> = snap
                .pin_texts
                .iter()
                .filter(|t| t.content == content)
                .collect();
            assert_eq!(hits.len(), 1, "exactly one text {content:?}");
            hits[0]
        };

        let name = find(&pin.name);
        assert!(near(name.position[0], tip.0 + dir * reach), "{}", pin.name);
        assert!(near(name.position[1], tip.1), "{}", pin.name);
        assert!(near(name.rotation_rad, 0.0));
        assert_eq!(
            name.h_align,
            if dir > 0.0 {
                HAlign::Left
            } else {
                HAlign::Right
            }
        );
        assert_eq!(name.v_align, VAlign::Center);
        assert!(
            rgba(&colors.pin)
                .iter()
                .zip(name.color)
                .all(|(a, b)| near(*a, b))
        );

        let number = find(&pin.number);
        assert!(
            near(number.position[0], tip.0 + dir * along),
            "{}",
            pin.number
        );
        assert!(near(number.position[1], tip.1 - 0.254), "{}", pin.number);
        assert!(near(number.rotation_rad, 0.0));
        assert_eq!(number.h_align, HAlign::Center);
        assert_eq!(number.v_align, VAlign::Bottom);
        assert!(
            rgba(&colors.reference)
                .iter()
                .zip(number.color)
                .all(|(a, b)| near(*a, b))
        );
    }
}
