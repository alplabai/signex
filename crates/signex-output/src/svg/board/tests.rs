use signex_types::pcb_layer::BoardSide;

use super::*;
use crate::assembly::assembly_drawing;
use crate::assembly::tests::{check_golden, real_board};

fn drawing(marks: Vec<AssemblyMark>) -> AssemblyDrawing {
    AssemblyDrawing {
        side: BoardSide::Top,
        min: Point::new(0.0, 0.0),
        max: Point::new(10.0, 5.0),
        marks,
    }
}

#[test]
fn the_page_is_the_board_and_its_margin_in_mm() {
    let svg = assembly_svg(&drawing(Vec::new()), &AssemblySvgOptions::default());
    assert!(svg.starts_with("<?xml"));
    assert!(svg.contains("width=\"14mm\" height=\"9mm\" viewBox=\"0 0 14 9\""));
    assert!(svg.trim_end().ends_with("</svg>"));
}

#[test]
fn a_mirrored_page_reverses_x_and_the_turn_of_a_text() {
    let marks = vec![
        AssemblyMark::Stroke {
            points: vec![Point::new(1.0, 1.0), Point::new(3.0, 1.0)],
            closed: false,
            width_mm: 0.0,
            ink: AssemblyInk::Fab,
        },
        AssemblyMark::Text {
            at: Point::new(1.0, 2.0),
            rotation_deg: 30.0,
            size_mm: 1.0,
            text: "R<1>".to_string(),
            ink: AssemblyInk::Designator,
        },
    ];
    let plain = assembly_svg(&drawing(marks.clone()), &AssemblySvgOptions::default());
    assert!(plain.contains("d=\"M3 3 L5 3\""), "{plain}");
    assert!(plain.contains("stroke-width=\"0.05\""), "a hairline shows");
    assert!(plain.contains("rotate(30 3 4)"), "{plain}");
    assert!(plain.contains(">R&lt;1&gt;</text>"), "text is escaped");
    let mirrored = AssemblySvgOptions {
        mirrored: true,
        monochrome: true,
    };
    let flipped = assembly_svg(&drawing(marks), &mirrored);
    assert!(flipped.contains("d=\"M11 3 L9 3\""), "{flipped}");
    assert!(flipped.contains("rotate(-30 11 4)"), "{flipped}");
    assert!(!flipped.contains("#c71f1f"), "monochrome has no colour");
}

#[test]
fn numbers_are_written_to_the_micrometre() {
    assert_eq!(num(1.23456), "1.235");
    assert_eq!(num(-0.0001), "0");
    assert_eq!(num(2.0), "2");
    assert_eq!(num(f64::NAN), "0");
}

/// The top and bottom drawings of a real board, the bottom mirrored as the
/// board PDF prints it by default.
#[test]
fn the_real_board_assembly_svgs_match_their_goldens() {
    let board = real_board();
    for (side, name, mirrored) in [
        (BoardSide::Top, "mixed-tht-smd-top.svg", false),
        (BoardSide::Bottom, "mixed-tht-smd-bottom.svg", true),
    ] {
        let options = AssemblySvgOptions {
            mirrored,
            monochrome: false,
        };
        check_golden(
            name,
            &assembly_svg(&assembly_drawing(&board, side), &options),
        );
    }
}
