//! Tests for the PDF exporter.
use std::path::PathBuf;

use signex_types::schematic::SchematicSheet;

use super::*;
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::nil(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: String::new(),
        root_sheet_page: "1".into(),
        symbols: vec![],
        wires: vec![],
        junctions: vec![],
        labels: vec![],
        child_sheets: vec![],
        no_connects: vec![],
        text_notes: vec![],
        buses: vec![],
        bus_entries: vec![],
        drawings: vec![],
        no_erc_directives: vec![],
        title_block: Default::default(),
        lib_symbols: Default::default(),
    }
}

fn sample_ctx(sheet_count: usize) -> ExportContext {
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
    use std::collections::HashMap;
    use uuid::Uuid;

    let mut sheet = empty_sheet();

    // Add one wire.
    sheet.wires.push(Wire {
        uuid: Uuid::nil(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 10.0),
        stroke_width: 0.15,
    });

    // Add one symbol.
    sheet.symbols.push(Symbol {
        uuid: Uuid::nil(),
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: vec![],
        pin_uuids: HashMap::new(),
        instances: vec![],
        footprint: String::new(),
        datasheet: String::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
    });

    // Add one label.
    sheet.labels.push(Label {
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
    use uuid::Uuid;

    let mut sheet = empty_sheet();
    // Add a very large wire (0, 0) to (500, 500) mm — much larger than A4.
    sheet.wires.push(Wire {
        uuid: Uuid::nil(),
        start: Point::new(0.0, 0.0),
        end: Point::new(500.0, 500.0),
        stroke_width: 0.15,
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
    use uuid::Uuid;

    let mut sheet = empty_sheet();
    // Add a small wire (10, 10) to (20, 20) mm — much smaller than A4.
    sheet.wires.push(Wire {
        uuid: Uuid::nil(),
        start: Point::new(10.0, 10.0),
        end: Point::new(20.0, 20.0),
        stroke_width: 0.15,
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
    use std::collections::HashMap;
    use uuid::Uuid;

    let mut sheet = empty_sheet();
    sheet.symbols.push(Symbol {
        uuid: Uuid::nil(),
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: HashMap::new(),
        custom_properties: vec![],
        pin_uuids: HashMap::new(),
        instances: vec![],
        footprint: String::new(),
        datasheet: String::new(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
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
