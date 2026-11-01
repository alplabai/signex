//! #1858 — every schematic stroke kind follows one zoom law: a world width in
//! mm that does not change with zoom (so the ratios between kinds never
//! change), drawn at `mm * scale` px and raised to one shared floor. Lowered
//! through the same `Scene` the CPU canvas replays and the GPU mount uploads.
//! The sheet frame / title block is drawn outside the scene (canvas
//! background) and is not covered here.

use iced::{Point as ScreenPoint, Rectangle, Size};
use signex_app::scene_shader::{SceneSurface, SchematicSurface};
use signex_app::schematic_runtime::{
    ScreenTransform, build_schematic_scene, schematic_draw_options,
};
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::scene::Scene;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{
    Bus, BusEntry, ChildSheet, FillType, Graphic, LabelType, NoConnect, Point, SchDrawing,
    SchematicSheet,
};
use signex_types::theme::{CanvasColors, Color, ThemeId, canvas_colors};
use std::collections::BTreeMap;

const SAMPLE: &str = include_str!("../../assets/sample-project/linear-regulator.snxsch");
const LIBRARY_WIDTH_MM: f64 = 0.254;
const ZOOMS_PERCENT: [f32; 7] = [25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0];
const NO_CONNECT_AT: (f64, f64) = (20.0, 20.0);
const BUS_ENTRY_AT: (f64, f64) = (150.0, 30.0);
const BUS_ENTRY_SIZE: (f64, f64) = (2.54, 2.54);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Wire,
    Bus,
    BusEntry,
    SymbolBody,
    Pin,
    NoConnect,
    NoErc,
    LabelMarker,
    ChildSheetFrame,
    Drawing,
}

const ALL_KINDS: [Kind; 10] = [
    Kind::Wire,
    Kind::Bus,
    Kind::BusEntry,
    Kind::SymbolBody,
    Kind::Pin,
    Kind::NoConnect,
    Kind::NoErc,
    Kind::LabelMarker,
    Kind::ChildSheetFrame,
    Kind::Drawing,
];

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

/// The sample sheet with library widths of 0.254 mm plus one of every other
/// stroke kind: a hierarchical label, a bus, a bus entry, a no-connect, a
/// No-ERC directive, a child sheet and a graphic line.
fn sheet() -> SchematicSheet {
    let mut sheet = SnxSchematic::parse(SAMPLE).expect("sample parses").sheet;
    for lib in sheet.lib_symbols.values_mut() {
        for item in &mut lib.graphics {
            set_graphic_width(&mut item.graphic, LIBRARY_WIDTH_MM);
        }
    }
    sheet.labels[0].label_type = LabelType::Hierarchical;
    sheet.buses.push(Bus {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(150.0, 20.0),
        end: Point::new(150.0, 60.0),
    });
    sheet.bus_entries.push(BusEntry {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(BUS_ENTRY_AT.0, BUS_ENTRY_AT.1),
        size: BUS_ENTRY_SIZE,
    });
    sheet.no_connects.push(NoConnect {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(NO_CONNECT_AT.0, NO_CONNECT_AT.1),
    });
    sheet.no_erc_directives.push(NoConnect {
        uuid: uuid::Uuid::now_v7(),
        position: Point::new(30.0, 20.0),
    });
    sheet.child_sheets.push(ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: "child".into(),
        filename: "child.snxsch".into(),
        position: Point::new(180.0, 20.0),
        size: (30.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    sheet.drawings.push(SchDrawing::Line {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(20.0, 150.0),
        end: Point::new(60.0, 150.0),
        width: 0.3,
        stroke_color: None,
        locked: false,
    });
    sheet
}

/// One distinct colour per kind that has its own theme token.
fn colors() -> CanvasColors {
    let c = |r| Color {
        r,
        g: 0,
        b: 0,
        a: 255,
    };
    let mut colors = canvas_colors(ThemeId::Signex);
    colors.wire = c(1);
    colors.bus = c(2);
    colors.body = c(3);
    colors.pin = c(4);
    colors.hier_label = c(5);
    colors.global_label = c(6);
    colors.no_connect = c(7);
    colors
}

fn rgba(color: Color) -> [f32; 4] {
    let c = iced::Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0);
    [c.r, c.g, c.b, c.a]
}

fn centred_at(p0: [f32; 2], p1: [f32; 2], at: (f64, f64)) -> bool {
    let mid = ((p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0);
    (mid.0 - at.0 as f32).abs() < 1e-3 && (mid.1 - at.1 as f32).abs() < 1e-3
}

fn line_kind(color: [f32; 4], p0: [f32; 2], p1: [f32; 2], colors: &CanvasColors) -> Option<Kind> {
    let entry_mid = (
        BUS_ENTRY_AT.0 + BUS_ENTRY_SIZE.0 / 2.0,
        BUS_ENTRY_AT.1 + BUS_ENTRY_SIZE.1 / 2.0,
    );
    if color == rgba(colors.wire) && centred_at(p0, p1, entry_mid) {
        Some(Kind::BusEntry)
    } else if color == rgba(colors.wire) {
        Some(Kind::Wire)
    } else if color == rgba(colors.bus) {
        Some(Kind::Bus)
    } else if color == rgba(colors.pin) {
        Some(Kind::Pin)
    } else if color == rgba(colors.no_connect) {
        Some(Kind::NoErc)
    } else if color == rgba(colors.body) && centred_at(p0, p1, NO_CONNECT_AT) {
        Some(Kind::NoConnect)
    } else if color == rgba(colors.body) {
        Some(Kind::SymbolBody)
    } else {
        None
    }
}

/// Every stroke width in `scene` (mm), by kind.
fn widths_by_kind(scene: &Scene, colors: &CanvasColors) -> BTreeMap<Kind, Vec<f32>> {
    let mut out: BTreeMap<Kind, Vec<f32>> = BTreeMap::new();
    for line in &scene.lines {
        if let Some(kind) = line_kind(line.color, line.p0, line.p1, colors) {
            out.entry(kind).or_default().push(line.width);
        }
    }
    for polygon in &scene.polygons {
        let kind = match polygon.stroke_color {
            Some(c) if c == rgba(colors.body) => Kind::SymbolBody,
            Some(c) if c == rgba(colors.hier_label) => Kind::LabelMarker,
            Some(c) if c == rgba(colors.global_label) => Kind::ChildSheetFrame,
            _ => continue,
        };
        out.entry(kind).or_default().push(polygon.stroke_width);
    }
    for item in scene.drawings.iter().chain(&scene.drawings_front) {
        if let OrderedItem::Line(line) = item {
            out.entry(Kind::Drawing).or_default().push(line.width);
        }
    }
    out
}

#[test]
fn stroke_kinds_share_one_zoom_law_1858() {
    let sheet = sheet();
    let colors = colors();
    let mut first_mm: BTreeMap<Kind, f32> = BTreeMap::new();
    let mut broken = Vec::new();
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
        // The CPU replay and the GPU surface raise thin strokes to one floor.
        let cpu_floor = schematic_draw_options(&transform).stroke_px(0.0);
        if cpu_floor != SchematicSurface::MIN_STROKE_PX {
            broken.push(format!(
                "{zoom}%: CPU floor {cpu_floor} px, GPU floor {} px",
                SchematicSurface::MIN_STROKE_PX
            ));
        }
        let by_kind = widths_by_kind(&scene, &colors);
        for kind in ALL_KINDS {
            let Some(widths) = by_kind.get(&kind) else {
                broken.push(format!("{zoom}%: no {kind:?} stroke in the scene"));
                continue;
            };
            if widths.iter().any(|w| *w != widths[0]) {
                broken.push(format!(
                    "{zoom}%: {kind:?} lowers to several widths {widths:?}"
                ));
            }
            let first = *first_mm.entry(kind).or_insert(widths[0]);
            if widths[0] != first {
                broken.push(format!(
                    "{kind:?} is {first} mm at {}% but {} mm at {zoom}%",
                    ZOOMS_PERCENT[0], widths[0]
                ));
            }
        }
    }
    if first_mm.get(&Kind::Pin) != first_mm.get(&Kind::SymbolBody) {
        broken.push("a pin does not draw at its symbol's library width".into());
    }
    if first_mm.get(&Kind::SymbolBody) != Some(&(LIBRARY_WIDTH_MM as f32)) {
        broken.push(format!("the body is not its library width: {first_mm:?}"));
    }
    assert!(
        broken.is_empty(),
        "stroke kinds no longer share one zoom law (#1858):\n{}",
        broken.join("\n")
    );
}
