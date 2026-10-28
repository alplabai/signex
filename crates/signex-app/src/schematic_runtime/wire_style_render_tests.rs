//! Net-class styling on the drawn wire and on its hit area.

use super::tests::empty_snapshot;
use super::*;
use signex_gfx::primitive::line::LineSegment;
use signex_types::net_class::NetClassStyle;
use signex_types::schematic::Wire;
use signex_types::theme::{ThemeId, canvas_colors};

const WIRE_ID: uuid::Uuid = uuid::Uuid::from_u128(7);

fn sheet_with_wire() -> SchematicRenderSnapshot {
    let mut sheet = empty_snapshot();
    sheet.wires.push(Wire {
        uuid: WIRE_ID,
        start: Point::new(10.0, 10.0),
        end: Point::new(60.0, 10.0),
        stroke_width: 0.15,
        ..Default::default()
    });
    sheet
}

fn class_style(
    line: NetLineStyle,
    width: Option<f64>,
    color: Option<ThemeColor>,
) -> WireClassStyles {
    let mut map = WireClassStyles::new();
    map.insert(
        WIRE_ID,
        NetClassStyle {
            color,
            wire_width_mm: width,
            line,
        },
    );
    map
}

fn scene_line(sheet: &SchematicRenderSnapshot, styling: Option<WireStyling<'_>>) -> LineSegment {
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let scene = build_schematic_scene(
        sheet,
        &transform,
        &colors,
        bounds,
        None,
        SceneOverrides {
            wire_styling: styling,
            ..SceneOverrides::default()
        },
    )
    .expect("a wire draws something");
    scene.lines[0]
}

/// The cached scene must rebuild when either half of the overrides changes:
/// the net-class styling and the placement designators (#1151), and be
/// reused when neither does.
#[test]
fn scene_cache_rebuilds_on_class_style_or_designator_change() {
    let sheet = sheet_with_wire();
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let no_colors = HashMap::new();
    let thin = class_style(NetLineStyle::Solid, Some(0.5), None);
    let thick = class_style(NetLineStyle::Solid, Some(1.0), None);
    let no_designators = HashMap::new();
    let mut renamed = HashMap::new();
    renamed.insert(uuid::Uuid::new_v4(), "R7".to_string());

    let mut cache = ContentSceneCache::default();
    let mut request = |styles: &WireClassStyles, designators: &HashMap<uuid::Uuid, String>| {
        cache.scene_for(
            &sheet,
            &transform,
            &colors,
            bounds,
            None,
            SceneOverrides {
                wire_styling: Some(WireStyling {
                    color_overrides: &no_colors,
                    class_styles: styles,
                }),
                designators: Some(designators),
                special_strings: None,
            },
        );
        cache.builds()
    };
    assert_eq!(request(&thin, &no_designators), 1);
    assert_eq!(request(&thin, &no_designators), 1, "same request is reused");
    assert_eq!(request(&thick, &no_designators), 2, "class style changed");
    assert_eq!(request(&thick, &renamed), 3, "designators changed");
}

#[test]
fn class_line_style_width_and_colour_reach_the_scene() {
    let sheet = sheet_with_wire();
    let red = ThemeColor::new(255, 0, 0, 255);
    let classes = class_style(NetLineStyle::Dashed, Some(0.6), Some(red));
    let no_overrides = HashMap::new();

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &no_overrides,
            class_styles: &classes,
        }),
    );

    assert!(line.is_dashed());
    assert!((line.width - 0.6).abs() < 1e-6);
    assert_eq!(line.color, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn dotted_class_draws_dotted() {
    let sheet = sheet_with_wire();
    let classes = class_style(NetLineStyle::Dotted, None, None);
    let no_overrides = HashMap::new();

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &no_overrides,
            class_styles: &classes,
        }),
    );

    assert!(line.is_dotted());
    assert!((line.width - 0.15).abs() < 1e-6, "own width is kept");
}

#[test]
fn per_net_colour_override_wins_over_the_class_colour() {
    let sheet = sheet_with_wire();
    let red = ThemeColor::new(255, 0, 0, 255);
    let green = ThemeColor::new(0, 255, 0, 255);
    let classes = class_style(NetLineStyle::Dashed, Some(0.6), Some(red));
    let mut overrides = HashMap::new();
    overrides.insert(WIRE_ID, green);

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
        }),
    );

    assert_eq!(line.color, [0.0, 1.0, 0.0, 1.0], "override colour wins");
    assert!(line.is_dashed(), "class line style still applies");
    assert!((line.width - 0.6).abs() < 1e-6, "class width still applies");
}

#[test]
fn a_wire_without_a_class_draws_as_before() {
    let sheet = sheet_with_wire();

    let line = scene_line(&sheet, None);

    assert_eq!(line.style, 0);
    assert!((line.width - 0.15).abs() < 1e-6);
}

#[test]
fn a_class_width_widens_the_hit_area_to_the_drawn_width() {
    let sheet = sheet_with_wire();
    // 1.5 mm off the wire: outside the default 0.25 mm tolerance.
    let (x, y) = (30.0, 11.5);
    assert!(hit_test::hit_test(&sheet, &Designators::new(), x, y).is_none());

    let wide = class_style(NetLineStyle::Solid, Some(2.0), None);
    let hit = hit_test::hit_test_styled(&sheet, &Designators::new(), x, y, &wide);

    assert_eq!(hit.map(|h| h.uuid), Some(WIRE_ID));
}

#[test]
fn a_style_without_width_keeps_the_default_hit_area() {
    let sheet = sheet_with_wire();
    let dashed = class_style(NetLineStyle::Dashed, None, None);

    assert!(hit_test::hit_test_styled(&sheet, &Designators::new(), 30.0, 11.5, &dashed).is_none());
    assert!(hit_test::hit_test_styled(&sheet, &Designators::new(), 30.0, 10.1, &dashed).is_some());
}

const JUNCTION_ID: uuid::Uuid = uuid::Uuid::from_u128(8);

/// The colour the one junction dot of a one-junction sheet is drawn with.
fn junction_draw_color(styling: Option<WireStyling<'_>>) -> [f32; 4] {
    let mut sheet = empty_snapshot();
    sheet.junctions.push(signex_types::schematic::Junction {
        uuid: JUNCTION_ID,
        position: Point::new(10.0, 10.0),
        ..Default::default()
    });
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let snapshot = build_renderer_snapshot(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        SceneOverrides {
            wire_styling: styling,
            ..SceneOverrides::default()
        },
    );
    snapshot.junctions[0].color
}

fn junction_class(color: Option<ThemeColor>) -> WireClassStyles {
    let mut map = WireClassStyles::new();
    map.insert(
        JUNCTION_ID,
        NetClassStyle {
            color,
            wire_width_mm: None,
            line: NetLineStyle::Solid,
        },
    );
    map
}

#[test]
fn a_junction_takes_its_net_class_colour_and_otherwise_the_theme_colour() {
    let theme = to_rgba(to_iced(&canvas_colors(ThemeId::Signex).junction));
    let red = ThemeColor::new(255, 0, 0, 255);
    let no_overrides = HashMap::new();
    let styling = |classes| WireStyling {
        color_overrides: &no_overrides,
        class_styles: classes,
    };

    let coloured = junction_class(Some(red));
    assert_eq!(
        junction_draw_color(Some(styling(&coloured))),
        [1.0, 0.0, 0.0, 1.0]
    );
    let uncoloured = junction_class(None);
    assert_eq!(junction_draw_color(Some(styling(&uncoloured))), theme);
    assert_eq!(junction_draw_color(None), theme);
}

#[test]
fn a_junction_override_beats_the_class_colour_but_does_not_tint_alone() {
    let theme = to_rgba(to_iced(&canvas_colors(ThemeId::Signex).junction));
    let green = ThemeColor::new(0, 255, 0, 255);
    let mut overrides = HashMap::new();
    overrides.insert(JUNCTION_ID, green);

    let coloured = junction_class(Some(ThemeColor::new(255, 0, 0, 255)));
    let with_class = WireStyling {
        color_overrides: &overrides,
        class_styles: &coloured,
    };
    assert_eq!(
        junction_draw_color(Some(with_class)),
        [0.0, 1.0, 0.0, 1.0],
        "override wins over the class colour, as for a wire"
    );

    let no_classes = WireClassStyles::new();
    let override_only = WireStyling {
        color_overrides: &overrides,
        class_styles: &no_classes,
    };
    assert_eq!(
        junction_draw_color(Some(override_only)),
        theme,
        "with no classes a junction is drawn as it was before classes existed"
    );
}
