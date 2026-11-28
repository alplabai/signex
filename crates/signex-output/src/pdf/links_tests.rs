//! Tests for the PDF link annotations (#1298) and the watermark colour
//! (#2609).
use signex_types::schematic::{
    ChildSheet, FillType, Graphic, InstancePath, Label, LabelType, LibGraphic, LibSymbol, Point,
    SheetPin, Symbol, TextNote,
};
use uuid::Uuid;

use super::links::{LinkPages, LinkTarget, PageLink, linkable_uri};
use super::optional_content_tests::page_ops;
use super::tests::sample_ctx;
use super::*;

const A4: (f64, f64) = (297.0, 210.0);

fn pdf_text(ctx: &ExportContext, opts: &PdfOptions) -> String {
    let out = PdfExporter.export(ctx, opts).expect("export");
    String::from_utf8_lossy(&out.bytes).into_owned()
}

fn note(text: &str) -> TextNote {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::nil(),
        "text": text,
        "position": { "x": 20.0, "y": 20.0 },
        "font_size": 2.54,
    }))
    .expect("text note")
}

fn links_of(ctx: &ExportContext, page: usize) -> Vec<PageLink> {
    let pages: Vec<usize> = (0..ctx.sheets.len()).collect();
    LinkPages::new(ctx, &PdfOptions::default(), &pages, A4).sheet_links(page)
}

fn goes_to_page(link: &PageLink) -> Option<usize> {
    match link.target {
        LinkTarget::Page { page_idx, .. } => Some(page_idx),
        LinkTarget::Uri(_) => None,
    }
}

#[test]
fn only_web_and_mail_addresses_are_linkable() {
    assert_eq!(
        linkable_uri(" https://example.com/ds.pdf "),
        Some("https://example.com/ds.pdf")
    );
    assert_eq!(
        linkable_uri("HTTP://EXAMPLE.COM"),
        Some("HTTP://EXAMPLE.COM")
    );
    assert_eq!(
        linkable_uri("mailto:hw@example.com"),
        Some("mailto:hw@example.com")
    );
    for refused in [
        "",
        "~",
        "https://",
        "datasheets/lm317.pdf",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://example.com/a b",
        "https://exämple.com",
    ] {
        assert_eq!(linkable_uri(refused), None, "{refused:?}");
    }
}

#[test]
fn a_text_link_becomes_a_uri_annotation_over_its_label() {
    let mut ctx = sample_ctx(1);
    ctx.sheets[0]
        .schematic
        .text_notes
        .push(note("see [the datasheet](https://example.com/ds.pdf)"));
    let text = pdf_text(&ctx, &PdfOptions::default());
    assert!(text.contains("/Annots ["), "the page lists its annotations");
    assert!(text.contains("/Subtype /Link"), "{text}");
    assert!(text.contains("/S /URI"), "{text}");
    assert!(text.contains("/URI (https://example.com/ds.pdf)"), "{text}");
    assert!(text.contains("/Border [0 0 0]"), "no border is drawn");
}

#[test]
fn an_address_that_may_not_open_prints_its_label_without_a_link() {
    let mut ctx = sample_ctx(1);
    ctx.sheets[0]
        .schematic
        .text_notes
        .push(note("[local copy](file:///tmp/ds.pdf)"));
    let text = pdf_text(&ctx, &PdfOptions::default());
    assert!(!text.contains("/Annots"), "{text}");
    assert!(!text.contains("file:///tmp"), "the address is not written");
}

#[test]
fn links_switched_off_write_no_annotation() {
    let mut ctx = sample_ctx(2);
    ctx.sheets[0]
        .schematic
        .text_notes
        .push(note("[site](https://example.com)"));
    for sheet in &mut ctx.sheets {
        sheet
            .schematic
            .labels
            .push(Label::new("VCC", Point::new(40.0, 40.0), LabelType::Global));
    }
    let on = pdf_text(&ctx, &PdfOptions::default());
    assert!(on.contains("/Annots"));
    let off = pdf_text(
        &ctx,
        &PdfOptions {
            links: false,
            ..PdfOptions::default()
        },
    );
    assert!(!off.contains("/Annots"), "{off}");
    assert!(!off.contains("/Subtype /Link"));
}

#[test]
fn a_page_without_links_writes_the_same_objects_as_before() {
    let ctx = sample_ctx(2);
    let on = PdfExporter.export(&ctx, &PdfOptions::default()).unwrap();
    let off = PdfExporter
        .export(
            &ctx,
            &PdfOptions {
                links: false,
                ..PdfOptions::default()
            },
        )
        .unwrap();
    assert_eq!(on.bytes, off.bytes);
}

#[test]
fn a_global_label_links_to_the_next_page_with_the_same_name() {
    let mut ctx = sample_ctx(3);
    for page in [0, 2] {
        ctx.sheets[page].schematic.labels.push(Label::new(
            "VCC",
            Point::new(40.0, 40.0),
            LabelType::Global,
        ));
    }
    let first = links_of(&ctx, 0);
    assert_eq!(first.len(), 1);
    assert_eq!(goes_to_page(&first[0]), Some(2));
    let last = links_of(&ctx, 2);
    assert_eq!(goes_to_page(&last[0]), Some(0), "wraps round to the first");
    assert!(links_of(&ctx, 1).is_empty());
    let [x1, y1, x2, y2] = first[0].rect;
    assert!(x1 < x2 && y1 < y2, "a box with area: {:?}", first[0].rect);
}

#[test]
fn a_lone_global_label_and_a_net_label_get_no_link() {
    let mut ctx = sample_ctx(2);
    ctx.sheets[0].schematic.labels.push(Label::new(
        "VCC",
        Point::new(40.0, 40.0),
        LabelType::Global,
    ));
    ctx.sheets[1]
        .schematic
        .labels
        .push(Label::new("VCC", Point::new(40.0, 40.0), LabelType::Net));
    assert!(links_of(&ctx, 0).is_empty());
    assert!(links_of(&ctx, 1).is_empty());
}

/// Page 0 places page 1 through a sheet symbol with an `SDA` entry; page 1
/// has the matching `SDA` port.
fn hierarchy() -> ExportContext {
    let child_uuid = Uuid::from_u128(7);
    let mut ctx = sample_ctx(2);
    ctx.sheets[0].schematic.child_sheets.push(ChildSheet {
        uuid: child_uuid,
        name: "Child".into(),
        filename: "child.snxsch".into(),
        position: Point::new(20.0, 20.0),
        size: (40.0, 30.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: Uuid::from_u128(8),
            name: "SDA".into(),
            direction: String::new(),
            position: Point::new(60.0, 30.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    ctx.sheets[1].placement = InstancePath::root().child(child_uuid);
    ctx.sheets[1].schematic.labels.push(Label::new(
        "SDA",
        Point::new(10.0, 10.0),
        LabelType::Hierarchical,
    ));
    ctx
}

#[test]
fn a_sheet_symbol_and_its_entry_link_to_the_child_page() {
    let ctx = hierarchy();
    let links = links_of(&ctx, 0);
    assert_eq!(links.len(), 2, "the symbol, then its entry");
    assert_eq!(goes_to_page(&links[0]), Some(1));
    let LinkTarget::Page { y_pt, .. } = links[0].target else {
        panic!("a page link");
    };
    let page_h_pt = (A4.1 * MM_TO_PT) as f32;
    assert_eq!(y_pt, page_h_pt, "the symbol lands on the page's top");
    assert_eq!(goes_to_page(&links[1]), Some(1));
    assert_ne!(
        links[1].target, links[0].target,
        "the entry lands on the port"
    );
}

#[test]
fn a_port_links_back_to_its_entry_on_the_parent_page() {
    let ctx = hierarchy();
    let links = links_of(&ctx, 1);
    assert_eq!(links.len(), 1);
    assert_eq!(goes_to_page(&links[0]), Some(0));
    let entry = links_of(&ctx, 0)[1].clone();
    let LinkTarget::Page { x_pt, y_pt, .. } = links[0].target else {
        panic!("a page link");
    };
    let [ex1, ey1, ex2, ey2] = entry.rect;
    assert!(
        (ex1..=ex2).contains(&x_pt) && (ey1..=ey2).contains(&y_pt),
        "the port jumps into its entry's box"
    );
}

#[test]
fn a_child_page_left_out_of_the_export_gets_no_link() {
    let ctx = hierarchy();
    let only_parent = LinkPages::new(&ctx, &PdfOptions::default(), &[0], A4).sheet_links(0);
    assert!(only_parent.is_empty());
}

fn part_with_datasheet(datasheet: &str) -> ExportContext {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "Device:R" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-1.0, -2.0),
            end: Point::new(1.0, 2.0),
            width: 0.0,
            fill: FillType::None,
        },
    });
    let mut ctx = sample_ctx(1);
    let schematic = &mut ctx.sheets[0].schematic;
    schematic.lib_symbols.insert("Device:R".into(), lib);
    schematic.symbols.push(Symbol {
        lib_id: "Device:R".into(),
        reference: "R1".into(),
        datasheet: datasheet.into(),
        position: Point::new(100.0, 80.0),
        ..Symbol::default()
    });
    ctx
}

#[test]
fn a_part_with_a_web_datasheet_links_to_it() {
    let ctx = part_with_datasheet("https://example.com/r.pdf");
    let links = links_of(&ctx, 0);
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target,
        LinkTarget::Uri("https://example.com/r.pdf".into())
    );
    let text = pdf_text(&ctx, &PdfOptions::default());
    assert!(text.contains("/URI (https://example.com/r.pdf)"), "{text}");
}

#[test]
fn a_part_with_a_file_datasheet_gets_no_link() {
    for datasheet in ["", "~", "docs/r.pdf"] {
        assert!(links_of(&part_with_datasheet(datasheet), 0).is_empty());
    }
}

#[test]
fn a_page_link_is_a_goto_annotation_to_the_partner_page() {
    let text = pdf_text(&hierarchy(), &PdfOptions::default());
    assert!(text.contains("/S /GoTo"), "{text}");
    // Page refs are 3 and 4: the first page's links go to page 4.
    assert!(text.contains("/D [4 0 R /XYZ"), "{text}");
    assert!(
        text.contains("/D [3 0 R /XYZ"),
        "the port goes back to page 3"
    );
}

#[test]
fn the_picked_watermark_colour_prints() {
    let blue = PdfOptions {
        watermark: Some(Watermark {
            colour_rgb: [0, 0, 255],
            ..Watermark::default()
        }),
        ..PdfOptions::default()
    };
    let ops = page_ops(&sample_ctx(1), &blue, 0);
    assert!(ops.contains("0 0 1 rg\n"), "{ops}");
}
