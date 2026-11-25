//! The block diagram on a printed page (#2796, mockup row 171).

use signex_types::block_diagram::{BlockDiagram, parse_block_diagram};

use super::*;
use crate::block_diagram::{BlockDiagramLook, block_diagram_drawing};

const DCDC: &str =
    include_str!("../../../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

fn drawing(diagram: &BlockDiagram, monochrome: bool) -> BlockDiagramDrawing {
    block_diagram_drawing(diagram, &BlockDiagramLook { monochrome })
}

fn dcdc() -> BlockDiagram {
    parse_block_diagram(DCDC)
        .expect("the corpus diagram reads")
        .diagram
}

fn text() -> BlockDiagramPdfText {
    BlockDiagramPdfText {
        title: "dcdc-buck.snxblk".into(),
        label: "Block diagram".into(),
    }
}

/// `/Type /Page` objects in the file, the page tree's `/Pages` left out.
fn page_objects(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page").count() - text.matches("/Type /Pages").count()
}

#[test]
fn the_diagram_is_one_fitted_page_with_its_marks_and_fonts() {
    let options = BlockDiagramPdfOptions::default();
    let out = block_diagram_pdf(&drawing(&dcdc(), false), &options, &text()).expect("pdf");
    assert!(out.bytes.starts_with(b"%PDF-"));
    assert_eq!(page_objects(&out.bytes), 1);
    assert!(out.scale > 0.0 && out.scale.is_finite());
    assert!(out.uncovered_chars.is_empty(), "{:?}", out.uncovered_chars);
    let file = String::from_utf8_lossy(&out.bytes);
    assert!(file.contains("/FontFile2"), "the page's font is embedded");
    assert!(file.contains("\nS\n"), "lines are stroked");
    assert!(file.contains("f*"), "blocks and arrows are filled");
}

#[test]
fn the_same_diagram_prints_the_same_bytes() {
    let options = BlockDiagramPdfOptions::default();
    let a = block_diagram_pdf(&drawing(&dcdc(), false), &options, &text()).expect("pdf");
    let b = block_diagram_pdf(&drawing(&dcdc(), false), &options, &text()).expect("pdf");
    assert_eq!(a.bytes, b.bytes);
}

#[test]
fn monochrome_prints_only_black_ink_on_paper() {
    let options = BlockDiagramPdfOptions {
        monochrome: true,
        ..BlockDiagramPdfOptions::default()
    };
    let out = block_diagram_pdf(&drawing(&dcdc(), true), &options, &text()).expect("pdf");
    let file = String::from_utf8_lossy(&out.bytes);
    for line in file.lines() {
        if line.ends_with(" rg") || line.ends_with(" RG") {
            let colour: Vec<&str> = line.split(' ').take(3).collect();
            assert!(
                colour == ["0", "0", "0"] || colour == ["1", "1", "1"],
                "only black ink and paper: {line}"
            );
        }
    }
}

#[test]
fn an_empty_diagram_and_no_title_block_still_print_a_page() {
    let options = BlockDiagramPdfOptions {
        title_block: false,
        ..BlockDiagramPdfOptions::default()
    };
    let empty = drawing(&BlockDiagram::new(), false);
    let out = block_diagram_pdf(&empty, &options, &BlockDiagramPdfText::default()).expect("pdf");
    assert_eq!(page_objects(&out.bytes), 1);
}
