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
use signex_types::schematic::{PinLabelKind, lower_pin_labels};
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
    let lib = sheet.lib_symbols.get(&sym.lib_id).expect("embedded body");
    assert_eq!(lib.pins.len(), 8, "the generic IC has 8 pins");

    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 10.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(4000.0, 4000.0));
    let colors = canvas_colors(ThemeId::CatppuccinMocha);
    let snap =
        build_renderer_snapshot(sheet, &transform, &colors, bounds, None, Default::default());

    let drawn: Vec<&str> = snap.pin_texts.iter().map(|t| t.content.as_str()).collect();
    for pin in lib.pins.iter().map(|p| &p.pin) {
        assert!(
            drawn.contains(&pin.number.as_str()),
            "number {}",
            pin.number
        );
        assert!(drawn.contains(&pin.name.as_str()), "name {}", pin.name);
    }
    assert_eq!(snap.pin_texts.len(), 16);

    // Names sit inside the body (pin ends at x = +-5.08 from the origin),
    // numbers outside it, beside the pin line.
    for label in lower_pin_labels(lib, sym) {
        let x = (label.position.x - sym.position.x).abs();
        match label.kind {
            PinLabelKind::Name => assert!(x < 5.08, "name {} is inside the body", label.text),
            PinLabelKind::Number => {
                assert!(x > 5.08, "number {} is outside the body", label.text)
            }
        }
    }
}
