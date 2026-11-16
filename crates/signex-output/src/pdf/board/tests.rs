use super::*;
use crate::board_report::tests::fixture;

fn options(sets: &[BoardLayerSet], pages: BoardPrintPages) -> BoardPrintOptions {
    BoardPrintOptions {
        sets: sets.to_vec(),
        pages,
        ..BoardPrintOptions::default()
    }
}

/// `/Type /Page` objects in the file, the page tree's `/Pages` left out.
fn page_objects(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn one_page_per_chosen_set() {
    let sets = [
        BoardLayerSet::Top,
        BoardLayerSet::Bottom,
        BoardLayerSet::Mechanical,
    ];
    let out = board_pdf(
        &fixture(),
        &options(&sets, BoardPrintPages::PerSet),
        &BoardPrintText::default(),
    )
    .expect("pdf");
    assert!(out.bytes.starts_with(b"%PDF-"));
    assert_eq!(out.pages.len(), 3);
    assert_eq!(page_objects(&out.bytes), 3);
    let printed: Vec<Vec<BoardLayerSet>> = out.pages.iter().map(|p| p.sets.clone()).collect();
    assert_eq!(
        printed,
        [
            vec![BoardLayerSet::Top],
            vec![BoardLayerSet::Bottom],
            vec![BoardLayerSet::Mechanical],
        ]
    );
    let mirrored: Vec<bool> = out.pages.iter().map(|p| p.mirrored).collect();
    assert_eq!(
        mirrored,
        [false, true, false],
        "the bottom page reads from below"
    );
}

#[test]
fn a_composite_print_is_one_page_with_every_set() {
    let sets = [BoardLayerSet::Bottom, BoardLayerSet::Top];
    let out = board_pdf(
        &fixture(),
        &options(&sets, BoardPrintPages::Composite),
        &BoardPrintText::default(),
    )
    .expect("pdf");
    assert_eq!(out.pages.len(), 1);
    assert_eq!(page_objects(&out.bytes), 1);
    assert_eq!(
        out.pages[0].sets,
        [BoardLayerSet::Top, BoardLayerSet::Bottom],
        "sets print in their fixed order"
    );
    assert!(!out.pages[0].mirrored);
}

#[test]
fn a_one_to_one_page_measures_the_board_size() {
    let print = BoardPrintOptions {
        scale: BoardPrintScale::OneToOne,
        ..options(&[BoardLayerSet::Top], BoardPrintPages::PerSet)
    };
    let board = fixture();
    let out = board_pdf(&board, &print, &BoardPrintText::default()).expect("pdf");
    assert_eq!(out.pages[0].scale, 1.0);
    assert_eq!(out.pages[0].printed_size_mm, (50.0, 30.0));
    // The outline's corners land 50 mm and 30 mm apart on the page.
    let page = print.page_size.dimensions_mm(print.orientation);
    let frame = PageFrame::new(&board, &print, page, false);
    let (x0, y0) = frame.to_pt(0.0, 0.0);
    let (x1, y1) = frame.to_pt(50.0, 30.0);
    let mm = |pt: f32| f64::from(pt) / MM_TO_PT;
    assert!((mm(x1 - x0) - 50.0).abs() < 1e-3, "{}", mm(x1 - x0));
    assert!((mm(y0 - y1) - 30.0).abs() < 1e-3, "{}", mm(y0 - y1));
}

#[test]
fn fit_fills_the_page_and_mirror_swaps_left_and_right() {
    let print = options(&[BoardLayerSet::Bottom], BoardPrintPages::PerSet);
    let board = fixture();
    let page = print.page_size.dimensions_mm(print.orientation);
    let flat = PageFrame::new(&board, &print, page, false);
    let mirrored = PageFrame::new(&board, &print, page, true);
    assert!(flat.scale() > 1.0, "a 50 mm board grows to fill A4");
    assert!(flat.to_pt(0.0, 0.0).0 < flat.to_pt(50.0, 0.0).0);
    assert!(mirrored.to_pt(0.0, 0.0).0 > mirrored.to_pt(50.0, 0.0).0);
    assert_eq!(flat.to_pt(0.0, 0.0), mirrored.to_pt(50.0, 0.0));
}

#[test]
fn monochrome_draws_every_layer_in_black() {
    let mono = BoardPrintOptions {
        monochrome: true,
        title_block: false,
        ..options(&[BoardLayerSet::Top], BoardPrintPages::PerSet)
    };
    let board = fixture();
    let page = mono.page_size.dimensions_mm(mono.orientation);
    let frame = PageFrame::new(&board, &mono, page, false);
    let mut fonts = crate::pdf::embed::FontUsage::new();
    let mut surface = PdfSurface::new(&mut fonts);
    let fills = pcb_geom::zone_fills(&board);
    draw::page(
        &mut surface,
        &board,
        &fills,
        &mono,
        &frame,
        &[BoardLayerSet::Top],
    );
    let ops = String::from_utf8(surface.finish()).expect("ops");
    for line in ops.lines() {
        if line.ends_with(" rg") || line.ends_with(" RG") {
            let colour: Vec<&str> = line.split(' ').take(3).collect();
            assert!(
                colour == ["0", "0", "0"] || colour == ["1", "1", "1"],
                "only black ink and paper holes: {line}"
            );
        }
    }
    assert!(
        ops.contains(" re\n") || ops.contains(" l\n"),
        "something was drawn"
    );
}

#[test]
fn no_set_is_refused() {
    let none = options(&[], BoardPrintPages::PerSet);
    let result = board_pdf(&fixture(), &none, &BoardPrintText::default());
    assert!(matches!(result, Err(BoardPdfError::NoLayerSets)));
}

#[test]
fn the_scale_reads_as_a_ratio() {
    assert_eq!(draw::scale_text(1.0), "1:1");
    assert_eq!(draw::scale_text(0.4), "1:2.5");
    assert_eq!(draw::scale_text(2.0), "2:1");
}

#[test]
fn each_set_holds_its_layers() {
    use signex_types::pcb_layer::{CopperLayer, InnerCopper, TechnicalLayer};
    let top = PcbLayer::Copper(CopperLayer::Top);
    let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(1).expect("inner")));
    let silk_bottom = PcbLayer::Technical(TechnicalLayer::SilkscreenBottom);
    let edge = PcbLayer::Technical(TechnicalLayer::EdgeCuts);
    assert!(BoardLayerSet::Top.contains(top));
    assert!(!BoardLayerSet::Top.contains(inner));
    assert!(BoardLayerSet::Inner.contains(inner));
    assert!(BoardLayerSet::Bottom.contains(silk_bottom));
    assert!(BoardLayerSet::Mechanical.contains(edge));
    assert!(!BoardLayerSet::Mechanical.contains(top));
    for set in BoardLayerSet::ALL {
        assert_eq!(BoardLayerSet::from_id(set.id()), Some(set));
    }
}

fn zone(fill_type: &str, net: u32, from: (f64, f64), to: (f64, f64)) -> signex_types::pcb::Zone {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::now_v7(),
        "net": net,
        "layer": "TopCopper",
        "fill_type": fill_type,
        "outline": [
            { "x": from.0, "y": from.1 },
            { "x": to.0, "y": from.1 },
            { "x": to.0, "y": to.1 },
            { "x": from.0, "y": to.1 },
        ],
    }))
    .expect("zone")
}

/// The operators the top set's zones of `board` draw.
fn zone_ops(board: &PcbBoard) -> String {
    let print = options(&[BoardLayerSet::Top], BoardPrintPages::PerSet);
    let page = print.page_size.dimensions_mm(print.orientation);
    let frame = PageFrame::new(board, &print, page, false);
    let fills = pcb_geom::zone_fills(board);
    let mut fonts = crate::pdf::embed::FontUsage::new();
    let mut surface = PdfSurface::new(&mut fonts);
    let black = (0.0, 0.0, 0.0);
    draw::draw_zones(
        &mut surface,
        board,
        &fills,
        &frame,
        BoardLayerSet::Top,
        black,
    );
    String::from_utf8(surface.finish()).expect("ops")
}

#[test]
fn a_pour_prints_its_fill_clear_of_other_nets() {
    // VCC pour over R10, whose GND pad must stay clear of it.
    let mut board = fixture();
    board.zones.push(zone("pour", 1, (25.0, 5.0), (35.0, 15.0)));
    let fills = pcb_geom::zone_fills(&board);
    let pieces = &fills.of(0).expect("the pour fills").pieces;
    assert!(
        pieces.iter().any(|p| !p.holes.is_empty()),
        "the GND pad is cut out"
    );
    let rings: usize = pieces.iter().map(|p| 1 + p.holes.len()).sum();
    let ops = zone_ops(&board);
    assert_eq!(ops.matches("f*\n").count(), pieces.len(), "{ops}");
    assert_eq!(
        ops.lines().filter(|l| l.ends_with(" m")).count(),
        rings + 1,
        "the fill's rings, then the outline stroked"
    );
}

#[test]
fn a_keepout_prints_its_outline_not_copper() {
    let mut board = fixture();
    board
        .zones
        .push(zone("keepout", 0, (40.0, 20.0), (45.0, 25.0)));
    let ops = zone_ops(&board);
    assert!(!ops.contains("f*"), "no copper: {ops}");
    assert!(ops.contains("S\n"), "the outline is stroked: {ops}");
}

#[test]
fn a_solid_zone_still_prints_filled() {
    let mut board = fixture();
    board
        .zones
        .push(zone("solid", 1, (40.0, 20.0), (45.0, 25.0)));
    let ops = zone_ops(&board);
    assert_eq!(ops.matches("f*\n").count(), 1, "{ops}");
}

#[test]
fn a_one_to_one_board_larger_than_the_page_is_flagged() {
    let mut board = fixture();
    for point in board.outline.iter_mut() {
        point.x *= 8.0;
        point.y *= 10.0;
    }
    let mut print = options(&[BoardLayerSet::Top], BoardPrintPages::PerSet);
    let fitted = board_pdf(&board, &print, &BoardPrintText::default()).expect("pdf");
    assert!(
        !fitted.pages[0].overflows,
        "a fitted board stays on the page"
    );
    print.scale = BoardPrintScale::OneToOne;
    let full = board_pdf(&board, &print, &BoardPrintText::default()).expect("pdf");
    assert!(full.pages[0].overflows, "400 mm x 300 mm runs off A4");
    let small = board_pdf(&fixture(), &print, &BoardPrintText::default()).expect("pdf");
    assert!(!small.pages[0].overflows, "50 mm x 30 mm fits A4");
}
