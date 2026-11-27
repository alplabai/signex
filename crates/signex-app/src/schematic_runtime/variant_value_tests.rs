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

/// The colour of the scene text `content`, the sheet drawn with `colors`
/// and the variant `values`, on the CPU replay's scene.
fn cpu_text_color(
    sheet: &SchematicSheet,
    colors: &signex_types::theme::CanvasColors,
    values: Option<&DrawnValueMap>,
    content: &str,
) -> [f32; 4] {
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let scene = super::build_schematic_scene(
        sheet,
        &transform,
        colors,
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0)),
        None,
        SceneOverrides {
            variant_values: values,
            ..SceneOverrides::default()
        },
    )
    .expect("the sheet draws");
    scene
        .texts
        .iter()
        .find(|text| text.content == content)
        .unwrap_or_else(|| panic!("{content} is drawn"))
        .color
}

/// Mockup row 209: a value the active variant gives the part draws in the
/// accent (the theme's selection colour) on the CPU replay; the sheet's own
/// value keeps the value colour. Both themes.
#[test]
fn the_cpu_scene_draws_a_variant_value_in_the_accent() {
    let sheet = sheet();
    let values = DrawnValueMap::from([(sheet.symbols[0].uuid, "47k".to_string())]);
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let colors = canvas_colors(theme);
        // The same text drawn in the value colour set to the accent.
        let accented = signex_types::theme::CanvasColors {
            value: colors.selection,
            ..colors
        };
        let expected = cpu_text_color(&sheet, &accented, None, "10k");
        let own = cpu_text_color(&sheet, &colors, None, "10k");
        assert_ne!(expected, own, "{theme:?}: the accent is not the value ink");
        assert_eq!(
            cpu_text_color(&sheet, &colors, Some(&values), "47k"),
            expected,
            "{theme:?}"
        );
        // A variant value equal to the sheet's is not the variant's own.
        let same = DrawnValueMap::from([(sheet.symbols[0].uuid, "10k".to_string())]);
        assert_eq!(cpu_text_color(&sheet, &colors, Some(&same), "10k"), own);
    }
}

/// The texts the GPU path uploads, recorded instead of sent to a device.
#[derive(Default)]
struct RecordedTexts(Vec<signex_gfx::primitive::text::TextItem>);

impl signex_gfx::scene::SceneUploadTarget for RecordedTexts {
    type TextError = std::convert::Infallible;

    fn upload_lines(&mut self, _: &[signex_gfx::primitive::line::LineSegment]) {}
    fn upload_circles(&mut self, _: &[signex_gfx::primitive::circle::Circle]) {}
    fn upload_arcs(&mut self, _: &[signex_gfx::primitive::arc::Arc]) {}
    fn upload_polygons(&mut self, _: &[signex_gfx::primitive::polygon::GpuPolygon]) {}
    fn upload_top_polygons(&mut self, _: &[signex_gfx::primitive::polygon::GpuPolygon]) {}
    fn upload_drawings(&mut self, _: &[signex_gfx::primitive::ordered::OrderedItem]) {}
    fn upload_drawings_front(&mut self, _: &[signex_gfx::primitive::ordered::OrderedItem]) {}
    fn upload_texts(
        &mut self,
        texts: &[signex_gfx::primitive::text::TextItem],
        _: signex_gfx::scene::TextUploadParams,
    ) -> Result<(), Self::TextError> {
        self.0 = texts.to_vec();
        Ok(())
    }
    fn refresh_grid(&mut self) {}
    fn upload_overlay_lines(&mut self, _: &[signex_gfx::primitive::line::LineSegment]) {}
    fn upload_overlay_circles(&mut self, _: &[signex_gfx::primitive::circle::Circle]) {}
    fn upload_overlay_polygons(&mut self, _: &[signex_gfx::primitive::polygon::GpuPolygon]) {}
    fn upload_erc_marker_lines(&mut self, _: &[signex_gfx::primitive::line::LineSegment]) {}
    fn upload_erc_marker_circles(&mut self, _: &[signex_gfx::primitive::circle::Circle]) {}
    fn upload_erc_marker_polygons(&mut self, _: &[signex_gfx::primitive::polygon::GpuPolygon]) {}
    fn refresh_theme(&mut self) {}
}

/// Mockup row 209 on the GPU path: the content scene the shader takes from
/// the shared scene cache uploads the variant value's text in the accent.
#[test]
fn the_gpu_upload_carries_a_variant_value_in_the_accent() {
    let sheet = sheet();
    let values = DrawnValueMap::from([(sheet.symbols[0].uuid, "47k".to_string())]);
    let colors = canvas_colors(ThemeId::GraphiteDark);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0));
    let mut cache = super::ContentSceneCache::default();
    let scene = cache
        .scene_for(
            &sheet,
            &transform,
            &colors,
            bounds,
            None,
            SceneOverrides {
                variant_values: Some(&values),
                ..SceneOverrides::default()
            },
        )
        .cloned()
        .expect("the sheet draws");
    let mut target = RecordedTexts::default();
    let uploaded = signex_gfx::scene::apply_dirty_uploads(
        &scene,
        signex_gfx::scene::DirtyFlags::TEXT,
        &mut target,
        signex_gfx::scene::TextUploadParams::new(4.0, [2000, 2000]),
    );
    assert!(uploaded.is_ok());
    let value = target
        .0
        .iter()
        .find(|text| text.content == "47k")
        .expect("the variant value is uploaded");
    let accented = signex_types::theme::CanvasColors {
        value: colors.selection,
        ..colors
    };
    assert_eq!(
        value.color,
        cpu_text_color(&sheet, &accented, None, "10k"),
        "the GPU upload and the CPU replay draw the same accent"
    );
}
