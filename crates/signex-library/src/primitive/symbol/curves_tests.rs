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
