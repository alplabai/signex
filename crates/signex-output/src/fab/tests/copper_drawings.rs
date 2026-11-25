//! #2883: a board drawing or string on copper is conductor copper of no
//! net on the copper image; on a legend it stays a drawing.

use signex_types::pcb::{BoardGraphic, BoardText, PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::fixture::{fixture_board, p};
use crate::fab::collect;
use crate::fab::frame::FabFrame;
use crate::fab::layers::{CopperStack, FabLayer};
use crate::fab::marks::{Attributes, Function, Mark, Step};
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

/// The Gerber copper and nothing else.
fn gerber_only() -> FabOptions {
    FabOptions {
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: None,
        ..FabOptions::all("fab")
    }
}

/// A copper `poly` or `polygon` drawing through `points`.
fn polygon(id: u128, kind: &str, points: Vec<Point>) -> BoardGraphic {
    BoardGraphic {
        points,
        ..drawing(id, kind, "TopCopper")
    }
}

/// The corners of `drawing`'s rectangle, in the order a rectangle is drawn.
fn corners() -> Vec<Point> {
    vec![p(1.0, 1.0), p(4.0, 1.0), p(4.0, 3.0), p(1.0, 3.0)]
}

/// The marks on the top copper image, and what was left out.
fn top_copper(board: &PcbBoard) -> (Vec<Mark>, Tally) {
    let extras = FabExtras::default();
    let stack = CopperStack::of(board, collect::used_copper(board, &extras).into_iter());
    let mut tally = Tally::default();
    let mut images = collect::collect(board, &extras, &stack, &mut tally);
    let marks = images
        .remove(&FabLayer::Copper(CopperLayer::Top))
        .unwrap_or_default();
    (marks, tally)
}

fn top_gerber(board: &PcbBoard) -> (String, Vec<FabNote>) {
    let set = generate(board, &FabExtras::default(), &gerber_only()).expect("generates");
    let gerber = super::file(&set, "fab-copper-l1-top.gbr")
        .expect("the top copper is written")
        .to_owned();
    (gerber, set.notes)
}

/// A polygon on copper stays in the Gerber as the closed outline at its
/// width it always was: the same bytes as the rectangle on its corners,
/// whose drawing the shared conversion never changed.
#[test]
fn a_copper_polygon_is_written_as_its_closed_outline_as_before() {
    let (before, _) = top_copper(&fixture_board());
    let mut rect = fixture_board();
    rect.graphics.push(drawing(88, "rect", "TopCopper"));
    let (rect_gerber, _) = top_gerber(&rect);
    for kind in ["poly", "polygon"] {
        let mut board = fixture_board();
        board.graphics.push(polygon(88, kind, corners()));
        let frame = FabFrame::of(&board);
        let at: Vec<_> = corners()
            .into_iter()
            .map(|c| frame.of_mm(c, "corner").expect("in range"))
            .collect();
        let outline = Mark::Stroke {
            width: 200_000,
            function: Function::Conductor,
            start: at[0],
            steps: vec![
                Step::Line(at[1]),
                Step::Line(at[2]),
                Step::Line(at[3]),
                Step::Line(at[0]),
            ],
            attributes: Attributes::default(),
        };
        let (marks, tally) = top_copper(&board);
        assert_eq!(tally.skipped, 0, "{kind}");
        assert!(tally.drawn_copper_skipped.is_empty(), "{kind}");
        assert_eq!(marks.len(), before.len() + 1, "{kind}");
        assert_eq!(
            marks.iter().filter(|m| **m == outline).count(),
            1,
            "{kind}: {marks:?}"
        );
        let (gerber, notes) = top_gerber(&board);
        assert!(notes.is_empty(), "{kind}: {notes:?}");
        assert_eq!(gerber, rect_gerber, "{kind}");
        assert_ne!(gerber, top_gerber(&fixture_board()).0, "{kind}");
    }
}

#[test]
fn a_copper_polygon_missing_or_with_bad_points_is_left_out_and_named() {
    let top = FabLayer::Copper(CopperLayer::Top);
    let named = |id: u128, fault: DrawnCopperFault| FabNote::DrawnCopperSkipped {
        item: DrawnCopperItem::Drawing {
            id: Uuid::from_u128(id),
            kind: None,
        },
        layer: CopperLayer::Top,
        fault,
    };
    for kind in ["poly", "polygon"] {
        let mut board = fixture_board();
        board.graphics.push(polygon(89, kind, vec![p(1.0, 1.0)]));
        let mut points = corners();
        points[2] = p(f64::NAN, 3.0);
        board.graphics.push(polygon(90, kind, points));
        assert_eq!(
            strokes(&board, top),
            strokes(&fixture_board(), top),
            "{kind}"
        );
        let (_, notes) = top_gerber(&board);
        assert_eq!(
            notes,
            vec![
                named(89, DrawnCopperFault::MissingPoints),
                named(
                    90,
                    DrawnCopperFault::Geometry(pcb_geom::GeomError::Unrepresentable(
                        "graphic.points"
                    ))
                ),
            ],
            "{kind}"
        );
    }
}

/// A copper circle whose rim and saved radius disagree has one radius: the
/// rim's, in the design rule check's copper and in the Gerber alike.
#[test]
fn a_circles_rim_sets_its_radius_in_the_drc_copper_and_the_gerber() {
    let mut circle = drawing(91, "circle", "TopCopper");
    circle.end = Some(p(4.0, 5.0));
    circle.radius = 2.0;
    let rim_nm = 5_000_000;

    let shapes = pcb_geom::board_graphic_copper(&circle).expect("converts");
    let Some(pcb_geom::Shape::Capsule { a, .. }) = shapes.first() else {
        panic!("{shapes:?}");
    };
    assert_eq!(a.x - 1_000_000, rim_nm, "the first chord starts at angle 0");

    let (before, _) = top_copper(&fixture_board());
    let mut board = fixture_board();
    board.graphics.push(circle);
    let (marks, tally) = top_copper(&board);
    assert!(tally.drawn_copper_skipped.is_empty());
    let added: Vec<_> = marks.iter().filter(|m| !before.contains(m)).collect();
    let [
        Mark::Stroke {
            start,
            steps,
            function: Function::Conductor,
            ..
        },
    ] = added.as_slice()
    else {
        panic!("{added:?}");
    };
    let [Step::Arc { to, center, .. }] = steps.as_slice() else {
        panic!("{steps:?}");
    };
    assert_eq!(to, start, "a full circle");
    assert_eq!(start.x - center.x, rim_nm);
}
