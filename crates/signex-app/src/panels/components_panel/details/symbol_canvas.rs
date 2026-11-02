//! The details pane's Symbol preview: the symbol as the schematic canvas
//! draws it placed on a sheet, through the same lowering, scene and replay,
//! on the active theme's paper colour.

use iced::widget::canvas::{self, Geometry};
use iced::{Element, Length, Rectangle, Size, Theme};
use signex_types::schematic::{Aabb, LibSymbol, SchematicSheet, Symbol};
use signex_types::theme::CanvasColors;

use crate::render_config::to_iced;
use crate::schematic_runtime::{
    SceneOverrides, ScreenTransform, build_schematic_scene, draw_built_scene, symbol_bounds,
};

/// Share of the box the symbol's extent fills; the rest is margin.
const FILL_RATIO: f32 = 0.8;

struct SymbolSheetPreview {
    sheet: SchematicSheet,
    symbol: Symbol,
    colors: CanvasColors,
}

/// A sheet holding `lib` placed once at the origin, unit 1, normal body.
fn preview_sheet(lib: &LibSymbol) -> (SchematicSheet, Symbol) {
    let symbol = Symbol {
        uuid: uuid::Uuid::nil(),
        lib_id: lib.id.clone(),
        ..Symbol::default()
    };
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert(lib.id.clone(), lib.clone());
    sheet.symbols.push(symbol.clone());
    (sheet, symbol)
}

/// The transform that centres `extent` in `size` and scales it to fit.
fn fit_transform(extent: Aabb, size: Size) -> ScreenTransform {
    let width = (extent.max_x - extent.min_x).max(f64::EPSILON) as f32;
    let height = (extent.max_y - extent.min_y).max(f64::EPSILON) as f32;
    let scale = (size.width / width).min(size.height / height) * FILL_RATIO;
    let mid_x = ((extent.min_x + extent.max_x) / 2.0) as f32;
    let mid_y = ((extent.min_y + extent.max_y) / 2.0) as f32;
    ScreenTransform {
        offset_x: size.width / 2.0 - mid_x * scale,
        offset_y: size.height / 2.0 - mid_y * scale,
        scale,
        device_px_per_logical_px: 1.0,
    }
}

impl canvas::Program<()> for SymbolSheetPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(
            iced::Point::ORIGIN,
            bounds.size(),
            to_iced(&self.colors.paper),
        );
        if bounds.width > 0.0 && bounds.height > 0.0 {
            let transform = fit_transform(symbol_bounds(&self.sheet, &self.symbol), bounds.size());
            if let Some(scene) = build_schematic_scene(
                &self.sheet,
                &transform,
                &self.colors,
                Rectangle::with_size(bounds.size()),
                None,
                SceneOverrides::default(),
            ) {
                draw_built_scene(&mut frame, &scene, &transform);
            }
        }
        vec![frame.into_geometry()]
    }
}

/// The Symbol preview of `lib`, `height` tall and as wide as its parent.
pub(super) fn symbol_preview(
    lib: &LibSymbol,
    colors: CanvasColors,
    height: f32,
) -> Element<'static, ()> {
    let (sheet, symbol) = preview_sheet(lib);
    canvas::Canvas::new(SymbolSheetPreview {
        sheet,
        symbol,
        colors,
    })
    .width(Length::Fill)
    .height(height)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{FillType, Graphic, LibGraphic, Point};
    use signex_types::theme::{ThemeId, canvas_colors};

    fn body_symbol() -> LibSymbol {
        let mut lib: LibSymbol =
            serde_json::from_value(serde_json::json!({ "id": "L:Body" })).expect("lib symbol");
        lib.graphics.push(LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(-5.0, -10.0),
                end: Point::new(5.0, 10.0),
                width: 0.0,
                fill: FillType::Background,
            },
        });
        lib
    }

    #[test]
    fn the_preview_is_a_placed_symbol_the_canvas_can_draw() {
        let (sheet, symbol) = preview_sheet(&body_symbol());
        let size = Size::new(200.0, 100.0);
        let transform = fit_transform(symbol_bounds(&sheet, &symbol), size);
        let scene = build_schematic_scene(
            &sheet,
            &transform,
            &canvas_colors(ThemeId::Signex),
            Rectangle::with_size(size),
            None,
            SceneOverrides::default(),
        );
        assert!(scene.is_some(), "a body must lower to drawn primitives");
    }

    #[test]
    fn the_symbol_is_centred_and_fits_inside_the_box() {
        let (sheet, symbol) = preview_sheet(&body_symbol());
        let size = Size::new(200.0, 100.0);
        let extent = symbol_bounds(&sheet, &symbol);
        let transform = fit_transform(extent, size);
        let low = transform.world_to_screen((extent.min_x, extent.min_y));
        let high = transform.world_to_screen((extent.max_x, extent.max_y));
        assert!(low.x >= 0.0 && low.y >= 0.0, "{low:?}");
        assert!(high.x <= size.width && high.y <= size.height, "{high:?}");
        assert!(
            (low.x + high.x - size.width).abs() < 1e-3,
            "{low:?} {high:?}"
        );
        assert!(
            (low.y + high.y - size.height).abs() < 1e-3,
            "{low:?} {high:?}"
        );
        // The 20 mm tall body is the limiting side.
        assert!((high.y - low.y - size.height * FILL_RATIO).abs() < 1e-3);
    }
}
