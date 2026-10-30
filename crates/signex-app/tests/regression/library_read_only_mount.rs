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

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
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
    stock_like_library_named(dir, "Signex Stock")
}

/// Same fixture under a caller-chosen display name, for tests that count
/// log lines in the process-wide diagnostics ring and so need a name no
/// concurrently running test also refuses writes against.
fn stock_like_library_named(dir: &Path, name: &str) -> (PathBuf, RowId) {
    let snxlib = dir.join("stock.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: name.into(),
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
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        server_revision: None,
        internal_pn: InternalPn::new("SNX-IC-00001"),
        class: ComponentClass::new("IC"),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
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
        LibraryMessage::BrowserConfirmRenameTable {
            library_path: snxlib.to_path_buf(),
        },
        LibraryMessage::BrowserBeginAddTable {
            library_path: snxlib.to_path_buf(),
        },
        LibraryMessage::BrowserCellCommit {
            library_path: snxlib.to_path_buf(),
            table: TABLE.to_string(),
            row_id,
            column: "mpn".to_string(),
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
        let task = app.update(Message::Library(msg));
        crate::drive::drive(&mut app, task);

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
    let _notes = crate::library_new_component::new_row_notes_lock();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-rw-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, _row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, false);
    assert!(!app.library.is_read_only(&snxlib));
    let before = bytes_of(tmp.path());

    let task = app.update(Message::Library(LibraryMessage::BrowserAddComponent {
        library_path: snxlib.clone(),
        table: Some(TABLE.to_string()),
    }));
    crate::drive::drive(&mut app, task);

    assert!(app.document_state.error_notice.is_none());
    assert_ne!(before, bytes_of(tmp.path()), "Add Component must write");
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
        let task = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Symbol,
            target,
        }));
        crate::drive::drive(&mut app, task);
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
    let task = app.update(Message::Library(LibraryMessage::OpenPrimitivePicker {
        kind: PrimitiveKind::Symbol,
        target,
    }));
    crate::drive::drive(&mut app, task);
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

    let task = app.update(Message::Library(LibraryMessage::EditorEvent {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
        msg: EditorMsg::SaveDraft,
    }));
    crate::drive::drive(&mut app, task);

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("save refusal must reach the user");
    assert_eq!(notice.title, "Library Is Read-Only");
    assert_eq!(before, bytes_of(tmp.path()));
}

/// Scrolling the SPICE deck on a read-only row is navigation: no card,
/// no warning. An editing action in the same deck is still refused.
#[test]
fn scrolling_the_spice_deck_on_a_read_only_row_is_not_refused() {
    use iced::widget::text_editor::{Action, Edit};
    ensure_logger();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-scroll-ro-")
        .tempdir()
        .expect("tempdir");
    // The diagnostics ring is shared by every test in this binary, and
    // other tests refuse writes against "Signex Stock" in parallel. A
    // unique name keeps this count about this test's library only.
    const NAME: &str = "Signex Stock (scroll test)";
    let (snxlib, row_id) = stock_like_library_named(tmp.path(), NAME);
    let mut app = app_with_browser(&snxlib, true);
    let before = bytes_of(tmp.path());
    let refusals = || {
        diagnostics::recent_entries()
            .iter()
            .filter(|e| e.message.contains("write refused") && e.message.contains(NAME))
            .count()
    };
    let event = |action| {
        Message::Library(LibraryMessage::EditorEvent {
            library_path: snxlib.clone(),
            table: TABLE.to_string(),
            row_id,
            msg: EditorMsg::SimBodyAction(action),
        })
    };
    let warned_before = refusals();

    let task = app.update(event(Action::Scroll { lines: 3 }));
    crate::drive::drive(&mut app, task);
    assert!(app.document_state.error_notice.is_none());
    assert_eq!(refusals(), warned_before, "scroll must not log a warning");

    let task = app.update(event(Action::Edit(Edit::Insert('x'))));
    crate::drive::drive(&mut app, task);
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("an edit must still be refused");
    assert_eq!(notice.title, "Library Is Read-Only");
    assert!(refusals() > warned_before);
    assert_eq!(before, bytes_of(tmp.path()));
}

// ── Primitive editors opened from inside a read-only mount ──────────

fn ensure_logger() {
    let _ = diagnostics::init_logging();
}

fn logged(fragment: &str) -> bool {
    diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(fragment))
}

/// Write a `.snxfpt` at `path` (parents created) and return its bytes.
fn write_footprint(path: &Path) -> Vec<u8> {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("stock_fp"));
    std::fs::write(path, file.to_toml_string().expect("encode")).expect("write footprint");
    std::fs::read(path).expect("read back")
}

/// Open `open_path`, rename the footprint, Ctrl+S it. Returns the app.
fn open_edit_save(snxlib: &Path, open_path: &Path) -> Signex {
    let mut app = app_with_browser(snxlib, true);
    // The read runs on a worker; settle runs it and installs the tab.
    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(open_path.to_path_buf()))),
    );
    assert!(
        app.document_state.footprint_editors.contains_key(open_path),
        "the footprint must open in an editor tab"
    );
    let task = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: open_path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SetName("renamed".into())),
    }));
    crate::drive::drive(&mut app, task);
    // The write runs off the UI thread; settle runs its task to completion.
    crate::settle_support::settle(
        &mut app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: open_path.to_path_buf(),
            msg: PrimitiveEdit::Save,
        }),
    );
    app
}

/// `save_primitive_tab_at` writes the container itself, bypassing the
/// adapter, so the dispatcher guard is the only thing protecting the
/// stock `.snxfpt` when the install directory is writable. Goes RED if
/// `read_only_write_error` is removed from it.
#[test]
fn saving_a_primitive_opened_from_a_read_only_mount_leaves_the_file_untouched() {
    ensure_logger();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-prim-")
        .tempdir()
        .expect("tempdir");
    let stock_dir = tmp.path().join("stock");
    std::fs::create_dir_all(&stock_dir).expect("mkdir");
    let (snxlib, _row_id) = stock_like_library(&stock_dir);
    let fp_path = stock_dir.join("footprints").join("stock_fp.snxfpt");
    let original = write_footprint(&fp_path);
    let before = bytes_of(tmp.path());

    let _app = open_edit_save(&snxlib, &fp_path);

    assert_eq!(
        original,
        std::fs::read(&fp_path).expect("read"),
        "the stock footprint must be byte-identical after edit + save"
    );
    assert_eq!(before, bytes_of(tmp.path()));
    assert!(
        logged("ships with Signex and is not editable"),
        "the refused save must reach the diagnostics ring"
    );
}

/// The same file reached through a `..` spelling that a lexical prefix
/// comparison does not see inside the mount.
#[test]
fn a_dotdot_spelling_of_a_stock_path_cannot_bypass_the_guard() {
    ensure_logger();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-dotdot-")
        .tempdir()
        .expect("tempdir");
    let stock_dir = tmp.path().join("stock");
    std::fs::create_dir_all(&stock_dir).expect("mkdir");
    let (snxlib, _row_id) = stock_like_library(&stock_dir);
    let fp_path = stock_dir.join("footprints").join("stock_fp.snxfpt");
    let original = write_footprint(&fp_path);
    std::fs::create_dir_all(tmp.path().join("elsewhere")).expect("mkdir");
    let sneaky = tmp
        .path()
        .join("elsewhere")
        .join("..")
        .join("stock")
        .join("footprints")
        .join("stock_fp.snxfpt");
    assert!(!sneaky.starts_with(&snxlib), "premise: lexically outside");

    let _app = open_edit_save(&snxlib, &sneaky);

    assert_eq!(original, std::fs::read(&fp_path).expect("read"));
}

/// Positive control — a footprint outside any read-only mount saves.
#[test]
fn a_primitive_outside_the_read_only_mount_still_saves() {
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-prim-rw-")
        .tempdir()
        .expect("tempdir");
    let stock_dir = tmp.path().join("stock");
    std::fs::create_dir_all(&stock_dir).expect("mkdir");
    let (snxlib, _row_id) = stock_like_library(&stock_dir);
    let fp_path = tmp.path().join("mine").join("mine.snxfpt");
    let original = write_footprint(&fp_path);

    let _app = open_edit_save(&snxlib, &fp_path);

    assert_ne!(original, std::fs::read(&fp_path).expect("read"));
}

// ── Refusal reaches the Messages panel ──────────────────────────────

#[test]
fn a_refused_write_is_logged_to_the_diagnostics_ring() {
    ensure_logger();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-log-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);

    let task = app.update(Message::Library(LibraryMessage::BrowserDeleteRowConfirm {
        library_path: snxlib.clone(),
        table: TABLE.to_string(),
        row_id,
    }));
    crate::drive::drive(&mut app, task);

    assert!(
        logged("[library] write refused: Signex Stock is mounted read-only"),
        "the refusal must be in the diagnostics ring, not only on the card"
    );
}

// ── New Component never targets a read-only library ─────────────────

#[test]
fn new_component_skips_a_read_only_library_and_uses_the_writable_one() {
    let _notes = crate::library_new_component::new_row_notes_lock();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-newc-")
        .tempdir()
        .expect("tempdir");
    let stock_dir = tmp.path().join("stock");
    let mine_dir = tmp.path().join("mine");
    std::fs::create_dir_all(&stock_dir).expect("mkdir");
    std::fs::create_dir_all(&mine_dir).expect("mkdir");
    let (stock, _) = stock_like_library(&stock_dir);
    let (mine, _) = stock_like_library(&mine_dir);
    let (mut app, _boot) = Signex::new();
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    app.library.open_library(mine.clone()).expect("mount mine");
    let stock_before = bytes_of(&stock_dir);
    let mine_before = bytes_of(&mine_dir);

    let task = app.update(Message::Library(LibraryMessage::NewComponent));
    crate::drive::drive(&mut app, task);

    assert_eq!(
        stock_before,
        bytes_of(&stock_dir),
        "stock must be untouched"
    );
    assert_ne!(mine_before, bytes_of(&mine_dir), "the row lands in mine");
}

#[test]
fn new_component_with_only_read_only_libraries_writes_nothing() {
    ensure_logger();
    let tmp = tempfile::Builder::new()
        .prefix("signex-329-newc-ro-")
        .tempdir()
        .expect("tempdir");
    let (snxlib, _row_id) = stock_like_library(tmp.path());
    let mut app = app_with_browser(&snxlib, true);
    let before = bytes_of(tmp.path());

    let task = app.update(Message::Library(LibraryMessage::NewComponent));
    crate::drive::drive(&mut app, task);

    assert_eq!(before, bytes_of(tmp.path()), "nothing is written");
}
