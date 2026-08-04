//! #599 — an inline Library Browser cell commit must not retype a
//! parameter.
//!
//! A cell holding `Measurement { 3.3, "V" }` edited to something that is
//! not an `f64` used to be written to the library as
//! `ParamValue::Text(buffer)`: the cell rendered the typed text so it
//! looked committed, while the numeric type and the unit that every
//! numeric facet filter, range query, sort and BOM column keys on were
//! gone from the `.snxlib` on disk.
//!
//! This drives the real dispatcher against a real `LocalGitAdapter`
//! library and then re-reads the row from disk through a fresh adapter,
//! so it fails if the refusal is removed.

use std::path::PathBuf;

use signex_app::app::{Message, Signex};
use signex_app::library::messages::LibraryMessage;
use signex_app::library::state::LibraryBrowserState;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, ParamValue, PlmReserved, PrimitiveRef, RowId,
};
use uuid::Uuid;

const TABLE: &str = "ICs";
const PARAM: &str = "Supply Voltage";
/// Engineering shorthand a user reasonably types into a numeric cell.
/// It is not an `f64`, so the cell must refuse it.
const UNPARSEABLE: &str = "4k7";

/// A `.snxlib` with one row whose `Supply Voltage` is a typed
/// measurement. Version control off — `LocalGitAdapter::open` treats a
/// missing `.git/` as "no version control" and every mutation is a
/// best-effort commit that no-ops, which keeps the fixture cheap while
/// still writing real bytes through the real writer.
fn library_with_a_measurement_row(dir: &std::path::Path) -> (PathBuf, RowId) {
    let snxlib = dir.join("cells.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "cells".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &snxlib,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
    let lib_id = adapter.library_id();

    let mut parameters = ParamMap::new();
    parameters.insert(
        PARAM.into(),
        ParamValue::Measurement {
            value: 3.3,
            unit: "V".into(),
        },
    );

    let now = chrono::Utc::now();
    let row = ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("SNX-IC-00001"),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme Semi", "SNX-IC-00001"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    };
    let row_id = RowId::from_uuid(row.row_id);
    adapter.insert_row(TABLE, row, "seed").expect("insert row");
    drop(adapter);

    (snxlib, row_id)
}

/// The parameter as it now stands on disk, read back through a fresh
/// adapter rather than the app's in-memory cache.
fn param_on_disk(snxlib: &std::path::Path, row_id: RowId) -> ParamValue {
    LocalGitAdapter::open(snxlib)
        .expect("re-open library")
        .read_row(TABLE, row_id)
        .expect("read row")
        .parameters
        .get(PARAM)
        .expect("the row keeps its parameter key")
        .clone()
}

#[test]
fn an_unparseable_buffer_does_not_retype_a_measurement_parameter() {
    // Arrange — a mounted library with a typed cell, and a browser tab
    // whose edit buffer holds something that is not a number.
    let tmp = tempfile::Builder::new()
        .prefix("signex-599-cell-commit-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());

    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    let mut browser = LibraryBrowserState::new(snxlib.clone());
    browser.active_table = Some(TABLE.to_string());
    browser.cell_edit.insert(
        (row_id, format!("parameters.{PARAM}")),
        UNPARSEABLE.to_string(),
    );
    app.library.library_browsers.insert(snxlib.clone(), browser);

    // Act — Enter in the cell.
    let _ = app.update(Message::Library(LibraryMessage::BrowserCellCommit {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
        column: format!("parameters.{PARAM}"),
    }));

    // Assert — the stored value keeps its type AND its unit.
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 3.3,
            unit: "V".into(),
        },
        "an unparseable buffer must not rewrite the parameter as text"
    );

    // The cell keeps what the user typed, so the edit is not lost and
    // can be corrected in place.
    assert_eq!(
        app.library
            .library_browsers
            .get(&snxlib)
            .and_then(|b| b.cell_edit.get(&(row_id, format!("parameters.{PARAM}"))))
            .map(String::as_str),
        Some(UNPARSEABLE),
        "a refused commit must leave the typed text in the cell"
    );
}

#[test]
fn a_refused_cell_commit_reaches_the_messages_panel() {
    // Arrange
    let _ = signex_app::diagnostics::init_logging();
    let tmp = tempfile::Builder::new()
        .prefix("signex-599-cell-report-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_a_measurement_row(tmp.path());

    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    let mut browser = LibraryBrowserState::new(snxlib.clone());
    browser.active_table = Some(TABLE.to_string());
    browser.cell_edit.insert(
        (row_id, format!("parameters.{PARAM}")),
        UNPARSEABLE.to_string(),
    );
    app.library.library_browsers.insert(snxlib.clone(), browser);

    // Act
    let _ = app.update(Message::Library(LibraryMessage::BrowserCellCommit {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
        column: format!("parameters.{PARAM}"),
    }));

    // Assert — the refusal is on screen in the same frame, naming the
    // parameter, what was typed and what was kept. A silent refusal
    // would look like a dead Enter key. The panel compacts records to
    // 160 characters, so all three have to survive that budget.
    let panel = &app.document_state.panel_ctx.diagnostics;
    assert!(
        panel.iter().any(|entry| {
            entry.message.contains("browser cell commit refused")
                && entry.message.contains(&format!("parameter={PARAM}"))
                && entry.message.contains(&format!("typed={UNPARSEABLE}"))
                && entry.message.contains("kept=3.3 V")
        }),
        "the refusal must be in the Messages panel snapshot; got {:?}",
        panel.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}
