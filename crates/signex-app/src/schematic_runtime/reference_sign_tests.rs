//! A reference draws as the project's designator standard shows it (#1416).

use iced::Rectangle;
use signex_types::designator::DesignatorStandard;
use signex_types::schematic::{Point, SchematicSheet, Symbol};
use signex_types::theme::{ThemeId, canvas_colors};

use super::{SceneOverrides, ScreenTransform, build_renderer_snapshot};

fn sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    for (reference, x) in [("QAC1", 20.0), ("RAC2", 60.0)] {
        sheet.symbols.push(Symbol {
            uuid: uuid::Uuid::now_v7(),
            reference: reference.to_owned(),
            position: Point::new(x, 20.0),
            ..Default::default()
        });
    }
    sheet
}

fn drawn_references(sheet: &SchematicSheet, standard: Option<DesignatorStandard>) -> Vec<String> {
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let snap = build_renderer_snapshot(
        sheet,
        &transform,
        &canvas_colors(ThemeId::GraphiteLight),
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0)),
        None,
        SceneOverrides {
            designator_standard: standard,
            ..SceneOverrides::default()
        },
    );
    snap.reference_value_texts
        .iter()
        .map(|text| text.content.clone())
        .collect()
}

#[test]
fn full_class_codes_draw_with_the_product_aspect_sign_and_stay_stored_bare() {
    let sheet = sheet();
    assert_eq!(
        drawn_references(&sheet, Some(DesignatorStandard::Iec81346_2Full)),
        ["-QAC1", "-RAC2"]
    );
    assert_eq!(sheet.symbols[0].reference, "QAC1");
    for standard in [None, Some(DesignatorStandard::Iec81346_2)] {
        assert_eq!(drawn_references(&sheet, standard), ["QAC1", "RAC2"]);
    }
}
