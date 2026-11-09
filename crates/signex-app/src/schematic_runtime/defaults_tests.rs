//! #2026: unset widths and sizes follow the project defaults on the canvas
//! and in export, the two agree, and a defaults change rebuilds the scene.

use super::tests::empty_snapshot;
use super::*;
use signex_output::{PdfOptions, SheetSnapshot, svg::SvgElement, svg::SvgRenderContext};
use signex_types::schematic::{Bus, Junction, SchematicDefaults, Wire};
use signex_types::theme::{ThemeId, canvas_colors};

const WIRE: uuid::Uuid = uuid::Uuid::from_u128(1);
const BUS: uuid::Uuid = uuid::Uuid::from_u128(2);
const EXPLICIT: uuid::Uuid = uuid::Uuid::from_u128(3);

fn project() -> SchematicDefaults {
    SchematicDefaults {
        wire_width_mm: 0.3,
        bus_width_mm: 0.6,
        junction_diameter_mm: 1.0,
        ..SchematicDefaults::BUILT_IN
    }
}

/// An unset wire, an explicit 0.5 mm wire, a bus and an unset junction.
fn sheet() -> SchematicRenderSnapshot {
    let mut sheet = empty_snapshot();
    sheet.paper_size = "A4".into();
    sheet.wires.push(Wire {
        uuid: WIRE,
        start: Point::new(10.0, 10.0),
        end: Point::new(60.0, 10.0),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        uuid: EXPLICIT,
        start: Point::new(10.0, 30.0),
        end: Point::new(60.0, 30.0),
        stroke_width: 0.5,
        ..Default::default()
    });
    sheet.buses.push(Bus {
        uuid: BUS,
        start: Point::new(10.0, 50.0),
        end: Point::new(60.0, 50.0),
    });
    sheet.junctions.push(Junction {
        uuid: uuid::Uuid::from_u128(4),
        position: Point::new(30.0, 10.0),
        diameter: 0.0,
        minted: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    sheet
}

fn transform() -> ScreenTransform {
    ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    }
}

fn bounds() -> Rectangle {
    Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0))
}

/// Canvas widths: (unset wire, explicit wire, bus, junction diameter).
fn canvas_widths(defaults: SchematicDefaults) -> (f32, f32, f32, f32) {
    let snap = build_renderer_snapshot(
        &sheet(),
        &transform(),
        &canvas_colors(ThemeId::GraphiteLight),
        bounds(),
        None,
        SceneOverrides {
            defaults,
            ..SceneOverrides::default()
        },
    );
    let width = |id| {
        snap.wires
            .iter()
            .find(|w| w.id == renderer_id(id))
            .expect("drawn")
            .width_mm
    };
    (
        width(WIRE),
        width(EXPLICIT),
        width(BUS),
        2.0 * snap.junctions[0].radius_mm,
    )
}

/// Export widths in the same order, in page units.
fn export_widths(defaults: SchematicDefaults) -> (f32, f32, f32, f32) {
    let snapshot = SheetSnapshot {
        path: "t.snxsch".into(),
        schematic: sheet(),
        sheet_name: "t".into(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    };
    let opts = PdfOptions {
        schematic_defaults: defaults,
        ..PdfOptions::default()
    };
    let page = SvgRenderContext::from_sheet(&snapshot, &opts, 297.0, 210.0, 1.0, None);
    let stroke = |i: usize| match &page.elements[i] {
        SvgElement::Path { style, .. } => style.stroke_width,
        SvgElement::Text { .. } => panic!("a path"),
    };
    // Page fill, border, then the two wires, the bus and the junction disc
    // (#2115): its x extent is the diameter.
    let side = match &page.elements[5] {
        SvgElement::Path { commands, .. } => {
            let xs: Vec<f32> = commands
                .iter()
                .filter_map(|c| match c {
                    signex_output::svg::SvgPathCommand::MoveTo(p)
                    | signex_output::svg::SvgPathCommand::LineTo(p)
                    | signex_output::svg::SvgPathCommand::CubicTo(_, _, p) => Some(p.x),
                    _ => None,
                })
                .collect();
            xs.iter().copied().fold(f32::MIN, f32::max)
                - xs.iter().copied().fold(f32::MAX, f32::min)
        }
        SvgElement::Text { .. } => panic!("a path"),
    };
    (stroke(2), stroke(3), stroke(4), side)
}

#[test]
fn canvas_and_export_agree_on_resolved_widths() {
    for defaults in [SchematicDefaults::BUILT_IN, project()] {
        let (canvas, export) = (canvas_widths(defaults), export_widths(defaults));
        // The page fits the sheet with margins: compare in the page's scale,
        // read off the explicit 0.5 mm wire.
        let scale = export.1 / 0.5;
        let close = |a: f32, b: f32| (a * scale - b).abs() < 1e-4;
        assert!(close(canvas.0, export.0), "{canvas:?} vs {export:?}");
        assert!(close(canvas.1, export.1), "{canvas:?} vs {export:?}");
        assert!(close(canvas.2, export.2), "{canvas:?} vs {export:?}");
        assert!(close(canvas.3, export.3), "{canvas:?} vs {export:?}");
    }
}

#[test]
fn unset_items_follow_the_project_and_explicit_ones_do_not() {
    assert_eq!(
        canvas_widths(SchematicDefaults::BUILT_IN),
        (0.254, 0.5, 0.45, 0.7)
    );
    assert_eq!(canvas_widths(project()), (0.3, 0.5, 0.6, 1.0));
}

#[test]
fn a_defaults_change_rebuilds_the_cached_scene_once() {
    let mut cache = ContentSceneCache::default();
    let sheet = sheet();
    let colors = canvas_colors(ThemeId::GraphiteLight);
    let mut build = |defaults| {
        let overrides = SceneOverrides {
            defaults,
            ..SceneOverrides::default()
        };
        let _ = cache.scene_for(&sheet, &transform(), &colors, bounds(), None, overrides);
        cache.builds()
    };
    assert_eq!(build(SchematicDefaults::BUILT_IN), 1);
    assert_eq!(build(SchematicDefaults::BUILT_IN), 1, "nothing changed");
    assert_eq!(build(project()), 2, "the project default changed");
    assert_eq!(build(project()), 2);
}

/// #2026: a project line style strokes a classless wire on the canvas and
/// in export alike.
#[test]
fn a_project_line_style_dashes_the_wire_on_canvas_and_in_export() {
    use signex_types::net_class::NetLineStyle;
    let dashed = SchematicDefaults {
        wire_line_style: NetLineStyle::Dashed,
        ..SchematicDefaults::BUILT_IN
    };
    let canvas = |defaults| {
        build_renderer_snapshot(
            &sheet(),
            &transform(),
            &canvas_colors(ThemeId::GraphiteLight),
            bounds(),
            None,
            SceneOverrides {
                defaults,
                ..SceneOverrides::default()
            },
        )
        .wire_line_styles
        .get(&renderer_id(WIRE))
        .copied()
    };
    assert_eq!(canvas(SchematicDefaults::BUILT_IN), None, "solid");
    assert_eq!(canvas(dashed), Some(NetLineStyle::Dashed));

    let export_segments = |defaults| {
        let snapshot = SheetSnapshot {
            path: "t.snxsch".into(),
            schematic: sheet(),
            sheet_name: "t".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        };
        let opts = PdfOptions {
            schematic_defaults: defaults,
            ..PdfOptions::default()
        };
        let page = SvgRenderContext::from_sheet(&snapshot, &opts, 297.0, 210.0, 1.0, None);
        match &page.elements[2] {
            SvgElement::Path { commands, .. } => commands.len(),
            SvgElement::Text { .. } => panic!("a path"),
        }
    };
    assert_eq!(
        export_segments(SchematicDefaults::BUILT_IN),
        2,
        "one solid segment"
    );
    assert!(export_segments(dashed) > 2, "dashes");
}

/// #2026: a class's explicit Solid overrides a project Dashed on the canvas
/// and in export; a class with no line style inherits the project's.
#[test]
fn an_explicit_solid_class_beats_a_dashed_project_and_none_inherits() {
    use signex_types::net_class::{NetClassStyle, NetLineStyle};
    let dashed = SchematicDefaults {
        wire_line_style: NetLineStyle::Dashed,
        ..SchematicDefaults::BUILT_IN
    };
    let classes = |line| {
        WireClassStyles::from([(
            WIRE,
            NetClassStyle {
                line,
                ..NetClassStyle::default()
            },
        )])
    };
    let canvas = |classes: &WireClassStyles| {
        let none = HashMap::new();
        build_renderer_snapshot(
            &sheet(),
            &transform(),
            &canvas_colors(ThemeId::GraphiteLight),
            bounds(),
            None,
            SceneOverrides {
                wire_styling: Some(WireStyling {
                    color_overrides: &none,
                    class_styles: classes,
                    class_revision: 0,
                }),
                defaults: dashed,
                ..SceneOverrides::default()
            },
        )
        .wire_line_styles
        .get(&renderer_id(WIRE))
        .copied()
    };
    let export_segments = |classes: &WireClassStyles| {
        let snapshot = SheetSnapshot {
            path: "t.snxsch".into(),
            schematic: sheet(),
            sheet_name: "t".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        };
        let opts = PdfOptions {
            schematic_defaults: dashed,
            net_class_styles: classes.clone(),
            ..PdfOptions::default()
        };
        let page = SvgRenderContext::from_sheet(&snapshot, &opts, 297.0, 210.0, 1.0, None);
        match &page.elements[2] {
            SvgElement::Path { commands, .. } => commands.len(),
            SvgElement::Text { .. } => panic!("a path"),
        }
    };
    let solid = classes(Some(NetLineStyle::Solid));
    assert_eq!(canvas(&solid), None, "explicit Solid draws solid");
    assert_eq!(export_segments(&solid), 2, "and exports solid");
    let inherit = classes(None);
    assert_eq!(
        canvas(&inherit),
        Some(NetLineStyle::Dashed),
        "None inherits"
    );
    assert!(export_segments(&inherit) > 2, "and exports dashed");
}
