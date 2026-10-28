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
use signex_app::library::messages::{BrowserEditMsg, LibraryMessage};
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
    library_with_measurement(dir, 3.3, "V")
}

/// Same fixture with the stored measurement chosen by the caller.
fn library_with_measurement(dir: &std::path::Path, value: f64, unit: &str) -> (PathBuf, RowId) {
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
            value,
            unit: unit.into(),
        },
    );

    let now = chrono::Utc::now();
    let row = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("SNX-IC-00001"),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
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
        server_revision: None,
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

/// Commit `typed` into the cell holding `{value, unit}` and return what
/// the library holds afterwards.
fn commit_into_measurement(value: f64, unit: &str, typed: &str) -> ParamValue {
    let tmp = tempfile::Builder::new()
        .prefix("signex-1572-cell-prefix-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = library_with_measurement(tmp.path(), value, unit);
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.clone())
        .expect("mount library");
    let mut browser = LibraryBrowserState::new(snxlib.clone());
    browser.active_table = Some(TABLE.to_string());
    browser
        .cell_edit
        .insert((row_id, format!("parameters.{PARAM}")), typed.to_string());
    app.library.library_browsers.insert(snxlib.clone(), browser);
    let _ = app.update(Message::Library(LibraryMessage::BrowserCellCommit {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
        column: format!("parameters.{PARAM}"),
    }));
    param_on_disk(&snxlib, row_id)
}

/// The row as the library holds it on disk (hash included).
fn row_on_disk(snxlib: &std::path::Path, row_id: RowId) -> signex_library::ComponentRow {
    LocalGitAdapter::open(snxlib)
        .expect("re-open library")
        .read_row(TABLE, row_id)
        .expect("read row")
}

/// The cell's buffer is the number in the prefix the cell shows (the
/// canonical `{4700, "ohm"}` shows `4.7 kΩ`), and what is stored is the
/// canonical base value.
#[test]
fn a_number_typed_into_a_canonical_cell_is_in_the_shown_prefix() {
    let stored = commit_into_measurement(4700.0, "ohm", "5.6");
    assert_eq!(
        stored,
        ParamValue::Measurement {
            value: 5600.0,
            unit: "ohm".into()
        }
    );
}

/// A legacy `{4.7, "kΩ"}` cell also shows `4.7 kΩ`; typing 5.6 stores
/// 5600 ohm, never `{5.6, "ohm"}` (#1606).
#[test]
fn a_number_typed_into_a_legacy_cell_stores_the_base_value() {
    let stored = commit_into_measurement(4.7, "kΩ", "5.6");
    assert_eq!(
        stored,
        ParamValue::Measurement {
            value: 5600.0,
            unit: "ohm".into()
        }
    );
}

/// Retyping the number a cell shows leaves the row as stored: the legacy
/// spelling stays, and so does the content hash.
#[test]
fn retyping_the_shown_number_leaves_the_row_and_its_hash() {
    for (value, unit) in [(4700.0, "ohm"), (4.7, "kΩ"), (0.1, "uF")] {
        let stored = ParamValue::Measurement {
            value,
            unit: unit.into(),
        };
        let shown = stored.measurement_parts().expect("known unit").mantissa;
        let tmp = tempfile::Builder::new()
            .prefix("signex-1572-cell-noop-")
            .tempdir()
            .expect("tempdir");
        let (snxlib, row_id) = library_with_measurement(tmp.path(), value, unit);
        // The fixture row is inserted unhashed; what its content hashes to
        // is what an untouched row must still hash to.
        let before = signex_library::hash_row_content(&row_on_disk(&snxlib, row_id)).expect("hash");

        let after = commit_into_measurement_at(&snxlib, row_id, &shown);

        assert_eq!(after, stored, "{value} {unit}");
        assert_eq!(
            row_on_disk(&snxlib, row_id).content_hash,
            before,
            "{value} {unit}"
        );
    }
}

fn commit_into_measurement_at(snxlib: &std::path::Path, row_id: RowId, typed: &str) -> ParamValue {
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.to_path_buf())
        .expect("mount library");
    let mut browser = LibraryBrowserState::new(snxlib.to_path_buf());
    browser.active_table = Some(TABLE.to_string());
    browser
        .cell_edit
        .insert((row_id, format!("parameters.{PARAM}")), typed.to_string());
    app.library
        .library_browsers
        .insert(snxlib.to_path_buf(), browser);
    let _ = app.update(Message::Library(LibraryMessage::BrowserCellCommit {
        library_path: snxlib.to_path_buf(),
        table: TABLE.to_string(),
        row_id,
        column: format!("parameters.{PARAM}"),
    }));
    param_on_disk(snxlib, row_id)
}

/// A unit the editor does not know keeps the raw string next to the number.
#[test]
fn an_unknown_unit_cell_keeps_its_unit_string() {
    let stored = commit_into_measurement(1.5, "furlong", "2");
    assert_eq!(
        stored,
        ParamValue::Measurement {
            value: 2.0,
            unit: "furlong".into()
        }
    );
}

/// The Edit Component Details modal, opened on the row, driven by `steps`
/// and saved. Returns what the modal seeded for the parameter (buffer and
/// prefix) and the app, so a test can read the draft error.
fn edit_in_modal(
    snxlib: &std::path::Path,
    row_id: RowId,
    steps: Vec<BrowserEditMsg>,
) -> (Signex, Option<(String, String)>) {
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library(snxlib.to_path_buf())
        .expect("mount library");
    let mut browser = LibraryBrowserState::new(snxlib.to_path_buf());
    browser.active_table = Some(TABLE.to_string());
    app.library
        .library_browsers
        .insert(snxlib.to_path_buf(), browser);
    let _ = app.update(Message::Library(LibraryMessage::BrowserOpenEditModal {
        library_path: snxlib.to_path_buf(),
        table: TABLE.to_string(),
        row_id,
    }));
    let seeded = app
        .library
        .library_browsers
        .get(snxlib)
        .and_then(|b| b.edit_modal.as_ref())
        .and_then(|m| m.param_buf.get(PARAM).cloned());
    for msg in steps {
        let _ = app.update(Message::Library(LibraryMessage::BrowserEdit {
            library_path: snxlib.to_path_buf(),
            msg,
        }));
    }
    (app, seeded)
}

fn modal_error(app: &Signex, snxlib: &std::path::Path) -> Option<String> {
    app.library
        .library_browsers
        .get(snxlib)
        .and_then(|b| b.edit_modal.as_ref())
        .and_then(|m| m.error.clone())
}

fn set_number(text: &str) -> BrowserEditMsg {
    BrowserEditMsg::SetParamValue {
        key: PARAM.into(),
        value: text.into(),
    }
}

fn commit_and_save() -> Vec<BrowserEditMsg> {
    vec![
        BrowserEditMsg::CommitParam { key: PARAM.into() },
        BrowserEditMsg::Save,
    ]
}

/// #1606: a legacy `{4.7, "kΩ"}` opens as 4.7 with the k prefix and the
/// ohm unit. Retyping the same number changes nothing (bytes and hash);
/// typing 10 stores 10000 ohm, not 10 ohm.
#[test]
fn the_modal_edits_a_legacy_kilo_ohm_value_as_4_7_with_the_k_prefix() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (snxlib, row_id) = library_with_measurement(tmp.path(), 4.7, "kΩ");
    let before = signex_library::hash_row_content(&row_on_disk(&snxlib, row_id)).expect("hash");

    // Unchanged retype.
    let mut steps = vec![set_number("4.7")];
    steps.extend(commit_and_save());
    let (_app, seeded) = edit_in_modal(&snxlib, row_id, steps);
    assert_eq!(seeded, Some(("4.7".to_string(), "ohm".to_string())));
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 4.7,
            unit: "kΩ".into()
        }
    );
    assert_eq!(row_on_disk(&snxlib, row_id).content_hash, before);

    // A real edit.
    let mut steps = vec![set_number("10")];
    steps.extend(commit_and_save());
    let _ = edit_in_modal(&snxlib, row_id, steps);
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 10_000.0,
            unit: "ohm".into()
        }
    );
}

/// Canonical `{4700, "ohm"}`: 4.7 with the M picked stores 4.7e6 ohm.
#[test]
fn the_modal_stores_the_picked_prefix_as_a_base_value() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (snxlib, row_id) = library_with_measurement(tmp.path(), 4700.0, "ohm");
    let steps = vec![
        set_number("4.7"),
        BrowserEditMsg::ParamPrefixPicked {
            key: PARAM.into(),
            prefix: signex_types::quantity::SiPrefix::Mega,
        },
        BrowserEditMsg::Save,
    ];
    let _ = edit_in_modal(&snxlib, row_id, steps);
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 4_700_000.0,
            unit: "ohm".into()
        }
    );
}

/// `4k7` is refused in the number field: nothing is stored, and the modal
/// says why.
#[test]
fn the_modal_refuses_a_prefix_typed_into_the_number_field() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (snxlib, row_id) = library_with_measurement(tmp.path(), 4700.0, "ohm");
    let mut steps = vec![set_number("4k7")];
    steps.extend(commit_and_save());
    let (app, _) = edit_in_modal(&snxlib, row_id, steps);
    assert!(modal_error(&app, &snxlib).is_some(), "the refusal is shown");
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 4700.0,
            unit: "ohm".into()
        }
    );
}

/// An unknown unit opens as the raw number with its own unit text and no
/// prefix, and keeps that string when the number changes.
#[test]
fn the_modal_keeps_the_text_editor_for_an_unknown_unit() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (snxlib, row_id) = library_with_measurement(tmp.path(), 1.5, "furlong");
    let mut steps = vec![set_number("2")];
    steps.extend(commit_and_save());
    let (_app, seeded) = edit_in_modal(&snxlib, row_id, steps);
    assert_eq!(seeded, Some(("1.5".to_string(), "furlong".to_string())));
    assert_eq!(
        param_on_disk(&snxlib, row_id),
        ParamValue::Measurement {
            value: 2.0,
            unit: "furlong".into()
        }
    );
}
