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

/// Hit-test widths from a class map alone, at the built-in defaults.
fn class_widths(styles: &WireClassStyles) -> WireWidths<'_> {
    WireWidths {
        class_styles: Some(styles),
        defaults: Default::default(),
    }
}

/// The same wire with its width unset, so a class width applies to it: an
/// explicit width beats the class (#2026).
fn sheet_with_unset_wire() -> SchematicRenderSnapshot {
    let mut sheet = sheet_with_wire();
    sheet.wires[0].stroke_width = 0.0;
    sheet
}

fn class_style(
    line: Option<NetLineStyle>,
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
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
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
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let no_colors = HashMap::new();
    let thin = class_style(None, Some(0.5), None);
    let thick = class_style(None, Some(1.0), None);
    let no_designators = HashMap::new();
    let mut renamed = HashMap::new();
    renamed.insert(uuid::Uuid::now_v7(), "R7".to_string());

    let mut cache = ContentSceneCache::default();
    let mut request =
        |styles: &WireClassStyles, revision: u64, designators: &HashMap<uuid::Uuid, String>| {
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
                        class_revision: revision,
                    }),
                    designators: Some(designators),
                    special_strings: None,
                    defaults: Default::default(),
                },
            );
            cache.builds()
        };
    assert_eq!(request(&thin, 1, &no_designators), 1);
    assert_eq!(
        request(&thin, 1, &no_designators),
        1,
        "same request is reused"
    );
    assert_eq!(
        request(&thick, 2, &no_designators),
        2,
        "class style changed"
    );
    assert_eq!(request(&thick, 2, &renamed), 3, "designators changed");
    // The map is not compared per frame: the revision is what says it moved.
    assert_eq!(request(&thin, 2, &renamed), 3, "same revision is reused");
}

#[test]
fn class_line_style_width_and_colour_reach_the_scene() {
    let sheet = sheet_with_unset_wire();
    let red = ThemeColor::new(255, 0, 0, 255);
    let classes = class_style(Some(NetLineStyle::Dashed), Some(0.6), Some(red));
    let no_overrides = HashMap::new();

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &no_overrides,
            class_styles: &classes,
            class_revision: 0,
        }),
    );

    assert!(line.is_dashed());
    assert!((line.width - 0.6).abs() < 1e-6);
    assert_eq!(line.color, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn dotted_class_draws_dotted() {
    let sheet = sheet_with_wire();
    let classes = class_style(Some(NetLineStyle::Dotted), None, None);
    let no_overrides = HashMap::new();

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &no_overrides,
            class_styles: &classes,
            class_revision: 0,
        }),
    );

    assert!(line.is_dotted());
    assert!((line.width - 0.15).abs() < 1e-6, "own width is kept");
}

#[test]
fn per_net_colour_override_wins_over_the_class_colour() {
    let sheet = sheet_with_unset_wire();
    let red = ThemeColor::new(255, 0, 0, 255);
    let green = ThemeColor::new(0, 255, 0, 255);
    let classes = class_style(Some(NetLineStyle::Dashed), Some(0.6), Some(red));
    let mut overrides = HashMap::new();
    overrides.insert(WIRE_ID, green);

    let line = scene_line(
        &sheet,
        Some(WireStyling {
            color_overrides: &overrides,
            class_styles: &classes,
            class_revision: 0,
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
    let sheet = sheet_with_unset_wire();
    // 1.5 mm off the wire: outside the default 0.25 mm tolerance.
    let (x, y) = (30.0, 11.5);
    assert!(hit_test::hit_test(&sheet, &Designators::new(), x, y).is_none());

    let wide = class_style(None, Some(2.0), None);
    let hit = hit_test::hit_test_styled(&sheet, &Designators::new(), x, y, class_widths(&wide));

    assert_eq!(hit.map(|h| h.uuid), Some(WIRE_ID));
}

/// #2026: a wide project wire width widens an unset wire's hit area and its
/// box, as a class width does.
#[test]
fn a_project_wire_width_widens_the_hit_area_and_the_box() {
    let sheet = sheet_with_unset_wire();
    let wide = WireWidths {
        class_styles: None,
        defaults: signex_types::schematic::SchematicDefaults {
            wire_width_mm: 2.0,
            ..signex_types::schematic::SchematicDefaults::BUILT_IN
        },
    };
    let none = Designators::new();
    let hit = |widths| hit_test::hit_test_styled(&sheet, &none, 30.0, 11.5, widths);
    assert!(hit(WireWidths::default()).is_none());
    assert_eq!(hit(wide).map(|h| h.uuid), Some(WIRE_ID));

    // A box just around the thin wire holds it; the wide wire sticks out.
    let tight = Aabb::new(9.5, 9.5, 60.5, 10.5);
    let inside = |widths| {
        hit_test::hit_test_rect_mode(
            &sheet,
            &none,
            &tight,
            hit_test::SelectionMode::Inside,
            widths,
        )
    };
    assert!(
        inside(WireWidths::default())
            .iter()
            .any(|h| h.uuid == WIRE_ID)
    );
    assert!(!inside(wide).iter().any(|h| h.uuid == WIRE_ID));
}

#[test]
fn a_style_without_width_keeps_the_default_hit_area() {
    let sheet = sheet_with_wire();
    let dashed = class_style(Some(NetLineStyle::Dashed), None, None);

    assert!(
        hit_test::hit_test_styled(
            &sheet,
            &Designators::new(),
            30.0,
            11.5,
            class_widths(&dashed)
        )
        .is_none()
    );
    assert!(
        hit_test::hit_test_styled(
            &sheet,
            &Designators::new(),
            30.0,
            10.1,
            class_widths(&dashed)
        )
        .is_some()
    );
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
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
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
            line: None,
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
        class_revision: 0,
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
        class_revision: 0,
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
        class_revision: 0,
    };
    assert_eq!(
        junction_draw_color(Some(override_only)),
        theme,
        "with no classes a junction is drawn as it was before classes existed"
    );
}

/// Both segments of a bent wire emit the same colour and width (#1654): the
/// difference seen on screen is pixel thickness, fixed in `scene_line_snap`.
#[test]
fn every_segment_of_a_bent_wire_gets_the_same_colour_and_width() {
    let mut sheet = empty_snapshot();
    let legs = [((10.0, 10.0), (60.0, 10.0)), ((60.0, 10.0), (60.0, 40.0))];
    for (index, (a, b)) in legs.into_iter().enumerate() {
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::from_u128(100 + index as u128),
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
            ..Default::default()
        });
    }
    let colors = canvas_colors(ThemeId::Signex);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));

    let scene = build_schematic_scene(
        &sheet,
        &transform,
        &colors,
        bounds,
        None,
        SceneOverrides::default(),
    )
    .expect("two wires draw something");

    assert_eq!(scene.lines.len(), 2);
    assert_eq!(scene.lines[0].color, scene.lines[1].color);
    assert_eq!(scene.lines[0].width, scene.lines[1].width);
}
