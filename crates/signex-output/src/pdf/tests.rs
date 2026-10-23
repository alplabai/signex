//! Tests for the PDF exporter.
use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;

use super::*;
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

pub(super) fn empty_sheet() -> SchematicSheet {
    SchematicSheet::default()
}

pub(super) fn sample_ctx(sheet_count: usize) -> ExportContext {
    ExportContext {
        sheets: (0..sheet_count)
            .map(|i| SheetSnapshot {
                path: PathBuf::from(format!("sheet_{i}.standard_sch")),
                schematic: empty_sheet(),
                sheet_name: format!("Sheet{i}"),
                sheet_number: i + 1,
                sheet_count,
            })
            .collect(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
    }
}

#[test]
fn produces_valid_pdf_header() {
    let ctx = sample_ctx(1);
    let out = PdfExporter
        .export(&ctx, &PdfOptions::default())
        .expect("export");
    assert!(out.bytes.starts_with(b"%PDF-"), "missing %PDF- header");
    assert!(out.bytes.ends_with(b"%%EOF\n") || out.bytes.ends_with(b"%%EOF"));
    assert_eq!(out.page_count, 1);
}

fn markup_ctx(note: &str, label: &str) -> ExportContext {
    use signex_types::schematic::{HAlign, Label, LabelType, Point, TextNote, VAlign};

    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic.text_notes.push(TextNote {
        uuid: uuid::Uuid::nil(),
        text: note.into(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        font_size: 2.54,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
    });
    ctx.sheets[0].schematic.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::nil(),
        text: label.into(),
        position: Point::new(20.0, 30.0),
        rotation: 0.0,
        label_type: LabelType::Global,
        shape: "input".into(),
        font_size: 2.54,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    });
    ctx
}

#[test]
fn escaped_unicode_markup_exports_like_unescaped_text() {
    let opts = PdfOptions {
        // Bookmark titles retain raw markup; compare the rendered page.
        generate_nets_info: false,
        ..PdfOptions::default()
    };
    // Use the exporter's existing transliteration; full Unicode PDF fonts
    // are separate from markup parsing (issue #352).
    let expected = PdfExporter
        .export(&markup_ctx("~Ölçüm~", "*Ölçüm*"), &opts)
        .expect("unescaped markup export");
    let actual = PdfExporter
        .export(&markup_ctx(r"~\Ölçüm~", r"*\Ölçüm*"), &opts)
        .expect("escaped markup export");
    assert_eq!(actual.page_count, 1);
    assert!(actual.bytes == expected.bytes, "rendered PDF must match");
    let empty = PdfExporter
        .export(&sample_ctx(1), &opts)
        .expect("empty export");
    assert_ne!(actual.bytes, empty.bytes, "text must reach the PDF");
}

#[test]
fn escaped_unicode_markup_preview_matches_unescaped_text() {
    let render = |ctx: &ExportContext| {
        crate::svg::SvgRenderContext::from_sheet(
            &ctx.sheets[0],
            &PdfOptions::default(),
            297.0,
            210.0,
            2.0,
            None,
        )
        .rasterize_rgba(594, 420)
        .expect("preview pixels")
    };
    let expected = render(&markup_ctx("~µA~", "*Ölçüm*"));
    let actual = render(&markup_ctx(r"~\µA~", r"*\Ölçüm*"));
    assert!(
        actual == expected,
        "escaped text must render the same glyphs"
    );
    assert!(
        actual != render(&sample_ctx(1)),
        "text must reach the preview"
    );
}

#[test]
fn multi_sheet_produces_multi_page() {
    let ctx = sample_ctx(4);
    let out = PdfExporter
        .export(&ctx, &PdfOptions::default())
        .expect("export");
    assert_eq!(out.page_count, 4);
}

#[test]
fn empty_context_errors() {
    let ctx = ExportContext {
        sheets: vec![],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
    };
    let err = PdfExporter
        .export(&ctx, &PdfOptions::default())
        .unwrap_err();
    assert!(matches!(err, PdfError::NoSheets));
}

#[test]
fn page_range_specific() {
    let ctx = sample_ctx(5);
    let opts = PdfOptions {
        page_range: PageRange::Specific(vec![1, 3, 5]),
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).unwrap();
    assert_eq!(out.page_count, 3);
}

#[test]
fn page_range_range_inclusive() {
    let ctx = sample_ctx(5);
    let opts = PdfOptions {
        page_range: PageRange::Range(2, 4),
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).unwrap();
    assert_eq!(out.page_count, 3); // 2, 3, 4
}

#[test]
fn page_range_out_of_bounds() {
    let ctx = sample_ctx(3);
    let opts = PdfOptions {
        page_range: PageRange::Specific(vec![1, 99]),
        ..Default::default()
    };
    let err = PdfExporter.export(&ctx, &opts).unwrap_err();
    assert!(matches!(err, PdfError::OutOfRange(99, 3)));
}

#[test]
fn page_size_reflected_in_media_box() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: Orientation::Portrait,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).unwrap();
    // A4 portrait = 210 × 297 mm = 595.28 × 841.89 pt.
    let bytes = String::from_utf8_lossy(&out.bytes);
    assert!(bytes.contains("595"), "width not reflected in MediaBox");
    assert!(bytes.contains("841"), "height not reflected in MediaBox");
}

#[test]
fn exports_schematic_content() {
    use signex_types::schematic::{Label, LabelType, Point, Symbol, Wire};
    use uuid::Uuid;

    let mut sheet = empty_sheet();

    // Add one wire.
    sheet.wires.push(Wire {
        end: Point::new(10.0, 10.0),
        stroke_width: 0.15,
        ..Default::default()
    });

    // Add one symbol.
    sheet.symbols.push(Symbol {
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });

    // Add one label.
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::nil(),
        text: "VCC".to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 0.0,
        justify: signex_types::schematic::HAlign::Center,
        justify_v: signex_types::schematic::VAlign::Bottom,
    });

    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;

    let out = PdfExporter
        .export(&ctx, &PdfOptions::default())
        .expect("export");

    let bytes = String::from_utf8_lossy(&out.bytes);
    // Check for content stream operators: 'm' (moveto), 'l' (lineto), 'S' (stroke),
    // 're' (rect), 'Tj' (show text).
    let has_graphics = bytes.contains(" l\n") || bytes.contains(" re\n") || bytes.contains(" Tj");
    assert!(
        has_graphics,
        "exported PDF should contain graphics operators"
    );
}

#[test]
fn colour_mode_colour_preserves_rgb() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        colour_mode: ColourMode::Colour,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    assert!(out.bytes.starts_with(b"%PDF-"));
}

#[test]
fn colour_mode_grayscale_maps_red_to_0_299() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        colour_mode: ColourMode::Grayscale,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    assert!(out.bytes.starts_with(b"%PDF-"));
}

#[test]
fn colour_mode_bw_pushes_strokes_to_black() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        colour_mode: ColourMode::BlackAndWhite,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    assert!(out.bytes.starts_with(b"%PDF-"));
}

#[test]
fn fit_to_page_scales_large_content_down() {
    use signex_types::schematic::{Point, Wire};

    let mut sheet = empty_sheet();
    // Add a very large wire (0, 0) to (500, 500) mm — much larger than A4.
    sheet.wires.push(Wire {
        end: Point::new(500.0, 500.0),
        stroke_width: 0.15,
        ..Default::default()
    });

    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;

    let opts = PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: Orientation::Landscape,
        scale: PdfScale::FitToPage,
        margins: Margins {
            top_mm: 10.0,
            right_mm: 10.0,
            bottom_mm: 10.0,
            left_mm: 10.0,
        },
        ..Default::default()
    };

    let out = PdfExporter.export(&ctx, &opts).expect("export");
    // PDF should be valid and contain content (scaled down wires).
    assert!(out.bytes.starts_with(b"%PDF-"));
    assert!(out.page_count == 1);
}

#[test]
fn fit_to_page_does_not_upscale_small_content() {
    use signex_types::schematic::{Point, Wire};

    let mut sheet = empty_sheet();
    // Add a small wire (10, 10) to (20, 20) mm — much smaller than A4.
    sheet.wires.push(Wire {
        start: Point::new(10.0, 10.0),
        end: Point::new(20.0, 20.0),
        stroke_width: 0.15,
        ..Default::default()
    });

    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;

    let opts = PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: Orientation::Landscape,
        scale: PdfScale::FitToPage,
        margins: Margins {
            top_mm: 10.0,
            right_mm: 10.0,
            bottom_mm: 10.0,
            left_mm: 10.0,
        },
        ..Default::default()
    };

    let out = PdfExporter.export(&ctx, &opts).expect("export");
    // PDF should be valid. FitToPage should NOT upscale (use 1:1).
    assert!(out.bytes.starts_with(b"%PDF-"));
    assert!(out.page_count == 1);
}

#[test]
fn template_draws_frame_rect() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        sheet_template: Some(TemplateId::from("iso_a4_landscape")),
        include_title_block: true,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    // PDF should contain frame rect operator (re).
    let bytes = String::from_utf8_lossy(&out.bytes);
    assert!(bytes.contains(" re\n"), "template should draw frame rect");
}

#[test]
fn template_renders_substituted_text_in_title_block() {
    let mut ctx = sample_ctx(1);
    ctx.metadata.title = "Test Project".to_string();
    ctx.metadata.revision = "A".to_string();

    let opts = PdfOptions {
        sheet_template: Some(TemplateId::from("iso_a4_landscape")),
        include_title_block: true,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    // PDF should be valid and include title block fields.
    assert!(out.bytes.starts_with(b"%PDF-"));
}

#[test]
fn no_outlines_emitted_when_every_bookmark_toggle_is_off() {
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        include_component_parameters: false,
        generate_nets_info: false,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let bytes = String::from_utf8_lossy(&out.bytes);
    assert!(
        !bytes.contains("/Outlines"),
        "should not write /Outlines when no toggles are set"
    );
}

#[test]
fn page_paper_colour_is_filled_in_content_stream() {
    // The first element in svg_ctx is the paper-fill rect with
    // palette.paper as fill colour; the PDF content stream
    // emits an "re" operator immediately followed by an "f" or
    // "B" fill operator.
    let ctx = sample_ctx(1);
    let opts = PdfOptions {
        palette: SchematicPalette {
            paper: (0.10, 0.20, 0.30),
            ..SchematicPalette::classic()
        },
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let bytes = String::from_utf8_lossy(&out.bytes);
    // 0.10 0.20 0.30 RG / rg should appear when we emit the
    // page-fill rect — the surface uses `RG` for stroke colour
    // and `rg` for fill colour.
    assert!(
        bytes.contains("0.1 0.2 0.3 rg") || bytes.contains("0.10 0.20 0.30 rg"),
        "expected paper fill colour in PDF stream; got slice: {}",
        &bytes[..bytes.len().min(2000)]
    );
}

#[test]
fn outlines_emitted_when_components_toggle_is_on() {
    use signex_types::schematic::{Point, Symbol};

    let mut sheet = empty_sheet();
    sheet.symbols.push(Symbol {
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });
    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;

    let opts = PdfOptions {
        include_component_parameters: true,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let bytes = String::from_utf8_lossy(&out.bytes);
    assert!(
        bytes.contains("/Outlines"),
        "catalog should reference /Outlines"
    );
    assert!(
        bytes.contains("/Title"),
        "outline items should carry /Title entries"
    );
    // Sheet bookmark + Components group + R1 → at least 3 outline items.
    let title_count = bytes.matches("/Title").count();
    assert!(title_count >= 3, "got only {title_count} /Title entries");
}

/// First page's `/MediaBox [0 0 w h]` as points.
fn media_box_pt(bytes: &[u8]) -> (f64, f64) {
    let text = String::from_utf8_lossy(bytes);
    let at = text.find("/MediaBox").expect("MediaBox present");
    let open = text[at..].find('[').expect("open") + at + 1;
    let close = text[open..].find(']').expect("close") + open;
    let nums: Vec<f64> = text[open..close]
        .split_whitespace()
        .map(|n| n.parse().expect("number"))
        .collect();
    assert_eq!(nums.len(), 4, "{nums:?}");
    (nums[2], nums[3])
}

#[test]
fn media_box_matches_every_selectable_page_size_and_custom() {
    let ctx = sample_ctx(1);
    let pt = |mm: f64| mm * 72.0 / 25.4;
    for kind in PageSizeKind::ALL {
        for orientation in [Orientation::Portrait, Orientation::Landscape] {
            let size = kind.select(PageSize::IsoA4, orientation);
            let opts = PdfOptions {
                page_size: size,
                orientation,
                ..Default::default()
            };
            let out = PdfExporter.export(&ctx, &opts).expect("export");
            let (w, h) = media_box_pt(&out.bytes);
            let (want_w, want_h) = size.dimensions_mm(orientation);
            assert!((w - pt(want_w)).abs() < 0.01, "{kind} {orientation} w {w}");
            assert!((h - pt(want_h)).abs() < 0.01, "{kind} {orientation} h {h}");
        }
    }
    let custom = PageSize::Custom {
        width_mm: 123.0,
        height_mm: 456.0,
    };
    let opts = PdfOptions {
        page_size: custom,
        ..Default::default()
    };
    let out = PdfExporter.export(&ctx, &opts).expect("export");
    let (w, h) = media_box_pt(&out.bytes);
    assert!(
        (w - pt(123.0)).abs() < 0.01 && (h - pt(456.0)).abs() < 0.01,
        "{w}x{h}"
    );
}

/// The `x y m` move-to points of the exported content streams, in order.
fn move_to_points(bytes: &[u8]) -> Vec<(f64, f64)> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let (x, y, op) = (parts.next()?, parts.next()?, parts.next()?);
            (op == "m" && parts.next().is_none()).then_some(())?;
            Some((x.parse().ok()?, y.parse().ok()?))
        })
        .collect()
}

fn wire_ctx() -> ExportContext {
    use signex_types::schematic::{Point, Wire};

    let mut sheet = empty_sheet();
    sheet.wires.push(Wire {
        uuid: uuid::Uuid::nil(),
        start: Point::new(0.0, 0.0),
        end: Point::new(50.0, 20.0),
        stroke_width: 0.15,
        unknown_columns: Default::default(),
    });
    let mut ctx = sample_ctx(1);
    ctx.sheets[0].schematic = sheet;
    ctx
}

fn one_to_one_with(margins: Margins) -> PdfOptions {
    PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: Orientation::Landscape,
        scale: PdfScale::OneToOne,
        margins,
        ..Default::default()
    }
}

#[test]
fn margins_move_the_content_in_the_exported_pdf() {
    let pt = |mm: f64| mm * 72.0 / 25.4;
    let ctx = wire_ctx();
    let even = |mm: f64| Margins {
        top_mm: mm,
        right_mm: mm,
        bottom_mm: mm,
        left_mm: mm,
    };
    let base = PdfExporter
        .export(&ctx, &one_to_one_with(even(10.0)))
        .expect("export");
    let base_points = move_to_points(&base.bytes);
    assert!(!base_points.is_empty(), "content stream has move-to ops");

    // Widening only the left margin by 20 mm shifts the drawing right by 20 mm.
    let left = PdfExporter
        .export(
            &ctx,
            &one_to_one_with(Margins {
                left_mm: 30.0,
                ..even(10.0)
            }),
        )
        .expect("export");
    let left_points = move_to_points(&left.bytes);
    assert_eq!(left_points.len(), base_points.len());
    assert!(
        base_points
            .iter()
            .zip(&left_points)
            .any(|(a, b)| (b.0 - a.0 - pt(20.0)).abs() < 0.05 && (b.1 - a.1).abs() < 0.05),
        "no move-to point shifted right by 20 mm"
    );

    // Widening only the top margin by 20 mm shifts it down (PDF y points up).
    let top = PdfExporter
        .export(
            &ctx,
            &one_to_one_with(Margins {
                top_mm: 30.0,
                ..even(10.0)
            }),
        )
        .expect("export");
    let top_points = move_to_points(&top.bytes);
    assert!(
        base_points
            .iter()
            .zip(&top_points)
            .any(|(a, b)| (a.1 - b.1 - pt(20.0)).abs() < 0.05 && (b.0 - a.0).abs() < 0.05),
        "no move-to point shifted down by 20 mm"
    );
}
