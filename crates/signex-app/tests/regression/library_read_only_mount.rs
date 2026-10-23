//! #329 — a library mounted read-only (the stock library) refuses every
//! write the Library Browser can dispatch, tells the user why in plain
//! language through the app's error card, and leaves its files
//! byte-identical.
//!
//! Real `LocalGitAdapter` library in a temp dir, real dispatcher. The
//! positive control mounts the same library normally and shows the same
//! message *does* write, so the assertions fail if the read-only mark is
//! removed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{Message, Signex};
use signex_app::library::messages::{EditorMsg, LibraryMessage};
use signex_app::library::mount::prepare_mount_read_only;
use signex_app::library::state::{EditorAddress, LibraryBrowserState, PrimitivePickerTarget};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveKind, PrimitiveRef, RowId,
};
use uuid::Uuid;

const TABLE: &str = "ICs";

fn stock_like_library(dir: &Path) -> (PathBuf, RowId) {
    let snxlib = dir.join("stock.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Signex Stock".into(),
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
        parameters: ParamMap::new(),
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
    (snxlib, row_id)
}

fn bytes_of(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                out.insert(
                    path.strip_prefix(root).expect("under root").to_path_buf(),
                    std::fs::read(&path).expect("read"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn app_with_browser(snxlib: &Path, read_only: bool) -> Signex {
    let (mut app, _boot) = Signex::new();
    if read_only {
        app.library
            .open_library_read_only(snxlib.to_path_buf())
            .expect("mount read-only");
    } else {
        app.library
            .open_library(snxlib.to_path_buf())
            .expect("mount");
    }
    let mut browser = LibraryBrowserState::new(snxlib.to_path_buf());
    browser.active_table = Some(TABLE.to_string());
    app.library
        .library_browsers
        .insert(snxlib.to_path_buf(), browser);
    app
}

fn write_attempts(snxlib: &Path, row_id: RowId) -> Vec<LibraryMessage> {
    vec![
        LibraryMessage::BrowserAddComponent {
            library_path: snxlib.to_path_buf(),
            table: Some(TABLE.to_string()),
        },
        LibraryMessage::BrowserDeleteRowConfirm {
            library_path: snxlib.to_path_buf(),
            table: TABLE.to_string(),
            row_id,
        },
        LibraryMessage::BrowserDeleteTable {
            library_path: snxlib.to_path_buf(),
            table: TABLE.to_string(),
        },
        LibraryMessage::BrowserDeleteClass {
            library_path: snxlib.to_path_buf(),
            key: "IC".to_string(),
        },
        LibraryMessage::BrowserBeginAddTable {
            library_path: snxlib.to_path_buf(),
        },
    ]
}

#[test]
fn write_attempts_on_a_read_only_mount_explain_and_leave_disk_untouched() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-ro-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);
    assert!(app.library.is_read_only(&snxlib));
    let before = bytes_of(tmp.path());

    for msg in write_attempts(&snxlib, row_id) {
        app.document_state.error_notice = None;
        let label = format!("{msg:?}");
        let _ = app.update(Message::Library(msg));

        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: no explanation reached the user"));
        assert_eq!(notice.title, "Library Is Read-Only", "{label}");
        assert!(
            notice
                .detail
                .contains("ships with Signex and is not editable"),
            "{label}: {}",
            notice.detail
        );
        assert!(
            !notice.detail.to_lowercase().contains("permission"),
            "{label}: no permission error may reach the user"
        );
    }

    assert_eq!(
        before,
        bytes_of(tmp.path()),
        "the stock library's files must be byte-identical after the attempts"
    );
    assert!(
        app.library
            .library_browsers
            .get(&snxlib)
            .is_some_and(|b| b.adding_table.is_none()),
        "a refused Begin-Add-Table must not open the inline form"
    );
}

/// Positive control — same library, ordinary mount: the same message
/// writes and raises no card.
#[test]
fn the_same_attempts_on_an_ordinary_mount_write() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-rw-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, _row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, false);
    assert!(!app.library.is_read_only(&snxlib));
    let before = bytes_of(tmp.path());

    let _ = app.update(Message::Library(LibraryMessage::BrowserAddComponent {
        library_path: snxlib.clone(),
        table: Some(TABLE.to_string()),
    }));

    assert!(app.document_state.error_notice.is_none());
    assert_ne!(before, bytes_of(tmp.path()), "Add Component must write");
}

/// The off-thread mount path marks the mount too.
#[test]
fn a_prepared_read_only_mount_lands_read_only() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-prep-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, _row_id) = stock_like_library(tmp.path());
    let prepared = prepare_mount_read_only(&snxlib).expect("prepare");
    let (mut app, _boot) = Signex::new();
    app.library.mount_prepared(prepared).expect("mount");
    assert!(app.library.is_read_only(&snxlib));
}

fn picker_targets(snxlib: &Path, row_id: RowId) -> Vec<PrimitivePickerTarget> {
    let address = || EditorAddress::new(snxlib.to_path_buf(), TABLE.to_string(), row_id);
    vec![
        PrimitivePickerTarget::BrowserRow(address()),
        PrimitivePickerTarget::PreviewRow(address()),
        PrimitivePickerTarget::EditRowModal(address()),
    ]
}

/// Pick Symbol / Pick Footprint on a stock row must explain itself
/// instead of opening a picker whose result could only fail silently.
#[test]
fn opening_a_primitive_picker_on_a_read_only_row_explains_and_opens_nothing() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-pick-ro-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);
    let before = bytes_of(tmp.path());

    for target in picker_targets(&snxlib, row_id) {
        app.document_state.error_notice = None;
        let label = format!("{target:?}");
        let _ = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Symbol,
            target,
        }));
        assert!(app.library.primitive_picker.is_none(), "{label}");
        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: no explanation reached the user"));
        assert_eq!(notice.title, "Library Is Read-Only", "{label}");
    }
    assert_eq!(before, bytes_of(tmp.path()));
}

/// Positive control — an ordinary mount opens the picker, no card.
#[test]
fn opening_a_primitive_picker_on_an_ordinary_row_opens_it() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-pick-rw-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, false);

    let target = PrimitivePickerTarget::BrowserRow(EditorAddress::new(
        snxlib.clone(),
        TABLE.to_string(),
        row_id,
    ));
    let _ = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
        kind: PrimitiveKind::Symbol,
        target,
    }));
    assert!(app.library.primitive_picker.is_some());
    assert!(app.document_state.error_notice.is_none());
}

/// A Component Preview save aimed at a read-only library reaches the
/// user through the card, not only the log.
#[test]
fn a_component_preview_save_on_a_read_only_library_explains() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-save-ro-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);
    let before = bytes_of(tmp.path());

    let _ = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
        msg: EditorMsg::SaveDraft,
    }));

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("save refusal must reach the user");
    assert_eq!(notice.title, "Library Is Read-Only");
    assert_eq!(before, bytes_of(tmp.path()));
}
