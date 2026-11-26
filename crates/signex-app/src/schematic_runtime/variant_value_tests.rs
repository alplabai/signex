//! #2948: the canvas scene draws, and boxes, a part's value as the active
//! variant gives it; with no variant value the sheet's value is drawn.

use iced::Rectangle;
use signex_output::DrawnValueMap;
use signex_types::schematic::{
    HAlign, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, TextProp, VAlign,
};
use signex_types::theme::{ThemeId, canvas_colors};

use super::{
    Designators, SceneOverrides, ScreenTransform, WireWidths, build_renderer_snapshot,
    selection::selection_bounds,
};

fn sheet() -> SchematicSheet {
    let text = TextProp {
        position: Point::new(20.0, 25.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        hidden: false,
    };
    SchematicSheet {
        symbols: vec![Symbol {
            uuid: uuid::Uuid::now_v7(),
            reference: "R1".to_owned(),
            value: "10k".to_owned(),
            position: Point::new(20.0, 20.0),
            val_text: Some(text),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn drawn_texts(sheet: &SchematicSheet, values: Option<&DrawnValueMap>) -> Vec<String> {
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
            variant_values: values,
            ..SceneOverrides::default()
        },
    );
    snap.reference_value_texts
        .iter()
        .map(|text| text.content.clone())
        .collect()
}

#[test]
fn the_scene_draws_the_variant_value_in_place_of_the_sheet_value() {
    let sheet = sheet();
    let values = DrawnValueMap::from([(sheet.symbols[0].uuid, "47k".to_string())]);
    let lite = drawn_texts(&sheet, Some(&values));
    assert!(lite.iter().any(|t| t == "47k"), "{lite:?}");
    assert!(!lite.iter().any(|t| t == "10k"), "{lite:?}");
    for none in [None, Some(&DrawnValueMap::new())] {
        let base = drawn_texts(&sheet, none);
        assert!(base.iter().any(|t| t == "10k"), "{base:?}");
    }
    // The stored value is the sheet's.
    assert_eq!(sheet.symbols[0].value, "10k");
}

/// The value field is boxed on the variant's longer value.
#[test]
fn the_value_box_measures_the_variant_value() {
    let sheet = sheet();
    let field = [SelectedItem::new(
        sheet.symbols[0].uuid,
        SelectedKind::SymbolValField,
    )];
    let width = |values: Option<&DrawnValueMap>| {
        let widths = WireWidths {
            variant_values: values,
            ..WireWidths::default()
        };
        let Some(bounds) = selection_bounds(&sheet, &field, &Designators::new(), widths) else {
            panic!("the value field has a box");
        };
        bounds.max_x - bounds.min_x
    };
    let values = DrawnValueMap::from([(sheet.symbols[0].uuid, "100.5k 1%".to_string())]);
    assert!(width(Some(&values)) > width(None));
}
