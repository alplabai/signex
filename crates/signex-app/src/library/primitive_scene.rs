//! A library symbol as a world-space [`Scene`], through the path the
//! Components-panel preview and the Symbol Editor's sheet use (#3116):
//! the symbol placed on a sheet and lowered by `build_schematic_scene`.
//! `PreviewFootprint::scene` is the footprint counterpart.

use iced::{Rectangle, Size};
use signex_gfx::scene::Scene;
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
use signex_types::theme::CanvasColors;

use crate::schematic_runtime::{SceneOverrides, ScreenTransform, build_schematic_scene};

/// The preview's grid, mm: the part sits on a multiple of it.
const PLACEMENT_GRID_MM: f64 = 2.54;
/// Zoom the scene is lowered at, px per mm; the sheet-like zoom, so no text
/// or pin is dropped for being small.
const LOWERING_SCALE: f32 = 20.0;

/// `lib`, unit 1, normal body, near the paper's centre; `None` for a symbol
/// that draws nothing.
pub fn symbol_scene(lib: &LibSymbol, colors: &CanvasColors) -> Option<Scene> {
    let mut sheet = SchematicSheet::default();
    let (width, height) = sheet.paper_mm();
    let on_grid = |mid: f64| (mid / PLACEMENT_GRID_MM).round() * PLACEMENT_GRID_MM;
    sheet.lib_symbols.insert(lib.id.clone(), lib.clone());
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::nil(),
        lib_id: lib.id.clone(),
        position: signex_types::schematic::Point::new(on_grid(width / 2.0), on_grid(height / 2.0)),
        ..Symbol::default()
    });
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
