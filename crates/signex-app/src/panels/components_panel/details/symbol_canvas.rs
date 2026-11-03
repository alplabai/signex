//! The details pane's Symbol preview: the symbol as the schematic canvas
//! draws it placed on a sheet, through the same lowering, scene and replay,
//! on the active theme's paper colour.

use std::cell::RefCell;

use iced::widget::canvas::{self, Geometry};
use iced::widget::container;
use iced::{Element, Length, Rectangle, Size, Theme};
use signex_types::schematic::{
    Aabb, LibSymbol, SchematicSheet, Symbol, UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
};
use signex_types::theme::CanvasColors;

use crate::render_config::to_iced;
use crate::schematic_runtime::{
    SCHEMATIC_TEXT_SIZE, SceneOverrides, ScreenTransform, build_schematic_scene, draw_built_scene,
    symbol_bounds,
};

/// Share of the box the symbol's extent fills; the rest is margin.
const FILL_RATIO: f32 = 0.8;
/// Pin and field text is drawn outside the extent; the margin is at least
/// this many times the minimum text size so it stays inside the box.
const TEXT_MARGIN_EM: f32 = 2.0;
/// Default schematic text height, mm; sets the largest sheet-like zoom.
const TEXT_HEIGHT_MM: f32 = 1.27;
/// The most a small part is magnified, in text heights at the max text size.
const MAX_SCALE_TEXT_HEIGHTS: f32 = 4.0;

/// What the cached scene was built from; a change drops the cache.
type Shown = (LibSymbol, CanvasColors);

#[derive(Default)]
struct PreviewState {
    cache: canvas::Cache,
    shown: RefCell<Option<Shown>>,
}

struct SymbolSheetPreview {
    lib: LibSymbol,
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

/// The margin kept free on each side of the extent along one axis, px.
fn fit_margin(side: f32) -> f32 {
    (side * (1.0 - FILL_RATIO) / 2.0).max(SCHEMATIC_TEXT_SIZE.min_px * TEXT_MARGIN_EM)
}

/// The largest scale, px per mm, a preview zooms to.
fn max_scale() -> f32 {
    SCHEMATIC_TEXT_SIZE.max_px / (TEXT_HEIGHT_MM * MAX_SCALE_TEXT_HEIGHTS)
}

/// The transform that centres `extent` in `size` and scales it to fit.
fn fit_transform(extent: Aabb, size: Size) -> ScreenTransform {
    let width = (extent.max_x - extent.min_x).max(f64::EPSILON) as f32;
    let height = (extent.max_y - extent.min_y).max(f64::EPSILON) as f32;
    let free_w = (size.width - 2.0 * fit_margin(size.width)).max(1.0);
    let free_h = (size.height - 2.0 * fit_margin(size.height)).max(1.0);
    let scale = (free_w / width).min(free_h / height).min(max_scale());
    let mid_x = ((extent.min_x + extent.max_x) / 2.0) as f32;
    let mid_y = ((extent.min_y + extent.max_y) / 2.0) as f32;
    ScreenTransform {
        offset_x: size.width / 2.0 - mid_x * scale,
        offset_y: size.height / 2.0 - mid_y * scale,
        scale,
        // The panel does not know its window's scale factor, so strokes snap
        // to the logical pixel grid; a fractional-DPI monitor blurs them.
        device_px_per_logical_px: UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    }
}

impl canvas::Program<()> for SymbolSheetPreview {
    type State = PreviewState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        {
            let mut shown = state.shown.borrow_mut();
            let current = shown
                .as_ref()
                .is_some_and(|(lib, colors)| *lib == self.lib && *colors == self.colors);
            if !current {
                state.cache.clear();
                *shown = Some((self.lib.clone(), self.colors));
            }
        }
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(
                iced::Point::ORIGIN,
                bounds.size(),
                to_iced(&self.colors.paper),
            );
            if bounds.width <= 0.0 || bounds.height <= 0.0 {
                return;
            }
            let (sheet, symbol) = preview_sheet(&self.lib);
            let transform = fit_transform(symbol_bounds(&sheet, &symbol), bounds.size());
            if let Some(scene) = build_schematic_scene(
                &sheet,
                &transform,
                &self.colors,
                Rectangle::with_size(bounds.size()),
                None,
                SceneOverrides::default(),
            ) {
                draw_built_scene(frame, &scene, &transform);
            }
        });
        vec![geometry]
    }
}

/// The Symbol preview of `lib`, `height` tall and as wide as its parent,
/// clipped to its box.
pub(crate) fn symbol_preview(
    lib: &LibSymbol,
    colors: CanvasColors,
    height: f32,
) -> Element<'static, ()> {
    let canvas = canvas::Canvas::new(SymbolSheetPreview {
        lib: lib.clone(),
        colors,
    })
    .width(Length::Fill)
    .height(height);
    container(canvas)
        .width(Length::Fill)
        .height(height)
        .clip(true)
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
        let size = Size::new(200.0, 156.0);
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
    }

    #[test]
    fn a_small_box_keeps_a_pixel_margin_for_pin_text() {
        let (sheet, symbol) = preview_sheet(&body_symbol());
        let size = Size::new(60.0, 40.0);
        let extent = symbol_bounds(&sheet, &symbol);
        let transform = fit_transform(extent, size);
        let low = transform.world_to_screen((extent.min_x, extent.min_y));
        let high = transform.world_to_screen((extent.max_x, extent.max_y));
        let room = SCHEMATIC_TEXT_SIZE.min_px * TEXT_MARGIN_EM;
        assert!(low.x >= room - 1e-3 && low.y >= room - 1e-3, "{low:?}");
        assert!(high.x <= size.width - room + 1e-3, "{high:?}");
        assert!(high.y <= size.height - room + 1e-3, "{high:?}");
    }

    #[test]
    fn a_tiny_symbol_is_not_blown_up_past_the_zoom_cap() {
        let extent = Aabb::new(-1.0, -0.5, 1.0, 0.5);
        let transform = fit_transform(extent, Size::new(300.0, 156.0));
        assert!(
            (transform.scale - max_scale()).abs() < 1e-3,
            "{}",
            transform.scale
        );
    }
}
