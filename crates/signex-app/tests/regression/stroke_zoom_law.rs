//! #1858 — every schematic stroke kind follows one zoom law: a world width in
//! mm that does not change with zoom, drawn at `mm * scale` px, raised to one
//! shared floor when thinner. Lowered through the same `Scene` the CPU canvas
//! replays and the GPU mount uploads.

use iced::{Point as ScreenPoint, Rectangle, Size};
use signex_app::scene_shader::{SceneSurface, SchematicSurface};
use signex_app::schematic_runtime::{
    ScreenTransform, build_schematic_scene, schematic_draw_options,
};
use signex_gfx::scene::Scene;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{Bus, Graphic, LabelType, NoConnect, Point, SchematicSheet};
use signex_types::theme::{CanvasColors, Color, ThemeId, canvas_colors};
use std::collections::BTreeMap;

const SAMPLE: &str = include_str!("../../assets/sample-project/linear-regulator.snxsch");
const LIBRARY_WIDTH_MM: f64 = 0.254;
const ZOOMS_PERCENT: [f32; 7] = [25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0];
const NO_CONNECT_AT: (f64, f64) = (20.0, 20.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Wire,
    Bus,
    SymbolBody,
    Pin,
    NoConnect,
    LabelMarker,
}

fn set_graphic_width(graphic: &mut Graphic, width_mm: f64) {
    match graphic {
        Graphic::Polyline { width, .. }
        | Graphic::Rectangle { width, .. }
        | Graphic::Circle { width, .. }
        | Graphic::Arc { width, .. }
        | Graphic::Bezier { width, .. } => *width = width_mm,
        _ => {}
    }
}

/// The sample sheet with library widths of 0.254 mm, a global label, a bus
/// and a no-connect, so every stroke kind is on it.
fn sheet() -> SchematicSheet {
    let mut sheet = SnxSchematic::parse(SAMPLE).expect("sample parses").sheet;
    for lib in sheet.lib_symbols.values_mut() {
        for item in &mut lib.graphics {
            set_graphic_width(&mut item.graphic, LIBRARY_WIDTH_MM);
        }
    }
    sheet.labels[0].label_type = LabelType::Global;
    sheet.buses.push(Bus {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(150.0, 20.0),
        end: Point::new(150.0, 60.0),
    });
    sheet.no_connects.push(NoConnect {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(NO_CONNECT_AT.0, NO_CONNECT_AT.1),
    });
    sheet
}

/// One distinct colour per kind, so the lowered `Scene` can be sorted by it.
fn colors() -> CanvasColors {
    let mut colors = canvas_colors(ThemeId::Signex);
    colors.wire = Color {
        r: 1,
        g: 0,
        b: 0,
        a: 255,
    };
    colors.bus = Color {
        r: 2,
        g: 0,
        b: 0,
        a: 255,
    };
    colors.body = Color {
        r: 3,
        g: 0,
        b: 0,
        a: 255,
    };
    colors.pin = Color {
        r: 4,
        g: 0,
        b: 0,
        a: 255,
    };
    colors.global_label = Color {
        r: 5,
        g: 0,
        b: 0,
        a: 255,
    };
    colors
}

fn rgba(color: Color) -> [f32; 4] {
    let c = iced::Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0);
    [c.r, c.g, c.b, c.a]
}

fn near_no_connect(p0: [f32; 2], p1: [f32; 2]) -> bool {
    let mid = ((p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0);
    (mid.0 - NO_CONNECT_AT.0 as f32).abs() < 1e-3 && (mid.1 - NO_CONNECT_AT.1 as f32).abs() < 1e-3
}

/// Every stroke width in `scene` (mm), by kind.
fn widths_by_kind(scene: &Scene, colors: &CanvasColors) -> BTreeMap<Kind, Vec<f32>> {
    let mut out: BTreeMap<Kind, Vec<f32>> = BTreeMap::new();
    for line in &scene.lines {
        let kind = if line.color == rgba(colors.wire) {
            Kind::Wire
        } else if line.color == rgba(colors.bus) {
            Kind::Bus
        } else if line.color == rgba(colors.pin) {
            Kind::Pin
        } else if line.color == rgba(colors.body) && near_no_connect(line.p0, line.p1) {
            Kind::NoConnect
        } else if line.color == rgba(colors.body) {
            Kind::SymbolBody
        } else {
            continue;
        };
        out.entry(kind).or_default().push(line.width);
    }
    for polygon in &scene.polygons {
        let kind = match polygon.stroke_color {
            Some(c) if c == rgba(colors.body) => Kind::SymbolBody,
            Some(c) if c == rgba(colors.global_label) => Kind::LabelMarker,
            _ => continue,
        };
        out.entry(kind).or_default().push(polygon.stroke_width);
    }
    out
}

/// The one width (mm) every stroke of `kind` lowers to at this zoom.
fn single_width(kind: Kind, widths: &[f32], zoom: f32) -> f32 {
    let first = widths[0];
    assert!(
        widths.iter().all(|w| *w == first),
        "stroke kinds no longer share one zoom law (#1858): {kind:?} lowers to several \
         widths at {zoom}%: {widths:?}"
    );
    first
}

#[test]
fn stroke_kinds_share_one_zoom_law_1858() {
    let sheet = sheet();
    let colors = colors();
    let mut mm_at_first_zoom: BTreeMap<Kind, f32> = BTreeMap::new();
    for zoom in ZOOMS_PERCENT {
        let scale = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * zoom / 100.0;
        let transform = ScreenTransform {
            offset_x: 0.0,
            offset_y: 0.0,
            scale,
        };
        let bounds = Rectangle::new(ScreenPoint::ORIGIN, Size::new(320.0 * scale, 320.0 * scale));
        let scene = build_schematic_scene(
            &sheet,
            &transform,
            &colors,
            bounds,
            None,
            Default::default(),
        )
        .expect("the sheet draws");
        let options = schematic_draw_options(&transform);
        let floor = options.stroke_px(0.0);
        assert_eq!(
            floor,
            SchematicSurface::MIN_STROKE_PX,
            "stroke kinds no longer share one zoom law (#1858): the CPU replay and the GPU \
             surface floor strokes differently"
        );
        let by_kind = widths_by_kind(&scene, &colors);
        for kind in [
            Kind::Wire,
            Kind::Bus,
            Kind::SymbolBody,
            Kind::Pin,
            Kind::NoConnect,
            Kind::LabelMarker,
        ] {
            let widths = by_kind
                .get(&kind)
                .unwrap_or_else(|| panic!("no {kind:?} stroke at {zoom}%"));
            let mm = single_width(kind, widths, zoom);
            let px = options.stroke_px(mm);
            let expected = (mm * scale).max(floor);
            assert_eq!(
                px, expected,
                "stroke kinds no longer share one zoom law (#1858): {kind:?} {mm} mm draws at \
                 {px} px at {zoom}%, not mm * scale or the shared floor"
            );
            let first = *mm_at_first_zoom.entry(kind).or_insert(mm);
            assert_eq!(
                mm, first,
                "stroke kinds no longer share one zoom law (#1858): {kind:?} is {first} mm at \
                 {}% but {mm} mm at {zoom}%",
                ZOOMS_PERCENT[0]
            );
        }
    }
    assert_eq!(
        mm_at_first_zoom[&Kind::Pin],
        mm_at_first_zoom[&Kind::SymbolBody],
        "stroke kinds no longer share one zoom law (#1858): a pin must draw at its symbol's \
         library width"
    );
    assert_eq!(mm_at_first_zoom[&Kind::SymbolBody], LIBRARY_WIDTH_MM as f32);
}
