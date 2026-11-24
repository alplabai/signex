//! #2883: a board drawing or string on copper is conductor copper of no
//! net on the copper image; on a legend it stays a drawing.

use signex_types::pcb::{BoardGraphic, BoardText, PcbBoard};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::fixture::{fixture_board, p};
use crate::fab::FabExtras;
use crate::fab::collect;
use crate::fab::layers::{CopperStack, FabLayer};
use crate::fab::marks::{Function, Mark};
use crate::fab::notes::Tally;

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
