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
    draw::page(&mut surface, &board, &mono, &frame, &[BoardLayerSet::Top]);
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
