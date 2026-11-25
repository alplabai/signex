//! The Bezier and Pie kinds (#2838): the token they need, save and reload,
//! and what a placed symbol draws for them.

use signex_types::schematic::{Graphic, SymbolPrimitive, lower_symbol};

use super::*;

fn bezier() -> SymbolGraphicKind {
    SymbolGraphicKind::Bezier {
        points: [[0.0, 0.0], [1.27, 2.54], [3.81, 2.54], [5.08, 0.0]],
    }
}

fn pie() -> SymbolGraphicKind {
    SymbolGraphicKind::Pie {
        center: [1.27, 1.27],
        radius: 2.54,
        start_deg: 300.0,
        end_deg: 30.0,
    }
}

fn file_with(kind: SymbolGraphicKind, body_style: u8) -> SymbolFile {
    let mut s = Symbol::empty("C");
    s.graphics.push(SymbolGraphic {
        kind,
        stroke_width: 0.15,
        fill: None,
        part_number: 1,
        body_style,
    });
    SymbolFile::from_symbol(s)
}

#[test]
fn a_curve_kind_saves_as_v4_and_reloads_byte_identical() {
    for kind in [bezier(), pie()] {
        let text = file_with(kind.clone(), BODY_NORMAL)
            .to_toml_string()
            .unwrap();
        assert!(text.contains("format = \"snxsym/v4\""), "{text}");
        let back = SymbolFile::from_toml_str(&text).unwrap();
        assert_eq!(back.symbols[0].graphics[0].kind, kind);
        assert_eq!(back.to_toml_string().unwrap(), text);
    }
}

#[test]
fn a_curve_kind_on_the_alternate_body_keeps_its_tag_at_v4() {
    let text = file_with(pie(), BODY_ALTERNATE).to_toml_string().unwrap();
    assert!(text.contains("format = \"snxsym/v4\""), "{text}");
    let back = SymbolFile::from_toml_str(&text).unwrap();
    assert_eq!(back.symbols[0].graphics[0].body_style, BODY_ALTERNATE);
}

#[test]
fn a_file_that_loses_its_last_curve_drops_back_to_its_lower_token() {
    let mut file = file_with(bezier(), BODY_NORMAL);
    file.symbols[0].graphics[0].kind = SymbolGraphicKind::Line {
        from: [0.0, 0.0],
        to: [1.0, 1.0],
    };
    let text = file.to_toml_string().unwrap();
    assert!(text.contains("format = \"snxsym/v1\""), "{text}");
}

#[test]
fn a_pie_ring_starts_at_the_center_and_sweeps_counter_clockwise() {
    let ring = SymbolGraphicKind::Pie {
        center: [0.0, 0.0],
        radius: 2.0,
        start_deg: 350.0,
        end_deg: 10.0,
    }
    .flattened_outline();
    assert_eq!(ring[0], [0.0, 0.0]);
    let mid = ring[1 + (ring.len() - 2) / 2];
    assert!(
        (mid[0] - 2.0).abs() < 1e-9,
        "the short way through 0: {mid:?}"
    );
    assert!(mid[1].abs() < 1e-9, "{mid:?}");
}

#[test]
fn a_bezier_outline_runs_from_its_start_to_its_end() {
    let outline = bezier().flattened_outline();
    assert_eq!(outline.first(), Some(&[0.0, 0.0]));
    assert_eq!(outline.last(), Some(&[5.08, 0.0]));
}

fn elliptical_arc() -> SymbolGraphicKind {
    SymbolGraphicKind::EllipticalArc {
        center: [1.27, 1.27],
        radius_x: 5.08,
        radius_y: 2.54,
        start_deg: 45.0,
        end_deg: 200.0,
    }
}

fn round_rectangle() -> SymbolGraphicKind {
    SymbolGraphicKind::round_rectangle([7.62, 5.08], [0.0, 0.0])
}

#[test]
fn an_elliptical_arc_or_round_rectangle_saves_as_v4_and_reloads() {
    for kind in [elliptical_arc(), round_rectangle()] {
        let text = file_with(kind.clone(), BODY_NORMAL)
            .to_toml_string()
            .unwrap();
        assert!(text.contains("format = \"snxsym/v4\""), "{text}");
        let back = SymbolFile::from_toml_str(&text).unwrap();
        assert_eq!(back.symbols[0].graphics[0].kind, kind);
        assert_eq!(back.to_toml_string().unwrap(), text);
    }
}

#[test]
fn elliptical_arc_angles_are_geometric() {
    let (rx, ry) = (4.0_f64, 2.0_f64);
    let p = SymbolGraphicKind::point_on_ellipse([0.0, 0.0], rx, ry, 45.0);
    assert!((p[0] - p[1]).abs() < 1e-9, "on the 45 degree ray: {p:?}");
    let on_ellipse = (p[0] / rx).powi(2) + (p[1] / ry).powi(2);
    assert!((on_ellipse - 1.0).abs() < 1e-9, "{p:?}");
}

#[test]
fn an_elliptical_arc_outline_ends_on_its_two_angles() {
    let outline = elliptical_arc().flattened_outline();
    let at = |deg| SymbolGraphicKind::point_on_ellipse([1.27, 1.27], 5.08, 2.54, deg);
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9;
    assert!(close(outline[0], at(45.0)));
    assert!(close(outline[outline.len() - 1], at(200.0)));
}

#[test]
fn a_new_round_rectangle_takes_the_default_radius_clamped_to_its_short_side() {
    let SymbolGraphicKind::RoundRectangle {
        from,
        to,
        corner_radius,
    } = round_rectangle()
    else {
        panic!("expected a RoundRectangle");
    };
    assert_eq!((from, to), ([0.0, 0.0], [7.62, 5.08]));
    assert_eq!(corner_radius, SymbolGraphicKind::DEFAULT_CORNER_RADIUS_MM);
    let SymbolGraphicKind::RoundRectangle { corner_radius, .. } =
        SymbolGraphicKind::round_rectangle([0.0, 0.0], [7.62, 1.27])
    else {
        panic!("expected a RoundRectangle");
    };
    assert_eq!(corner_radius, 0.635);
}

#[test]
fn a_round_rectangle_ring_stays_inside_its_box_with_an_oversized_radius() {
    let ring = SymbolGraphicKind::RoundRectangle {
        from: [0.0, 0.0],
        to: [4.0, 2.0],
        corner_radius: 9.0,
    }
    .flattened_outline();
    assert!(ring.len() > 4);
    for p in &ring {
        assert!((-1e-9..=4.0 + 1e-9).contains(&p[0]), "{p:?}");
        assert!((-1e-9..=2.0 + 1e-9).contains(&p[1]), "{p:?}");
    }
}

#[test]
fn an_elliptical_arc_lowers_open_and_a_round_rectangle_closed() {
    let placed = signex_types::schematic::Symbol {
        lib_id: "L:C".to_string(),
        ..Default::default()
    };
    for (kind, expect_closed) in [(elliptical_arc(), false), (round_rectangle(), true)] {
        let lib = file_with(kind, BODY_NORMAL).symbols[0].to_lib_symbol("L:C");
        let drawn = lower_symbol(
            &lib,
            &placed,
            &signex_types::schematic::SchematicDefaults::BUILT_IN,
        );
        assert!(
            matches!(drawn.as_slice(), [SymbolPrimitive::Path { closed, .. }] if *closed == expect_closed),
            "{drawn:?}"
        );
    }
}

#[test]
fn a_bezier_lowers_to_the_schematic_bezier() {
    let lib = file_with(bezier(), BODY_NORMAL).symbols[0].to_lib_symbol("L:C");
    let Graphic::Bezier { points, .. } = &lib.graphics[0].graphic else {
        panic!("expected a Bezier, got {:?}", lib.graphics[0].graphic);
    };
    assert_eq!(points.len(), 4);
    assert_eq!((points[1].x, points[1].y), (1.27, 2.54));
}

#[test]
fn a_filled_pie_draws_as_a_closed_filled_path_on_a_placed_symbol() {
    let mut file = file_with(pie(), BODY_NORMAL);
    file.symbols[0].graphics[0].fill = Some([255, 0, 0, 255]);
    let lib = file.symbols[0].to_lib_symbol("L:C");
    let placed = signex_types::schematic::Symbol {
        lib_id: "L:C".to_string(),
        ..Default::default()
    };
    let drawn = lower_symbol(
        &lib,
        &placed,
        &signex_types::schematic::SchematicDefaults::BUILT_IN,
    );
    assert!(matches!(
        drawn.as_slice(),
        [SymbolPrimitive::Path {
            closed: true,
            fill: signex_types::schematic::FillType::Background,
            ..
        }]
    ));
}
