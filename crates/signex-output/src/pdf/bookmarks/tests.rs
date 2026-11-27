//! Tests for PDF bookmark/outline generation.
use super::*;
use crate::pdf::{ColourMode, Margins, PageRange, PageSize, PdfScale, SchematicPalette};
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, SheetOrientation, Symbol};

fn empty_sheet() -> SchematicSheet {
    SchematicSheet::default()
}

fn ctx_with_one_sheet() -> ExportContext {
    let mut sheet = empty_sheet();
    sheet.symbols.push(Symbol {
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
    });
    sheet.labels.push(Label {
        uuid: uuid::Uuid::nil(),
        font_size: 0.0,
        justify: signex_types::schematic::HAlign::Center,
        justify_v: signex_types::schematic::VAlign::Bottom,
        ..Label::new("VCC", Point::new(20.0, 20.0), LabelType::Net)
    });
    ExportContext {
        sheets: vec![SheetSnapshot {
            path: std::path::PathBuf::from("a.standard_sch"),
            schematic: sheet,
            sheet_name: "Power".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    }
}

fn default_opts() -> PdfOptions {
    PdfOptions {
        page_size: PageSize::IsoA4,
        orientation: SheetOrientation::Landscape,
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
        include_no_connect_markers: false,
        include_parameter_sets: false,
        include_probes: false,
        include_blankets: false,
        include_compile_masks: false,
        include_diff_pairs: false,
        include_notes: false,
        include_collapsed_notes: false,
        bookmark_zoom: 0.5,
        generate_nets_info: false,
        bookmark_pins: false,
        bookmark_net_labels: false,
        bookmark_ports: false,
        global_bookmarks: false,
        bookmark_sheets: true,
        bookmark_parts: false,
        part_bookmark_order: PartBookmarkOrder::BySheet,
        watermark: None,
        layers: None,
        links: true,
        palette: SchematicPalette::classic(),
        net_class_styles: Default::default(),
        schematic_defaults: Default::default(),
        port_cross_refs: Default::default(),
        draw_not_fitted: true,
    }
}

#[test]
fn no_toggles_yields_empty_bookmarks() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        bookmark_sheets: false,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert!(items.is_empty());
}

#[test]
fn components_toggle_emits_sheet_components_and_each_symbol() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        bookmark_parts: true,
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
        bookmark_parts: true,
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
        lib_id: "Device:R".to_string(),
        reference: "R1".to_string(),
        value: "10k".to_string(),
        position: Point::new(50.0, 50.0),
        ..Default::default()
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
            placement: Default::default(),
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
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
                lib_id: "Device:R".to_string(),
                reference: format!("R{}", i + 1),
                value: "10k".to_string(),
                position: Point::new(50.0, 50.0),
                ..Default::default()
            });
            sheets.push(SheetSnapshot {
                path: std::path::PathBuf::from(format!("sheet_{i}.standard_sch")),
                schematic: sheet,
                sheet_name: format!("Sheet{}", i + 1),
                sheet_number: i + 1,
                sheet_count: 3,
                placement: Default::default(),
            });
        }
        ExportContext {
            sheets,
            metadata: ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
            current_sheet: None,
            netlist_placements: Vec::new(),
        }
    };
    let opts = PdfOptions {
        bookmark_parts: true,
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

#[test]
fn pin_bookmarks_are_sorted_and_skip_power_symbols() {
    let mut sheet = empty_sheet();
    let mut sym = Symbol {
        reference: "U1".to_string(),
        ..Default::default()
    };
    for n in ["9", "3", "10", "1", "2", "7"] {
        sym.pin_uuids.insert(n.to_string(), uuid::Uuid::nil());
    }
    sheet.symbols.push(sym);
    let mut pwr = Symbol {
        reference: "#PWR01".to_string(),
        is_power: true,
        ..Default::default()
    };
    pwr.pin_uuids.insert("1".to_string(), uuid::Uuid::nil());
    sheet.symbols.push(pwr);
    let ctx = ExportContext {
        sheets: vec![SheetSnapshot {
            path: std::path::PathBuf::from("a.standard_sch"),
            schematic: sheet,
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }],
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let opts = PdfOptions {
        generate_nets_info: true,
        bookmark_pins: true,
        bookmark_net_labels: false,
        bookmark_ports: false,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    let pins: Vec<&str> = items
        .iter()
        .filter(|i| i.title.starts_with("pin "))
        .map(|i| i.title.as_str())
        .collect();
    assert_eq!(
        pins,
        [
            "pin U1.1",
            "pin U1.10",
            "pin U1.2",
            "pin U1.3",
            "pin U1.7",
            "pin U1.9"
        ]
    );
    // Sheet -> Nets group -> 6 pins: the sheet owns 7 descendants.
    assert_eq!(descendant_count(&items, 0), 7);
}

/// One sheet per designator list, each part at the same spot.
fn ctx_with_parts(pages: &[&[&str]]) -> ExportContext {
    let sheets = pages
        .iter()
        .enumerate()
        .map(|(i, references)| {
            let mut sheet = empty_sheet();
            for reference in *references {
                sheet.symbols.push(Symbol {
                    reference: reference.to_string(),
                    position: Point::new(50.0, 50.0),
                    ..Default::default()
                });
            }
            SheetSnapshot {
                path: std::path::PathBuf::from(format!("sheet_{i}.standard_sch")),
                schematic: sheet,
                sheet_name: format!("Sheet{}", i + 1),
                sheet_number: i + 1,
                sheet_count: pages.len(),
                placement: Default::default(),
            }
        })
        .collect();
    ExportContext {
        sheets,
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    }
}

/// Titles of the children of `items[idx]`, in outline order.
fn child_titles(items: &[PendingBookmark], idx: usize) -> Vec<&str> {
    items[idx]
        .children
        .iter()
        .map(|&c| items[c].title.as_str())
        .collect()
}

/// The default options keep the outline the exporter always wrote: the
/// sheet, with its Components and Nets groups under it.
#[test]
fn default_options_keep_the_sheet_components_nets_outline() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        physical_designators: false,
        ..PdfOptions::default()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Sheet 1: Power", "Components", "Nets", "R1", "/VCC"]
    );
    assert_eq!(child_titles(&items, 0), ["Components", "Nets"]);
    assert_eq!(child_titles(&items, 1), ["R1"]);
    assert_eq!(child_titles(&items, 2), ["/VCC"]);
}

#[test]
fn without_sheet_bookmarks_the_groups_sit_at_the_top_level() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        bookmark_sheets: false,
        bookmark_parts: true,
        generate_nets_info: true,
        bookmark_net_labels: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert!(
        !items.iter().any(|i| i.title.starts_with("Sheet")),
        "no sheet items"
    );
    let top: Vec<&str> = items
        .iter()
        .filter(|i| i.parent_idx.is_none())
        .map(|i| i.title.as_str())
        .collect();
    assert_eq!(top, ["Components", "Nets"]);
    assert_eq!(child_titles(&items, 0), ["R1"]);
    assert_eq!(child_titles(&items, 1), ["/VCC"]);
}

#[test]
fn sheet_bookmarks_alone_list_one_item_per_page() {
    let ctx = ctx_with_parts(&[&["R1"], &["R2"]]);
    let items = build_bookmarks(&ctx, &default_opts(), &[0, 1], 297.0, 210.0, 595.0);
    let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Sheet 1: Sheet1", "Sheet 2: Sheet2"]);
    assert_eq!(items[1].page_idx, 1);
}

/// Part bookmarks off writes no Components group and no part item.
#[test]
fn part_bookmarks_off_writes_no_part_items() {
    let ctx = ctx_with_one_sheet();
    let opts = PdfOptions {
        bookmark_parts: false,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0], 297.0, 210.0, 595.0);
    assert!(!items.iter().any(|i| i.title == "Components"));
    assert!(!items.iter().any(|i| i.title.starts_with("R1")));
    assert_eq!(items.len(), 1, "the sheet item only");
}

#[test]
fn alphabetical_parts_are_one_top_level_group_in_natural_order() {
    let ctx = ctx_with_parts(&[&["R10", "U1"], &["R2", "C3", "R1"]]);
    let opts = PdfOptions {
        bookmark_parts: true,
        part_bookmark_order: PartBookmarkOrder::Alphabetical,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &opts, &[0, 1], 297.0, 210.0, 595.0);
    assert_eq!(items[0].title, "Components");
    assert!(items[0].parent_idx.is_none());
    assert_eq!(child_titles(&items, 0), ["C3", "R1", "R2", "R10", "U1"]);
    // Each part still jumps to its own page.
    let page_of = |title: &str| items.iter().find(|i| i.title == title).map(|i| i.page_idx);
    assert_eq!(page_of("R10"), Some(0));
    assert_eq!(page_of("R2"), Some(1));
    // The sheets stay as top-level items with no Components group under them.
    let sheets: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].title.starts_with("Sheet"))
        .collect();
    assert_eq!(sheets.len(), 2);
    assert!(sheets.iter().all(|&i| items[i].children.is_empty()));
}

/// A to Z owns the Components group; global mode then moves only Nets.
#[test]
fn alphabetical_parts_leave_nets_to_the_global_setting() {
    let ctx = ctx_with_one_sheet();
    let alphabetical = PdfOptions {
        bookmark_parts: true,
        part_bookmark_order: PartBookmarkOrder::Alphabetical,
        generate_nets_info: true,
        bookmark_net_labels: true,
        ..default_opts()
    };
    let items = build_bookmarks(&ctx, &alphabetical, &[0], 297.0, 210.0, 595.0);
    let sheet = items
        .iter()
        .position(|i| i.title == "Sheet 1: Power")
        .expect("sheet item");
    assert_eq!(
        child_titles(&items, sheet),
        ["Nets"],
        "Nets stays per sheet"
    );

    let global = PdfOptions {
        global_bookmarks: true,
        ..alphabetical
    };
    let items = build_bookmarks(&ctx, &global, &[0], 297.0, 210.0, 595.0);
    let top: Vec<&str> = items
        .iter()
        .filter(|i| i.parent_idx.is_none())
        .map(|i| i.title.as_str())
        .collect();
    assert_eq!(top, ["Components", "Nets", "Sheet 1: Power"]);
}
