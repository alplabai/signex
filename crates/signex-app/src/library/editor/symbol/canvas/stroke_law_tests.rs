//! #1858: a symbol drawn in the Symbol Editor and the same symbol placed on
//! a schematic draw every stroke at the same pixel width at the same scale.

use super::*;
use crate::schematic_runtime::{ScreenTransform, build_schematic_scene, schematic_draw_options};
use signex_gfx::scene::Scene;
use signex_library::{PinOrientation, Symbol, SymbolGraphic, SymbolGraphicKind, SymbolPin};

const LIBRARY_WIDTH_MM: f64 = 0.254;

fn library_symbol() -> Symbol {
    let mut symbol = Symbol::empty("T");
    symbol.graphics.push(SymbolGraphic {
        body_style: 1,
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, -5.08],
            to: [10.16, 5.08],
        },
        stroke_width: LIBRARY_WIDTH_MM,
        part_number: 0,
        fill: None,
    });
    let mut pin = SymbolPin::new("1", "IN");
    pin.length = 2.54;
    pin.orientation = PinOrientation::Left;
    symbol.pins.push(pin);
    symbol
}

fn rgba(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

/// Pre-snap widths (body outline, pin) the Symbol Editor replays at `scale`.
fn editor_widths_px(symbol: &Symbol, scale: f32) -> (f32, f32) {
    let camera = crate::canvas::Camera::default();
    let canvas = SymbolCanvas::new(
        symbol,
        None,
        SymbolTool::Select,
        &[],
        state::PartView::normal(1),
        false,
        &camera,
        1.27,
        true,
        crate::render_config::GridStyle::default(),
        true,
        Color::WHITE,
        Color::from_rgb(0.2, 0.4, 0.9),
        Color::BLACK,
        Color::BLACK,
        Color::BLACK,
    );
    let snapshot = canvas.build_symbol_renderer_snapshot(&None, scale);
    let mut scene = Scene::default();
    SchematicRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::from_canvas_colors(signex_types::theme::canvas_colors(
            signex_types::theme::ThemeId::Signex,
        )),
        DirtyFlags::LINES | DirtyFlags::POLYGONS,
        &mut scene,
    );
    let options = symbol_draw_options(scale);
    let body = scene
        .polygons
        .iter()
        .find(|p| p.stroke_color == Some(rgba(canvas.body_color)))
        .expect("editor body outline");
    let pin = scene
        .lines
        .iter()
        .find(|l| l.color == rgba(canvas.pin_color))
        .expect("editor pin line");
    (
        options.stroke_px(body.stroke_width),
        options.stroke_px(pin.width),
    )
}

/// Pre-snap widths (body outline, pin) the schematic replays `symbol` at.
fn schematic_widths_px(symbol: &Symbol, scale: f32) -> (f32, f32) {
    let mut sheet = signex_types::schematic::SchematicSheet::default();
    sheet
        .lib_symbols
        .insert("T:T".to_string(), symbol.to_lib_symbol("T:T"));
    sheet.symbols.push(signex_types::schematic::Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "T:T".to_string(),
        unit: 1,
        ..Default::default()
    });
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 400.0,
        offset_y: 400.0,
        scale,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 800.0));
    let scene = build_schematic_scene(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        Default::default(),
    )
    .expect("placed symbol draws");
    let options = schematic_draw_options(&transform);
    let as_rgba =
        |c: &signex_types::theme::Color| rgba(Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0));
    let body = scene
        .polygons
        .iter()
        .find(|p| p.stroke_color == Some(as_rgba(&colors.body)))
        .expect("schematic body outline");
    let pin = scene
        .lines
        .iter()
        .find(|l| l.color == as_rgba(&colors.pin))
        .expect("schematic pin line");
    (
        options.stroke_px(body.stroke_width),
        options.stroke_px(pin.width),
    )
}

#[test]
fn symbol_editor_and_schematic_draw_a_symbol_at_the_same_px_width() {
    let symbol = library_symbol();
    for zoom_percent in [25.0_f32, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0] {
        let scale = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * zoom_percent / 100.0;
        let editor = editor_widths_px(&symbol, scale);
        let schematic = schematic_widths_px(&symbol, scale);
        assert_eq!(
            editor, schematic,
            "(body, pin) px at {zoom_percent}%: Symbol Editor {editor:?} vs schematic \
             {schematic:?} (#1858)"
        );
    }
}
