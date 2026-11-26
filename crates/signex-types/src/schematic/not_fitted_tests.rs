//! #1294 — the not-fitted mark: greyed ink and the cross over the body.

use super::*;
use crate::schematic::{FillType, Graphic, LibGraphic};

fn rect_lib() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:R" })).unwrap();
    lib.graphics = vec![LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 4.0),
            width: 0.25,
            fill: FillType::None,
        },
    }];
    lib
}

#[test]
fn the_cross_runs_corner_to_corner_of_the_body_box() {
    let sym = Symbol {
        position: Point::new(100.0, 100.0),
        ..Symbol::default()
    };
    let [a, b] = not_fitted_cross(&rect_lib(), &sym).expect("a body");
    // Y-up library y 0..4 is world y 96..100 (Y-down).
    assert_eq!(a, (Point::new(100.0, 96.0), Point::new(110.0, 100.0)));
    assert_eq!(b, (Point::new(100.0, 100.0), Point::new(110.0, 96.0)));
}

#[test]
fn a_symbol_with_no_body_gets_no_cross() {
    let lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:E" })).unwrap();
    assert_eq!(not_fitted_cross(&lib, &Symbol::default()), None);
}

#[test]
fn greyed_ink_moves_toward_the_paper_and_stops_short_of_it() {
    let ink = [0.0, 0.0, 0.0];
    let paper = [1.0, 1.0, 1.0];
    let grey = greyed_ink(ink, paper);
    for channel in grey {
        assert!((channel - NOT_FITTED_PAPER_SHARE).abs() < 1e-6, "{grey:?}");
    }
    assert_eq!(greyed_ink(paper, paper), paper, "paper stays paper");
}
