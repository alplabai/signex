//! #2663: contents rows as PDF links, and the drawn text of a contents table
//! and a pin table against their golden (the SCH Tables & Docs exit
//! criterion: tables export to PDF and match their golden output).

use std::path::PathBuf;

use signex_types::schematic::{PinSort, Point, SchDrawing, SchTable, SchematicSheet, TableSource};

use super::*;
use crate::pdf::{PageRange, PdfExporter};
use crate::svg::{SvgElement, SvgRenderContext};
use crate::{ExportContext, Exporter, ProjectMetadata, SheetSnapshot};

fn page(path: &str, sheet: SchematicSheet, number: usize) -> SheetSnapshot {
    SheetSnapshot {
        path: PathBuf::from(path),
        schematic: sheet,
        sheet_name: format!("Sheet{number}"),
        sheet_number: number,
        sheet_count: 3,
        placement: Default::default(),
    }
}

fn ctx(sheets: Vec<SheetSnapshot>) -> ExportContext {
    ExportContext {
        sheets,
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    }
}

/// A contents table at (20, 20) with `rows` of (cells, link).
fn contents_table(rows: &[([&str; 5], &str)]) -> SchTable {
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(1), Point::new(20.0, 20.0));
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = rows
        .iter()
        .map(|(cells, _)| cells.iter().map(|c| c.to_string()).collect())
        .collect();
    table.links = rows.iter().map(|(_, link)| link.to_string()).collect();
    table
}

fn sheet_with(table: SchTable) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.drawings.push(SchDrawing::Table(table));
    sheet
}

/// A pin table for U1 with typed rows (a refresh is the app's; the PDF
/// draws what the table holds).
fn pin_table() -> SchTable {
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(2), Point::new(20.0, 80.0));
    table.source = TableSource::ComponentPins {
        designator: "U1".into(),
        sort: PinSort::Number,
    };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = vec![
        vec!["1", "BOOT", "SW", "Passive", "", "2.54"],
        vec!["2", "VIN", "+12V", "Power input", "", "2.54"],
    ]
    .into_iter()
    .map(|r| r.into_iter().map(str::to_string).collect())
    .collect();
    table
}

/// contents.snxsch (the contents table: itself, the buck sheet, and a sheet
/// no page prints) and power/buck.snxsch (a pin table).
fn project() -> ExportContext {
    let contents = contents_table(&[
        (
            ["1", "Contents", "Contents", "A", "2026-11-20"],
            "contents.snxsch",
        ),
        (
            ["2", "Power", "Buck converter", "B", "2026-11-21"],
            "power/buck.snxsch",
        ),
        (["3", "Old", "Removed sheet", "C", ""], "gone.snxsch"),
    ]);
    ctx(vec![
        page("/p/contents.snxsch", sheet_with(contents), 1),
        page("/p/power/buck.snxsch", sheet_with(pin_table()), 2),
    ])
}

fn opts() -> PdfOptions {
    PdfOptions {
        include_title_block: false,
        ..Default::default()
    }
}

#[test]
fn each_contents_row_links_to_the_page_that_prints_its_sheet() {
    let links = pdf_table_links(&project(), &opts()).expect("links");
    let found: Vec<(usize, usize, &str, usize)> = links
        .iter()
        .map(|l| (l.page, l.row, l.link.as_str(), l.target_page))
        .collect();
    assert_eq!(
        found,
        vec![(0, 0, "contents.snxsch", 0), (0, 1, "power/buck.snxsch", 1)],
        "a row whose sheet no page prints stays plain text"
    );
}

#[test]
fn a_link_climbing_out_of_the_folder_reaches_its_page() {
    let back = contents_table(&[(["1", "Top", "", "", ""], "../contents.snxsch")]);
    let ctx = ctx(vec![
        page("/p/contents.snxsch", SchematicSheet::default(), 1),
        page("/p/power/buck.snxsch", sheet_with(back), 2),
    ]);
    let links = pdf_table_links(&ctx, &opts()).expect("links");
    assert_eq!(links.len(), 1);
    assert_eq!((links[0].page, links[0].target_page), (1, 0));
}

#[test]
fn the_rows_of_a_repeated_sheet_go_to_its_pages_in_order() {
    let rows = [
        (["2", "buck · CH1", "", "", ""], "buck.snxsch"),
        (["3", "buck · CH2", "", "", ""], "buck.snxsch"),
        (["4", "buck · CH3", "", "", ""], "buck.snxsch"),
    ];
    let ctx = ctx(vec![
        page("/p/contents.snxsch", sheet_with(contents_table(&rows)), 1),
        page("/p/buck.snxsch", SchematicSheet::default(), 2),
        page("/p/buck.snxsch", SchematicSheet::default(), 3),
    ]);
    let targets: Vec<usize> = pdf_table_links(&ctx, &opts())
        .expect("links")
        .iter()
        .map(|l| l.target_page)
        .collect();
    assert_eq!(
        targets,
        vec![1, 2, 1],
        "a row past the last page goes to the first"
    );
}

#[test]
fn a_link_sits_over_its_row_inside_the_page() {
    let options = opts();
    let links = pdf_table_links(&project(), &options).expect("links");
    let (w_mm, h_mm) = options.page_size.dimensions_mm(options.orientation);
    let (w_pt, h_pt) = ((w_mm * MM_TO_PT) as f32, (h_mm * MM_TO_PT) as f32);
    for link in &links {
        let [left, bottom, right, top] = link.rect_pt;
        assert!(0.0 <= left && left < right && right <= w_pt, "{link:?}");
        assert!(0.0 <= bottom && bottom < top && top <= h_pt, "{link:?}");
    }
    let (first, second) = (links[0].rect_pt, links[1].rect_pt);
    assert!(
        (first[1] - second[3]).abs() < 1e-3,
        "row 2 starts where row 1 ends"
    );
}

#[test]
fn a_page_range_without_the_target_leaves_the_row_plain() {
    let options = PdfOptions {
        page_range: PageRange::Specific(vec![1]),
        ..opts()
    };
    let links = pdf_table_links(&project(), &options).expect("links");
    let found: Vec<&str> = links.iter().map(|l| l.link.as_str()).collect();
    assert_eq!(found, vec!["contents.snxsch"]);
}

#[test]
fn the_exported_pdf_carries_one_goto_link_per_row() {
    let output = PdfExporter.export(&project(), &opts()).expect("pdf");
    let text = String::from_utf8_lossy(&output.bytes);
    assert_eq!(output.table_links.len(), 2);
    assert_eq!(text.matches("/Subtype /Link").count(), 2, "{text}");
    assert_eq!(text.matches("/S /GoTo").count(), 2);
    assert_eq!(text.matches("/Annots").count(), 1, "only page 1 has links");
}

#[test]
fn a_pdf_with_no_linked_rows_has_no_annotations() {
    let output = PdfExporter
        .export(&crate::pdf::tests::sample_ctx(2), &opts())
        .expect("pdf");
    let text = String::from_utf8_lossy(&output.bytes);
    assert!(output.table_links.is_empty());
    assert!(!text.contains("/Annots"));
}

/// The table text each page draws (header then rows, column by column, as
/// the PDF writes it) and the links, as the golden lists them.
fn golden_text(ctx: &ExportContext, options: &PdfOptions) -> String {
    let (w_mm, h_mm) = options.page_size.dimensions_mm(options.orientation);
    let mut out = String::new();
    for (index, sheet) in ctx.sheets.iter().enumerate() {
        out.push_str(&format!("# page {}\n", index + 1));
        let drawn = SvgRenderContext::from_sheet(sheet, options, w_mm, h_mm, MM_TO_PT, None);
        for element in &drawn.elements {
            if let SvgElement::Text {
                font_alias: "F3",
                text,
                ..
            } = element
            {
                out.push_str(text);
                out.push('\n');
            }
        }
    }
    out.push_str("# links\npage\trow\tlink\ttarget_page\n");
    for link in pdf_table_links(ctx, options).expect("links") {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            link.page + 1,
            link.row + 1,
            link.link,
            link.target_page + 1
        ));
    }
    out
}

/// Compare with `testdata/tables/<name>`; `UPDATE_GOLDEN=1` writes it. A
/// diff is a change to what the PDF prints, not a snapshot to refresh.
fn check_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/tables")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("golden folder");
        std::fs::write(&path, actual).expect("golden written");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run with UPDATE_GOLDEN=1", path.display()))
        .replace("\r\n", "\n");
    assert!(
        expected == actual,
        "{} differs:\n{actual}\nregenerate with UPDATE_GOLDEN=1 cargo test -p \
         signex-output table_links if the change is meant",
        path.display()
    );
}

#[test]
fn the_contents_and_pin_tables_print_their_golden_text_and_links() {
    check_golden(
        "contents_and_pins.golden.txt",
        &golden_text(&project(), &opts()),
    );
}

/// `links: false` writes no link at all, the contents rows' included, and
/// answers none (#1298, #2663; DECISIONS 2026-11-27).
#[test]
fn links_off_drops_the_contents_rows_links() {
    let options = PdfOptions {
        links: false,
        ..opts()
    };
    assert!(
        pdf_table_links(&project(), &options)
            .expect("links")
            .is_empty()
    );
    let output = PdfExporter.export(&project(), &options).expect("pdf");
    let text = String::from_utf8_lossy(&output.bytes);
    assert!(output.table_links.is_empty());
    assert!(!text.contains("/Annots"), "{text}");
    assert!(!text.contains("/Subtype /Link"), "{text}");
}

/// The contents rows' links and a text link on the same page share the
/// page's one `/Annots` array, each a distinct annotation object.
#[test]
fn the_contents_rows_share_the_pages_one_annots_array_with_the_other_links() {
    let mut ctx = project();
    let note: signex_types::schematic::TextNote = serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(9),
        "text": "[site](https://example.com)",
        "position": { "x": 120.0, "y": 20.0 },
        "font_size": 2.54,
    }))
    .expect("text note");
    ctx.sheets[0].schematic.text_notes.push(note);
    let output = PdfExporter.export(&ctx, &opts()).expect("pdf");
    let text = String::from_utf8_lossy(&output.bytes);
    assert_eq!(output.table_links.len(), 2);
    assert_eq!(text.matches("/Annots").count(), 1, "one array, on page 1");
    assert_eq!(text.matches("/Subtype /Link").count(), 3, "{text}");
    assert_eq!(text.matches("/S /GoTo").count(), 2);
    assert!(text.contains("/URI (https://example.com)"), "{text}");
    let annots = &text[text.find("/Annots [").expect("the array")..];
    let annots = &annots[..annots.find(']').expect("closed")];
    assert_eq!(annots.matches(" 0 R").count(), 3, "{annots}");
}
