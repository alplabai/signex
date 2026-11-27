//! #1116 (mockup row 204): the project's image assets on the headless path
//! (`list_image_assets`) and the GUI path (the scan behind the Projects
//! tree and the status bar count), the missing-image ERC check's file
//! reads, and the bytes a PDF export embeds.

use std::collections::HashMap;

use signex_types::image_asset::content_hash;
use signex_types::schematic::{Point, SchDrawing, SchImage, SchematicSheet};
use uuid::Uuid;

use super::tests::{app_in, empty_sheet, land_image_messages, png, write};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

/// A sheet whose images name `present.png` (on disk) and `gone.png` (not).
fn sheet(present_hash: &str) -> SchematicSheet {
    let mut sheet = empty_sheet();
    for (i, (file, hash)) in [("present.png", present_hash), ("gone.png", "ab")]
        .into_iter()
        .enumerate()
    {
        sheet.drawings.push(SchDrawing::Image(SchImage::new(
            Uuid::from_u128(i as u128 + 1),
            Point::new(10.16 + 30.0 * i as f64, 10.16),
            (20.0, 10.0),
            format!("assets/images/{file}"),
            hash.to_string(),
        )));
    }
    sheet
}

#[test]
fn list_image_assets_answers_every_file_with_its_uses_and_the_missing_count() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = png(40, 20);
    write(&dir.path().join("assets/images/present.png"), &bytes);
    write(&dir.path().join("assets/images/unused.png"), &png(8, 8));
    let (mut app, path) = app_in(dir.path(), sheet(&content_hash(&bytes)));
    let id = AppCommandId::new("list_image_assets").expect("a catalog id");
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&id, &[&document]).expect("operands parse");

    let outcome = match app.run_query(&id, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    };

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["folder"], "assets/images");
    assert_eq!(answer["missing"], 1);
    let assets = answer["assets"].as_array().expect("a list");
    let by_file: HashMap<&str, &serde_json::Value> = assets
        .iter()
        .map(|a| (a["file"].as_str().unwrap(), a))
        .collect();
    assert_eq!(by_file["assets/images/present.png"]["uses"], 1);
    assert_eq!(by_file["assets/images/present.png"]["format"], "png");
    assert_eq!(by_file["assets/images/present.png"]["width_px"], 40);
    assert_eq!(by_file["assets/images/gone.png"]["missing"], true);
    assert_eq!(by_file["assets/images/unused.png"]["uses"], 0);
    assert_eq!(by_file["assets/images/unused.png"]["missing"], false);
}

#[test]
fn the_scan_marks_the_status_bar_count_and_the_panel_facts() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = png(40, 20);
    write(&dir.path().join("assets/images/present.png"), &bytes);
    let (mut app, path) = app_in(dir.path(), sheet(&content_hash(&bytes)));
    assert_eq!(app.missing_asset_count(), 0, "nothing is scanned yet");

    let task = app.request_image_loads();
    land_image_messages(&mut app, task);

    assert_eq!(app.missing_asset_count(), 1);
    let root = app.image_root(&path);
    let report = app
        .ui_state
        .image_cache
        .assets
        .report(&root)
        .expect("a report");
    assert!(report.is_missing("assets/images/gone.png"));
    assert!(!report.is_missing("assets/images/present.png"));
    // A second look with nothing changed scans nothing.
    assert_eq!(app.request_asset_scan().units(), 0);
}

#[test]
fn a_root_with_no_image_is_not_scanned_until_asked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_in(dir.path(), empty_sheet());
    assert_eq!(app.request_asset_scan().units(), 0);
    assert!(
        app.scan_active_assets(true).units() > 0,
        "Place ▸ Graphic… scans"
    );
}

#[test]
fn the_erc_run_names_the_missing_and_changed_files_but_not_a_pick_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = png(40, 20);
    write(&dir.path().join("assets/images/present.png"), &bytes);
    let path = dir.path().join("main.snxsch");
    let mut sheets = HashMap::new();
    let mut with_changed = sheet(&content_hash(&bytes));
    with_changed.drawings.push(SchDrawing::Image(SchImage::new(
        Uuid::from_u128(9),
        Point::new(80.0, 10.16),
        (20.0, 10.0),
        "assets/images/present.png".into(),
        "stale".into(),
    )));
    sheets.insert(path.clone(), with_changed);

    let config = super::with_missing_images(
        &signex_erc::ErcConfig::default(),
        Some(dir.path()),
        &sheets,
        &[],
    );
    assert!(config.missing_images.contains("assets/images/gone.png"));
    assert!(config.missing_images.contains("assets/images/present.png"));

    let waiting = [("assets/images/gone.png".to_string(), "ab".to_string())];
    let mut only_gone = HashMap::new();
    only_gone.insert(path, sheet(&content_hash(&bytes)));
    let config = super::with_missing_images(
        &signex_erc::ErcConfig::default(),
        Some(dir.path()),
        &only_gone,
        &waiting,
    );
    assert!(
        config.missing_images.is_empty(),
        "{:?}",
        config.missing_images
    );
}

#[test]
fn a_pdf_export_reads_the_placed_files_and_takes_a_waiting_pick_from_memory() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = png(40, 20);
    write(&dir.path().join("assets/images/present.png"), &bytes);
    let (mut app, path) = app_in(dir.path(), sheet(&content_hash(&bytes)));
    app.document_state
        .image_copies
        .push(signex_types::image_asset::PendingImageCopy {
            sheet: path.clone(),
            root: dir.path().to_path_buf(),
            file: "assets/images/gone.png".into(),
            hash: "ab".into(),
            bytes: std::sync::Arc::from(&b"picked"[..]),
        });
    let schematic = app
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .clone();
    let ctx = signex_output::ExportContext {
        sheets: vec![signex_output::SheetSnapshot {
            path,
            schematic,
            sheet_name: "main".into(),
            sheet_number: 1,
            sheet_count: 1,
            placement: Default::default(),
        }],
        metadata: Default::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };

    let reads = app.export_image_reads(&ctx);
    assert_eq!(
        reads.counts(),
        (1, 1),
        "one file to read, one pick in memory"
    );
    let options = reads.into_options(signex_output::PdfOptions::default());

    assert_eq!(options.images.len(), 2);
}
