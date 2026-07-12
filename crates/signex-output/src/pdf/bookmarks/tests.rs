//! Tests for PDF bookmark/outline generation.
use super::*;
use crate::pdf::{
    ColourMode, Margins, Orientation, PageRange, PageSize, PdfScale, SchematicPalette,
};
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Symbol};
use std::collections::HashMap;

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

fn ctx_with_one_sheet() -> ExportContext {
    let mut sheet = empty_sheet();
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::nil(),
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
    sheet.labels.push(Label {
        uuid: uuid::Uuid::nil(),
        text: "VCC".to_string(),
        position: Point::new(20.0, 20.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 0.0,
        justify: signex_types::schematic::HAlign::Center,
        justify_v: signex_types::schematic::VAlign::Bottom,
    });
    ExportContext {
        sheets: vec![SheetSnapshot {
            path: std::path::PathBuf::from("a.standard_sch"),
            schematic: sheet,
            sheet_name: "Power".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
    }
}

fn default_opts() -> PdfOptions {
    PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: Orientation::Landscape,
        colour_mode: ColourMode::Colour,
        page_range: PageRange::All,
        sheet_template: None,
        margins: Margins {
            top_mm: 10.0,
            right_mm: 10.0,
            bottom_mm: 10.0,
            left_mm: 10.0,
        },
        scale: PdfScale::FitToPage,
        include_title_block: true,
        pcb_colour_mode: ColourMode::Colour,
        dpi: 96.0,
        variant: None,
        use_physical_structure: false,
        physical_designators: false,
        physical_net_labels: false,
        physical_ports: false,
        physical_sheet_number: true,
        physical_document_number: false,
        include_no_erc_markers: false,
        include_parameter_sets: false,
        include_probes: false,
        include_blankets: false,
        include_notes: false,
        include_collapsed_notes: false,
        bookmark_zoom: 0.5,
        generate_nets_info: false,
        bookmark_pins: false,
        bookmark_net_labels: false,
        bookmark_ports: false,
        include_component_parameters: false,
        global_bookmarks: false,
        palette: SchematicPalette::classic(),
    }
}

#[test]
fn no_toggles_yields_empty_bookmarks() {
    let ctx = ctx_with_one_sheet();
    let opts = default_opts();
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert!(items.is_empty());
}

#[test]
fn components_toggle_emits_sheet_components_and_each_symbol() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        include_component_parameters: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    // Expect: Sheet, Components group, R1 → 3 items.
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].title, "Sheet 1: Power");
    assert_eq!(items[1].title, "Components");
    assert!(items[2].title.starts_with("R1"));
    assert_eq!(items[1].parent_idx, Some(0));
    assert_eq!(items[2].parent_idx, Some(1));
}

#[test]
fn nets_group_emits_only_when_a_net_subtoggle_is_on() {
    let ctx = ctx_with_one_sheet();
    // generate_nets_info on but every sub-toggle off → still
    // emits the sheet item (so the user gets per-page nav) but
    // no Nets group lands inside.
    let opts = PdfOptions {
        generate_nets_info: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert_eq!(items.len(), 1, "only the sheet bookmark");
    assert!(
        !items.iter().any(|i| i.title == "Nets"),
        "no Nets group when every sub-toggle is off"
    );

    let opts = PdfOptions {
        generate_nets_info: true,
        bookmark_net_labels: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    // Sheet, Nets group, /VCC → 3 items.
    assert_eq!(items.len(), 3);
    assert_eq!(items[1].title, "Nets");
    assert_eq!(items[2].title, "/VCC");
}

#[test]
fn global_bookmarks_pulls_groups_to_top_level() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        include_component_parameters: true,
        generate_nets_info: true,
        bookmark_net_labels: true,
        global_bookmarks: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    // Global Components, Global Nets, Sheet 1, R1 (under Components),
    // /VCC (under Nets) → 5 items.
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].title, "Components");
    assert_eq!(items[1].title, "Nets");
    assert_eq!(items[2].title, "Sheet 1: Power");
    assert!(items[2].parent_idx.is_none());
    // R1 → parent is global Components (idx 0)
    assert_eq!(items[3].parent_idx, Some(0));
    // /VCC → parent is global Nets (idx 1)
    assert_eq!(items[4].parent_idx, Some(1));
}

#[test]
fn variant_tag_appended_when_physical_structure_on() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        include_component_parameters: true,
        use_physical_structure: true,
        variant: Some("VarA".to_string()),
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert!(items[0].title.contains("[VarA]"), "got {}", items[0].title);
}

#[test]
fn bookmark_zoom_default_resolves_to_unit_zoom() {
    let z = bookmark_zoom_factor(0.5);
    assert!((z - 1.0).abs() < 1e-6);
}

#[test]
fn bookmark_zoom_clamps_out_of_range() {
    // Slider clamps to [0, 1] — nothing in PDF land below 25 %
    // is useful, nothing above 4× either.
    assert!((bookmark_zoom_factor(-1.0) - 0.25).abs() < 1e-6);
    assert!((bookmark_zoom_factor(2.0) - 4.0).abs() < 1e-6);
}

#[test]
fn pin_bookmarks_skipped_when_subtoggle_off() {
    let mut sheet = empty_sheet();
    let mut sym = Symbol {
        uuid: uuid::Uuid::nil(),
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
    };
    sym.pin_uuids.insert("1".to_string(), uuid::Uuid::nil());
    sym.pin_uuids.insert("2".to_string(), uuid::Uuid::nil());
    sheet.symbols.push(sym);

    let ctx = ExportContext {
        sheets: vec![SheetSnapshot {
            path: std::path::PathBuf::from("a.standard_sch"),
            schematic: sheet,
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
    };

    let opts = PdfOptions {
        generate_nets_info: true,
        bookmark_pins: false,
        bookmark_net_labels: false,
        bookmark_ports: false,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    // Sheet only — every sub-toggle is off.
    assert_eq!(items.len(), 1);
    assert!(!items.iter().any(|i| i.title.starts_with("pin ")));

    let opts = PdfOptions {
        generate_nets_info: true,
        bookmark_pins: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    // Sheet + Nets group + 2 pins → 4 items.
    assert_eq!(items.len(), 4);
    assert!(items.iter().any(|i| i.title == "pin R1.1"));
    assert!(items.iter().any(|i| i.title == "pin R1.2"));
}

#[test]
fn multi_sheet_bookmarks_have_independent_groups() {
    let ctx = {
        let mut sheets = vec![];
        for i in 0..3 {
            let mut sheet = empty_sheet();
            sheet.symbols.push(Symbol {
                uuid: uuid::Uuid::nil(),
                lib_id: "Device:R".to_string(),
                reference: format!("R{}", i + 1),
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
            sheets.push(SheetSnapshot {
                path: std::path::PathBuf::from(format!("sheet_{i}.standard_sch")),
                schematic: sheet,
                sheet_name: format!("Sheet{}", i + 1),
                sheet_number: i + 1,
                sheet_count: 3,
            });
        }
        ExportContext {
            sheets,
            metadata: ProjectMetadata::default(),
            netlist: None,
        }
    };
    let opts = PdfOptions {
        include_component_parameters: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0, 1, 2], 297.0, 210.0, 595.0);
    // 3 sheets × (Sheet + Components + 1 Symbol) = 9 items.
    assert_eq!(items.len(), 9);

    // Top-level items = the 3 Sheet bookmarks.
    let top: Vec<&PendingBookmark> = items.iter().filter(|b| b.parent_idx.is_none()).collect();
    assert_eq!(top.len(), 3);

    // Each Sheet bookmark has exactly one child (its Components group).
    for sheet_bookmark in top {
        assert_eq!(sheet_bookmark.children.len(), 1);
    }

    // Page indices on the symbol bookmarks match their sheet's
    // page index — bookmarks navigate to the right page.
    for item in &items {
        if item.title.starts_with('R') {
            let n: usize = item.title.trim_start_matches('R').parse().unwrap();
            assert_eq!(item.page_idx, n - 1, "{} jumped to wrong page", item.title);
        }
    }
}
