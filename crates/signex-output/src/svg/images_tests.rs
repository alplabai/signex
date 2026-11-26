//! #1116 (mockup row 204): placed images in the SVG page model, the SVG
//! document, the print preview and the PDF.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::image_asset::content_hash;
use signex_types::schematic::{Point, SchDrawing, SchImage, SchematicSheet};
use uuid::Uuid;

use super::images::base64;
use super::*;
use crate::pdf::PdfOptions;
use crate::{ExportContext, Exporter, PdfExporter, ProjectMetadata, SheetSnapshot};

/// A 4 × 2 px red PNG, half transparent in its right column.
fn png() -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(4, 2).unwrap();
    pixmap.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
    pixmap.pixels_mut()[3] = tiny_skia::PremultipliedColorU8::from_rgba(64, 0, 0, 64).unwrap();
    pixmap.encode_png().unwrap()
}

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20"><rect x="0" y="0" width="40" height="20" fill="#0000ff"/></svg>"##;

fn image(file: &str, bytes: &[u8], border: bool) -> SchImage {
    let mut image = SchImage::new(
        Uuid::from_u128(5),
        Point::new(20.0, 30.0),
        (40.0, 20.0),
        file.into(),
        content_hash(bytes),
    );
    image.border = border;
    image
}

fn snapshot(image: SchImage) -> SheetSnapshot {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.drawings.push(SchDrawing::Image(image));
    SheetSnapshot {
        path: PathBuf::from("test.snxsch"),
        schematic: sheet,
        sheet_name: "Sheet1".to_string(),
        sheet_number: 1,
        sheet_count: 1,
        placement: Default::default(),
    }
}

fn options_with(file: &str, bytes: &[u8]) -> PdfOptions {
    let mut opts = PdfOptions::default();
    opts.images
        .insert(file, content_hash(bytes), Arc::from(bytes));
    opts
}

fn page(snapshot: &SheetSnapshot, opts: &PdfOptions) -> SvgRenderContext {
    let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
    SvgRenderContext::from_sheet(snapshot, opts, w_mm, h_mm, 72.0 / 25.4, None)
}

#[test]
fn base64_pads_as_a_data_uri_needs() {
    assert_eq!(base64(b"Man"), "TWFu");
    assert_eq!(base64(b"Ma"), "TWE=");
    assert_eq!(base64(b"M"), "TQ==");
    assert_eq!(base64(b""), "");
}

#[test]
fn a_raster_image_with_its_bytes_is_one_image_element_in_its_box() {
    let bytes = png();
    let snapshot = snapshot(image("assets/images/plot.png", &bytes, false));
    let opts = options_with("assets/images/plot.png", &bytes);
    let ctx = page(&snapshot, &opts);
    let images: Vec<(f32, f32)> = ctx
        .elements
        .iter()
        .filter_map(|e| match e {
            SvgElement::Image { width, height, .. } => Some((*width, *height)),
            _ => None,
        })
        .collect();
    assert_eq!(images.len(), 1);
    // The 40 × 20 mm box, at the page's scale.
    let (w, h) = images[0];
    assert!(w > 0.0 && (w / h - 2.0).abs() < 1e-3, "{w} × {h}");
    let svg = ctx.svg_document();
    assert!(svg.contains("<image "), "{svg}");
    assert!(svg.contains("href=\"data:image/png;base64,"));
}

#[test]
fn a_missing_file_prints_an_empty_box_with_only_its_border() {
    let bytes = png();
    let plain = page(
        &snapshot(image("assets/images/gone.png", &bytes, false)),
        &PdfOptions::default(),
    );
    let bordered = page(
        &snapshot(image("assets/images/gone.png", &bytes, true)),
        &PdfOptions::default(),
    );
    let count = |ctx: &SvgRenderContext| {
        ctx.elements
            .iter()
            .filter(|e| matches!(e, SvgElement::Image { .. }))
            .count()
    };
    assert_eq!(count(&plain), 0);
    assert_eq!(count(&bordered), 0);
    assert_eq!(bordered.elements.len(), plain.elements.len() + 1);
}

#[test]
fn an_svg_picture_is_drawn_as_vector_paths() {
    let snapshot = snapshot(image("assets/images/logo.svg", SVG, false));
    let opts = options_with("assets/images/logo.svg", SVG);
    let ctx = page(&snapshot, &opts);
    assert!(
        !ctx.elements
            .iter()
            .any(|e| matches!(e, SvgElement::Image { .. }))
    );
    let blue = ctx.elements.iter().any(
        |e| matches!(e, SvgElement::Path { style, .. } if style.fill_rgb == Some((0.0, 0.0, 1.0))),
    );
    assert!(blue, "the rect becomes a blue path");
}

#[test]
fn the_preview_paints_the_picture() {
    let bytes = png();
    let snapshot = snapshot(image("assets/images/plot.png", &bytes, false));
    let opts = options_with("assets/images/plot.png", &bytes);
    let ctx = page(&snapshot, &opts);
    let (w, h) = (ctx.width.ceil() as u32, ctx.height.ceil() as u32);
    let rgba = ctx.rasterize_rgba(w, h).expect("a raster");
    // A point a quarter into the box: the opaque red part of the picture.
    let (x, y, bw, bh) = ctx
        .elements
        .iter()
        .find_map(|e| match e {
            SvgElement::Image {
                x,
                y,
                width,
                height,
                ..
            } => Some((*x, *y, *width, *height)),
            _ => None,
        })
        .expect("the image");
    let (px, py) = ((x + bw * 0.25) as usize, (y + bh * 0.5) as usize);
    let at = (py * w as usize + px) * 4;
    assert!(
        rgba[at] > 200 && rgba[at + 1] < 60,
        "{:?}",
        &rgba[at..at + 4]
    );
}

#[test]
fn the_pdf_embeds_the_picture_once_as_an_image_object() {
    let bytes = png();
    let snapshot = snapshot(image("assets/images/plot.png", &bytes, false));
    let ctx = ExportContext {
        sheets: vec![snapshot],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let opts = options_with("assets/images/plot.png", &bytes);
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let text = String::from_utf8_lossy(&out.bytes);
    assert!(text.contains("/Subtype /Image"), "an image object");
    assert!(text.contains("/SMask"), "its transparency as a soft mask");
    assert!(text.contains("/Im0 Do"), "painted on the page");
    assert_eq!(
        text.matches("/Subtype /Image").count(),
        2,
        "the picture and its mask"
    );
}
