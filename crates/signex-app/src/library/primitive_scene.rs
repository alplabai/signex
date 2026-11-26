//! A library symbol as a world-space [`Scene`], through the path the
//! Components-panel preview and the Symbol Editor's sheet use (#3116):
//! the symbol placed on a sheet and lowered by `build_schematic_scene`.
//! The placement is the preview's own (`preview_sheet`). `PreviewFootprint::scene` is the footprint counterpart.

use iced::{Rectangle, Size};
use signex_gfx::scene::Scene;
use signex_types::schematic::LibSymbol;
use signex_types::theme::CanvasColors;

use crate::panels::components_panel::details::symbol_canvas::preview_sheet;
use crate::schematic_runtime::{SceneOverrides, ScreenTransform, build_schematic_scene};

/// Zoom the scene is lowered at, px per mm; the sheet-like zoom, so no text
/// or pin is dropped for being small.
const LOWERING_SCALE: f32 = 20.0;

/// `lib`, unit 1, normal body, near the paper's centre; `None` for a symbol
/// that draws nothing.
pub fn symbol_scene(lib: &LibSymbol, colors: &CanvasColors) -> Option<Scene> {
    let (sheet, _) = preview_sheet(lib);
    let (width, height) = sheet.paper_mm();
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: LOWERING_SCALE,
        device_px_per_logical_px: 1.0,
    };
    let area = Rectangle::with_size(Size::new(
        width as f32 * LOWERING_SCALE,
        height as f32 * LOWERING_SCALE,
    ));
    build_schematic_scene(
        &sheet,
        &transform,
        colors,
        area,
        None,
        SceneOverrides::default(),
    )
}
