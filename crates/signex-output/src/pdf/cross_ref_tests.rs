//! Cross references on the exported page and their links (#1273).

use signex_types::schematic::{InstancePath, Label, LabelType, Point};

use super::tests::sample_ctx;
use super::*;
use crate::cross_ref::{CrossRefTarget, PortCrossRefs};

fn global(text: &str, at: Point, uuid: u128) -> Label {
    Label {
        uuid: uuid::Uuid::from_u128(uuid),
        ..Label::new(text, at, LabelType::Global)
    }
}

fn target(page: usize, zone: &str, x_mm: f64, y_mm: f64) -> CrossRefTarget {
    CrossRefTarget {
        page,
        zone: Some(zone.to_string()),
        x_mm,
        y_mm,
    }
}

/// Two pages, each with a VCC Global label pointing at the other.
fn linked() -> (ExportContext, PdfOptions) {
    let mut ctx = sample_ctx(2);
    ctx.sheets[0]
        .schematic
        .labels
        .push(global("VCC", Point::new(50.0, 50.0), 1));
    ctx.sheets[1]
        .schematic
        .labels
        .push(global("VCC", Point::new(100.0, 80.0), 2));
    let mut refs = PortCrossRefs::default();
    let root = InstancePath::root();
    refs.push(
        &ctx.sheets[0].path,
        &root,
        uuid::Uuid::from_u128(1),
        vec![target(2, "C3", 100.0, 80.0)],
    );
    refs.push(
        &ctx.sheets[1].path,
        &root,
        uuid::Uuid::from_u128(2),
        vec![target(1, "B2", 50.0, 50.0)],
    );
    let opts = PdfOptions {
        port_cross_refs: refs,
        // No outline, so every `/XYZ` destination is a link's.
        generate_nets_info: false,
        bookmark_sheets: false,
        bookmark_parts: false,
        ..PdfOptions::default()
    };
    (ctx, opts)
}

fn texts(elements: &[SvgElement]) -> Vec<String> {
    elements
        .iter()
        .filter_map(|e| match e {
            SvgElement::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_page_draws_the_mark_beside_the_label() {
    let (ctx, opts) = linked();
    let elements = cross_ref::cross_ref_elements(&ctx.sheets[0], &opts, 297.0, 210.0, MM_TO_PT);
    assert_eq!(texts(&elements), ["[2/C3]"]);
    let elements = cross_ref::cross_ref_elements(&ctx.sheets[1], &opts, 297.0, 210.0, MM_TO_PT);
    assert_eq!(texts(&elements), ["[1/B2]"]);
}

#[test]
fn without_cross_references_a_page_draws_and_links_nothing() {
    let (ctx, _) = linked();
    let opts = PdfOptions::default();
    let elements = cross_ref::cross_ref_elements(&ctx.sheets[0], &opts, 297.0, 210.0, MM_TO_PT);
    assert!(elements.is_empty());
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let text = String::from_utf8_lossy(&out.bytes);
    assert!(!text.contains("/Annots"), "no links");
}

/// Each entry is a link annotation that goes to the partner's page: page 1's
/// mark to page 2 (object 4) and back (object 3).
#[test]
fn each_entry_links_to_its_page() {
    let (ctx, opts) = linked();
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let text = String::from_utf8_lossy(&out.bytes);
    assert_eq!(text.matches("/Subtype /Link").count(), 2, "{text}");
    assert_eq!(text.matches("/Annots").count(), 2);
    assert!(text.contains("/S /GoTo"));
    assert!(text.contains("[4 0 R /XYZ"), "page 1 links to page 2");
    assert!(text.contains("[3 0 R /XYZ"), "page 2 links to page 1");
}

/// A page left out of the export has no link to it; the text still names it.
#[test]
fn a_page_not_exported_is_named_but_not_linked() {
    let (ctx, mut opts) = linked();
    opts.page_range = PageRange::Specific(vec![1]);
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let text = String::from_utf8_lossy(&out.bytes);
    assert!(!text.contains("/Subtype /Link"), "{text}");
    let links = cross_ref::page_links(&ctx, &opts, &[0], 297.0, 210.0);
    assert_eq!(links, [Vec::new()]);
}

/// The link box covers the entry's text on the page.
#[test]
fn the_link_box_lies_on_the_page_inside_the_mark() {
    let (ctx, opts) = linked();
    let links = cross_ref::page_links(&ctx, &opts, &[0, 1], 297.0, 210.0);
    let link = &links[0][0];
    assert_eq!(link.to_page, 1);
    let (left, bottom, right, top) = link.rect;
    let page_w_pt = (297.0 * MM_TO_PT) as f32;
    let page_h_pt = (210.0 * MM_TO_PT) as f32;
    assert!(0.0 < left && left < right && right < page_w_pt, "{link:?}");
    assert!(0.0 < bottom && bottom < top && top < page_h_pt, "{link:?}");
}
