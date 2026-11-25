//! #2883: a board drawing or string on copper is conductor copper of no
//! net on the copper image; on a legend it stays a drawing.

use signex_types::pcb::{BoardGraphic, BoardText, PcbBoard};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::fixture::{fixture_board, p};
use crate::fab::collect;
use crate::fab::layers::{CopperStack, FabLayer};
use crate::fab::marks::{Function, Mark};
use crate::fab::notes::Tally;
use crate::fab::{DrawnCopperFault, DrawnCopperItem, FabExtras, FabNote, FabOptions, generate};

/// The function and net of each stroke on `layer`'s image.
fn strokes(board: &PcbBoard, layer: FabLayer) -> Vec<(Function, Option<String>)> {
    let extras = FabExtras::default();
    let stack = CopperStack::of(board, collect::used_copper(board, &extras).into_iter());
    let mut tally = Tally::default();
    let images = collect::collect(board, &extras, &stack, &mut tally);
    assert_eq!(tally.skipped, 0);
    images
        .get(&layer)
        .into_iter()
        .flatten()
        .filter_map(|mark| match mark {
            Mark::Stroke {
                function,
                attributes,
                ..
            } => Some((*function, attributes.net.clone())),
            _ => None,
        })
        .collect()
}

fn drawing(id: u128, kind: &str, layer: &str) -> BoardGraphic {
    BoardGraphic {
        uuid: Uuid::from_u128(id),
        graphic_type: kind.into(),
        layer: layer.into(),
        width: 0.2,
        start: Some(p(1.0, 1.0)),
        end: Some(p(4.0, 3.0)),
        center: Some(p(1.0, 1.0)),
        radius: 2.0,
        points: Vec::new(),
        text: "LOGO".into(),
        font_size: 0.5,
    }
}

fn with_drawings(layer: &str) -> PcbBoard {
    let mut board = fixture_board();
    for (id, kind) in [(81, "rect"), (82, "circle"), (83, "text_frame")] {
        board.graphics.push(drawing(id, kind, layer));
    }
    board.texts.push(BoardText {
        uuid: Uuid::from_u128(84),
        text: "REV A".into(),
        position: p(2.0, 6.0),
        layer: layer.into(),
        font_size: 1.0,
        rotation: 0.0,
    });
    board
}

#[test]
fn drawings_and_strings_on_copper_are_conductor_of_no_net() {
    let top = FabLayer::Copper(CopperLayer::Top);
    let before = strokes(&fixture_board(), top);
    let after = strokes(&with_drawings("TopCopper"), top);
    let non_conductors = |s: &[(Function, Option<String>)]| {
        s.iter()
            .filter(|(f, _)| *f == Function::NonConductor)
            .count()
    };
    assert_eq!(non_conductors(&after), non_conductors(&before), "{after:?}");
    let added = after.len() - before.len();
    // The rectangle, circle and frame box, the frame's text and the string.
    assert!(added > 3, "{added}");
    let conductors = |s: &[(Function, Option<String>)]| {
        s.iter()
            .filter(|(f, net)| *f == Function::Conductor && net.is_none())
            .count()
    };
    assert_eq!(conductors(&after) - conductors(&before), added);
}

#[test]
fn drawings_and_strings_on_a_legend_stay_drawings() {
    let top = FabLayer::Copper(CopperLayer::Top);
    let board = with_drawings("TopSilkscreen");
    assert_eq!(strokes(&board, top), strokes(&fixture_board(), top));
    let legend = FabLayer::of_token("TopSilkscreen").expect("the top legend");
    let marks = strokes(&board, legend);
    assert!(!marks.is_empty());
    assert!(
        marks.iter().all(|(f, _)| *f == Function::Graphic),
        "{marks:?}"
    );
}

/// The fixture board with a good copper line, and optionally a copper
/// circle whose rim is not a number and a copper string whose turn is not
/// one: neither converts to copper.
fn with_good_and_bad(bad: bool) -> PcbBoard {
    let mut board = fixture_board();
    board.graphics.push(drawing(85, "line", "TopCopper"));
    if bad {
        let mut circle = drawing(86, "circle", "TopCopper");
        circle.end = Some(p(f64::NAN, 3.0));
        board.graphics.push(circle);
        board.texts.push(BoardText {
            uuid: Uuid::from_u128(87),
            text: "REV A".into(),
            position: p(2.0, 6.0),
            layer: "TopCopper".into(),
            font_size: 1.0,
            rotation: f64::NAN,
        });
    }
    board
}

fn skipped(item: DrawnCopperItem, field: &'static str) -> FabNote {
    FabNote::DrawnCopperSkipped {
        item,
        layer: CopperLayer::Top,
        fault: DrawnCopperFault::Geometry(pcb_geom::GeomError::Unrepresentable(field)),
    }
}

#[test]
fn copper_the_conversion_refuses_is_left_out_of_the_gerber_and_named() {
    let top = FabLayer::Copper(CopperLayer::Top);
    let good = strokes(&with_good_and_bad(false), top);
    assert!(good.len() > strokes(&fixture_board(), top).len());
    assert_eq!(
        strokes(&with_good_and_bad(true), top),
        good,
        "the good line only"
    );
    let gerber_only = FabOptions {
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: None,
        ..FabOptions::all("fab")
    };
    let board = with_good_and_bad(true);
    let set = generate(&board, &FabExtras::default(), &gerber_only).expect("generates");
    let circle = DrawnCopperItem::Drawing {
        id: Uuid::from_u128(86),
        kind: Some(signex_types::pcb_graphic::BoardGraphicKind::Circle),
    };
    let string = DrawnCopperItem::Text {
        id: Uuid::from_u128(87),
    };
    assert_eq!(
        set.notes,
        vec![
            skipped(circle, "graphic.radius"),
            skipped(string, "text.rotation")
        ]
    );
    // Every copper output skips them; each is named once.
    let all = generate(&board, &FabExtras::default(), &FabOptions::all("fab")).expect("generates");
    for note in [
        skipped(circle, "graphic.radius"),
        skipped(string, "text.rotation"),
    ] {
        assert_eq!(
            all.notes.iter().filter(|n| **n == note).count(),
            1,
            "{:?}",
            all.notes
        );
    }
}
