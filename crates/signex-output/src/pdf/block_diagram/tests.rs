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

fn table(rows: usize) -> BlockDiagramTable {
    BlockDiagramTable {
        heading: "Budget table".into(),
        columns: vec!["Rail".into(), "Load".into()],
        rows: (0..rows)
            .map(|i| BlockDiagramTableRow {
                cells: vec![format!("RAIL_{i}"), "0.10 A".into()],
                indent: (i % 2) as u8,
                strong: i % 2 == 0,
            })
            .collect(),
    }
}

/// Mockup row 171's Pages: the diagram, then each table on as many pages
/// as its rows need, on the paper asked for.
#[test]
fn pages_print_in_order_and_a_long_table_runs_on() {
    let a3 = BlockDiagramPdfOptions {
        page_size: crate::pdf::PageSize::IsoA3,
        ..BlockDiagramPdfOptions::default()
    };
    let diagram = drawing(&dcdc(), false);
    let (short, long) = (table(3), table(200));
    let pages = [
        BlockDiagramPage::Diagram(&diagram),
        BlockDiagramPage::Table(&short),
        BlockDiagramPage::Table(&long),
    ];
    let out = block_diagram_pdf_pages(&pages, &a3, &text()).expect("pdf");
    let per_page = tables::rows_per_page(a3.page_size.dimensions_mm(a3.orientation));
    assert_eq!(page_objects(&out.bytes), 2 + 200usize.div_ceil(per_page));
    let media = |bytes: &[u8]| {
        let file = String::from_utf8_lossy(bytes).into_owned();
        let at = file.find("/MediaBox").expect("a media box");
        file[at..].lines().next().unwrap_or_default().to_string()
    };
    let a4 = block_diagram_pdf(&diagram, &BlockDiagramPdfOptions::default(), &text()).expect("a4");
    assert_ne!(media(&out.bytes), media(&a4.bytes), "the paper asked for");

    let tables_only = [BlockDiagramPage::Table(&short)];
    let out = block_diagram_pdf_pages(&tables_only, &a3, &text()).expect("pdf");
    assert_eq!(page_objects(&out.bytes), 1);
    assert_eq!(out.scale, 1.0);
}
