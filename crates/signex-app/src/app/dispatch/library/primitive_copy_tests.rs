//! #1841 — a symbol saved by Add New ▸ Symbol Library sits in the
//! project's own `SymbolLibrary.snxsym`, in no library. The picker offers
//! it, asks to copy it into the row's library, binds the copy, and Edit
//! Symbol then opens the copy.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{LibraryAdapter, PrimitiveKind, PrimitiveRef, RowId, Symbol, SymbolFile};
use signex_types::project::{LibraryEntry, LibraryEntryKind};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::messages::{LibraryMessage, PrimitivePickerMsg};
use crate::library::state::{EditorAddress, PrimitivePickerTarget};

const TABLE: &str = "generics";

struct Fixture {
    app: Signex,
    address: EditorAddress,
    library_id: Uuid,
    source: PathBuf,
    symbol: Symbol,
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let next = app.update(message);
        settle(app, next);
    }
}

fn picker(app: &mut Signex, msg: PrimitivePickerMsg) {
    let task = app.update(Message::Library(LibraryMessage::PrimitivePicker(msg)));
    settle(app, task);
}

fn fixture(dir: &Path) -> Fixture {
    signex_library::git_identity::test_identity::install();
    let snxlib = dir.join("cells-lib").join("cells.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
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
    .expect("init");
    let mut seed = test_row();
    seed.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    drop(adapter);
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));

    // What Add New ▸ Symbol Library saves, in the project folder.
    let symbol = Symbol::empty("GATEDRV8");
    let source = dir.join("SymbolLibrary.snxsym");
    std::fs::write(
        &source,
        SymbolFile::from_symbol(symbol.clone())
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();

    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: PathBuf::from("SymbolLibrary.snxsym"),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        });
    app.library.open_library(snxlib.clone()).expect("mount");
    let address = EditorAddress::new(snxlib, TABLE.into(), RowId::from_uuid(seed.row_id));
    Fixture {
        app,
        address,
        library_id,
        source,
        symbol,
    }
}

fn open_picker(f: &mut Fixture) {
    let task = f
        .app
        .update(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::BrowserRow(f.address.clone()),
        }));
    settle(&mut f.app, task);
}

fn bound_symbol(f: &Fixture) -> PrimitiveRef {
    f.app
        .library
        .set
        .get(f.library_id)
        .unwrap()
        .read_row(TABLE, f.address.row_id)
        .unwrap()
        .symbol_ref
}

#[test]
fn the_picker_offers_the_projects_own_symbol_file_marked_as_not_in_a_library() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());

    open_picker(&mut f);

    let picker = f.app.library.primitive_picker.as_ref().expect("open");
    assert_eq!(picker.loose.len(), 1);
    assert_eq!(picker.loose[0].name, "GATEDRV8");
    assert_eq!(picker.loose[0].file, f.source);
}

#[test]
fn copying_a_loose_symbol_binds_the_copy_and_edit_symbol_opens_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let original = std::fs::read(&f.source).unwrap();
    open_picker(&mut f);
    let chosen = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(chosen));
    assert!(
        f.app
            .library
            .primitive_picker
            .as_ref()
            .unwrap()
            .copy_confirm
            .is_some()
    );
    picker(&mut f.app, PrimitivePickerMsg::CopyConfirmed);

    let bound = bound_symbol(&f);
    assert_eq!(bound.library_id, f.library_id);
    assert_eq!(bound.uuid, f.symbol.uuid, "a free uuid is kept");
    let copy = f
        .app
        .library
        .primitive_file(&bound, "symbols", "snxsym")
        .expect("library is open");
    assert!(
        copy.is_file(),
        "the copy is a file of the library: {copy:?}"
    );
    assert!(copy.starts_with(f.address.library_path.parent().unwrap()));
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        original,
        "original untouched"
    );
    assert!(
        f.app.library.primitive_picker.is_none(),
        "picker closed by the bind"
    );
    let task = f.app.handle_open_primitive(copy.clone());
    settle(&mut f.app, task);
    assert!(f.app.document_state.symbol_editors.contains_key(&copy));
}

#[test]
fn declining_the_copy_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_symbol(&f);
    open_picker(&mut f);
    let chosen = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(chosen));
    picker(&mut f.app, PrimitivePickerMsg::CopyDeclined);

    assert_eq!(bound_symbol(&f), before);
    let state = f.app.library.primitive_picker.as_ref().unwrap();
    assert!(state.copy_confirm.is_none());
    assert!(
        !f.address
            .library_path
            .parent()
            .unwrap()
            .join("symbols")
            .exists()
    );
}

#[test]
fn a_symbol_whose_uuid_the_library_already_holds_is_copied_under_a_fresh_one() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let adapter = f.app.library.set.get(f.library_id).unwrap();
    adapter
        .save_symbol(f.symbol.clone(), "same uuid already there")
        .unwrap();
    open_picker(&mut f);
    let chosen = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(chosen));
    picker(&mut f.app, PrimitivePickerMsg::CopyConfirmed);

    assert_ne!(bound_symbol(&f).uuid, f.symbol.uuid);
}

#[test]
fn a_failed_copy_reaches_the_picker_and_binds_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_symbol(&f);
    open_picker(&mut f);
    let chosen = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();
    std::fs::remove_file(&f.source).unwrap();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(chosen));
    picker(&mut f.app, PrimitivePickerMsg::CopyConfirmed);

    assert_eq!(bound_symbol(&f), before);
    let state = f.app.library.primitive_picker.as_ref().expect("still open");
    assert!(
        state
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Could not read"))
    );
    assert!(!state.copying);
}

#[test]
fn a_copy_finishing_after_the_picker_was_replaced_binds_nothing_there() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_symbol(&f);
    open_picker(&mut f);
    let chosen = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(chosen));
    // The copy starts; the user cancels and opens a footprint picker for the
    // same row before it finishes.
    let running = f
        .app
        .update(Message::Library(LibraryMessage::PrimitivePicker(
            PrimitivePickerMsg::CopyConfirmed,
        )));
    picker(&mut f.app, PrimitivePickerMsg::Cancel);
    let task = f
        .app
        .update(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Footprint,
            target: PrimitivePickerTarget::BrowserRow(f.address.clone()),
        }));
    settle(&mut f.app, task);

    settle(&mut f.app, running);

    assert_eq!(bound_symbol(&f), before, "nothing was bound");
    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    assert_eq!(open.kind, PrimitiveKind::Footprint);
    assert!(
        f.address
            .library_path
            .parent()
            .unwrap()
            .join("symbols")
            .exists(),
        "the copy itself landed in the library"
    );
}

/// A second, unmounted library `x.snxlib` holding `symbols/Held.snxsym`,
/// listed in the project like the loose file (#1869).
fn unmounted_library_symbol(f: &mut Fixture, dir: &Path) -> PathBuf {
    let snxlib = dir.join("vendor").join("x.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "x".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            &snxlib,
            manifest,
            LibraryInitOptions {
                enable_git: false,
                use_lfs: false,
            },
        )
        .expect("init"),
    );
    drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    let held = snxlib.parent().unwrap().join("symbols").join("Held.snxsym");
    std::fs::create_dir_all(held.parent().unwrap()).unwrap();
    std::fs::write(
        &held,
        SymbolFile::from_symbol(Symbol::empty("HELD"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    f.app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: held,
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        });
    snxlib
}

fn headers(f: &Fixture) -> Vec<String> {
    let picker = f.app.library.primitive_picker.as_ref().unwrap();
    crate::library::primitive_picker::header_labels(&f.app.library, picker)
}

#[test]
fn a_symbol_of_an_unmounted_library_is_labelled_with_it_and_offers_open_not_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);

    let labels = headers(&f);
    assert!(
        labels.contains(&"In library x (not open): Held.snxsym  (1)".to_string()),
        "{labels:?}"
    );
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("Not in a library: SymbolLibrary.snxsym")),
        "the truly loose file keeps its label: {labels:?}"
    );

    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    assert_eq!(held.library.as_deref(), Some(snxlib.as_path()));
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held));
    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);

    assert!(f.app.library.library_at(&snxlib).is_some(), "mounted");
    assert!(
        !f.address
            .library_path
            .parent()
            .unwrap()
            .join("symbols")
            .exists(),
        "nothing was copied into the row's library"
    );
    let labels = headers(&f);
    assert!(
        labels.iter().all(|l| !l.contains("not open")),
        "now listed under the open library: {labels:?}"
    );
}

#[test]
fn a_truly_loose_symbol_has_no_library_and_still_offers_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    open_picker(&mut f);
    let loose = f.app.library.primitive_picker.as_ref().unwrap().loose[0].clone();
    assert_eq!(loose.library, None);
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(loose));
    picker(&mut f.app, PrimitivePickerMsg::CopyConfirmed);
    assert_eq!(bound_symbol(&f).uuid, f.symbol.uuid);
}

#[test]
fn a_shared_symbol_file_outside_the_project_is_not_claimed_by_a_stray_snxlib_above_it() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).unwrap();
    let mut f = fixture(&project);
    // A `.snxlib` the file is not inside, higher up than the project.
    std::fs::write(tmp.path().join("stray.snxlib"), "").unwrap();
    let shared = tmp.path().join("shared").join("sym").join("Shared.snxsym");
    std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
    std::fs::write(
        &shared,
        SymbolFile::from_symbol(Symbol::empty("SHARED"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    f.app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: shared,
            kind: LibraryEntryKind::Shared,
            library_id: None,
            unknown: Default::default(),
        });
    open_picker(&mut f);

    let labels = headers(&f);
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("Not in a library: Shared.snxsym")),
        "{labels:?}"
    );
    let picked = f.app.library.primitive_picker.as_ref().unwrap();
    assert!(picked.loose.iter().all(|s| s.library.is_none()));
}

#[test]
fn a_failed_open_of_the_symbols_library_shows_in_the_picker() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held));
    std::fs::remove_file(&snxlib).unwrap();
    picker(&mut f.app, PrimitivePickerMsg::OpenLibraryConfirmed);

    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    assert!(open.opening_library.is_none());
    assert!(
        open.error
            .as_deref()
            .is_some_and(|e| e.starts_with("Could not open library x")),
        "{:?}",
        open.error
    );
}

fn chosen_held(f: &mut Fixture) -> crate::library::loose_symbols::LooseSymbol {
    let held = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "HELD")
        .cloned()
        .unwrap();
    picker(&mut f.app, PrimitivePickerMsg::PickLoose(held.clone()));
    held
}

fn opening(f: &Fixture) -> Option<PathBuf> {
    f.app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .opening_library
        .clone()
}

#[test]
fn a_mount_cancelled_while_preparing_clears_the_opening_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());

    // The mount finishes after its request was withdrawn (library closed).
    let task = f
        .app
        .update(Message::Library(LibraryMessage::MountFinished {
            path: snxlib,
            prepared: crate::library::mount::PreparedMountCell::new(Err(
                signex_library::LibraryError::Backend("x".into()),
            )),
        }));
    settle(&mut f.app, task);

    assert_eq!(opening(&f), None);
    let open = f.app.library.primitive_picker.as_ref().unwrap();
    assert!(
        open.error
            .as_deref()
            .is_some_and(|e| e.contains("cancelled"))
    );
}

#[test]
fn cancel_while_opening_clears_the_opening_state() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib);
    picker(&mut f.app, PrimitivePickerMsg::CopyDeclined);
    assert_eq!(opening(&f), None);
}

#[test]
fn picking_another_symbol_while_opening_drops_the_opening_state_and_keeps_the_new_pick() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let snxlib = unmounted_library_symbol(&mut f, tmp.path());
    open_picker(&mut f);
    chosen_held(&mut f);
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());
    let loose = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .unwrap()
        .loose
        .iter()
        .find(|s| s.name == "GATEDRV8")
        .cloned()
        .unwrap();

    picker(&mut f.app, PrimitivePickerMsg::PickLoose(loose.clone()));
    assert_eq!(opening(&f), None);

    // The mount of the first library still lands: the new pick stays.
    f.app
        .library
        .primitive_picker
        .as_mut()
        .unwrap()
        .opening_library = Some(snxlib.clone());
    f.app.picker_mount_settled(&snxlib, None);
    let open = f.app.library.primitive_picker.as_ref().unwrap();
    assert_eq!(open.copy_confirm.as_ref(), Some(&loose));
}

#[test]
fn browsing_a_file_of_an_open_library_is_not_called_unopened() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    open_picker(&mut f);
    // `cells-lib/cells.snxlib` is mounted by the fixture.
    let held = tmp
        .path()
        .join("cells-lib")
        .join("symbols")
        .join("Held.snxsym");
    std::fs::create_dir_all(held.parent().unwrap()).unwrap();
    std::fs::write(
        &held,
        SymbolFile::from_symbol(Symbol::empty("HELD"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    let held_uuid = SymbolFile::from_bytes(&std::fs::read(&held).unwrap())
        .unwrap()
        .symbols[0]
        .uuid;
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(held)));
    // The pick bound the file's symbol to the row and closed the picker; no
    // "not open" question was raised, and no copy was made.
    assert!(
        f.app.library.primitive_picker.is_none(),
        "{:?}",
        f.app
            .library
            .primitive_picker
            .as_ref()
            .map(|p| &p.copy_confirm)
    );
    let bound = bound_symbol(&f);
    assert_eq!(bound.library_id, f.library_id);
    assert_eq!(bound.uuid, held_uuid);
}

/// A mounted library nested in another's folder, in a directory that also
/// holds an unmounted `.snxlib` (so the on-disk layout rule cannot name one).
#[test]
fn browsing_a_file_of_a_nested_open_library_binds_from_the_inner_one() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let nested = tmp.path().join("cells-lib").join("vendor");
    let snxlib = nested.join("v.snxlib");
    let id = Uuid::now_v7();
    {
        let manifest = SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: id,
            library: LibrarySection {
                name: "v".into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        };
        drop(
            LocalGitAdapter::init(
                &snxlib,
                manifest,
                LibraryInitOptions {
                    enable_git: false,
                    use_lfs: false,
                },
            )
            .expect("init"),
        );
        drop(LocalGitAdapter::recover_init(&snxlib).expect("recover_init"));
    }
    // A second `.snxlib` beside it: the on-disk layout rule names neither.
    std::fs::write(nested.join("other.snxlib"), "").unwrap();
    f.app.library.open_library(nested.join("v.snxlib")).unwrap();
    let held = nested.join("symbols").join("Held.snxsym");
    std::fs::create_dir_all(held.parent().unwrap()).unwrap();
    std::fs::write(
        &held,
        SymbolFile::from_symbol(Symbol::empty("HELD"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    open_picker(&mut f);
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(held)));
    assert!(f.app.library.primitive_picker.is_none());
    let bound = bound_symbol(&f);
    assert_eq!(bound.library_id, id);
    assert_ne!(bound.library_id, f.library_id);
}

fn init_library(snxlib: &Path) -> Uuid {
    let id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: id,
        library: LibrarySection {
            name: "inner".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            snxlib,
            manifest,
            LibraryInitOptions {
                enable_git: false,
                use_lfs: false,
            },
        )
        .expect("init"),
    );
    drop(LocalGitAdapter::recover_init(snxlib).expect("recover_init"));
    id
}

fn write_symbol(file: &Path) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        file,
        SymbolFile::from_symbol(Symbol::empty("STRAY"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
}

fn copy_question(f: &Fixture) -> Option<crate::library::loose_symbols::LooseSymbol> {
    f.app
        .library
        .primitive_picker
        .as_ref()
        .and_then(|p| p.copy_confirm.clone())
}

/// A symbol file under a mounted library's folder but not in its `symbols/`
/// is not held by it: it is offered as a copy, never bound dangling.
#[test]
fn browsing_a_stray_symbol_under_an_open_library_is_offered_as_a_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_symbol(&f);
    let stray = tmp
        .path()
        .join("cells-lib")
        .join("drafts")
        .join("Stray.snxsym");
    write_symbol(&stray);
    open_picker(&mut f);
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(stray)));
    let question = copy_question(&f).expect("copy offered");
    assert_eq!(question.library, None);
    assert_eq!(bound_symbol(&f), before);
}

/// The nearest library is NOT open: the outer open library does not hold the
/// file, so the question names the inner one instead of binding to the outer.
#[test]
fn browsing_a_symbol_of_an_unopened_nested_library_is_not_bound_to_the_outer_one() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_symbol(&f);
    let inner = tmp.path().join("cells-lib").join("vendor").join("v.snxlib");
    init_library(&inner);
    let held = inner.parent().unwrap().join("symbols").join("Held.snxsym");
    write_symbol(&held);
    open_picker(&mut f);
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(held)));
    let question = copy_question(&f).expect("question raised");
    assert_eq!(question.library, Some(inner));
    assert_eq!(bound_symbol(&f), before);
}

fn open_footprint_picker(f: &mut Fixture) {
    let task = f
        .app
        .update(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Footprint,
            target: PrimitivePickerTarget::BrowserRow(f.address.clone()),
        }));
    settle(&mut f.app, task);
}

fn bound_footprint(f: &Fixture) -> Option<PrimitiveRef> {
    f.app
        .library
        .set
        .get(f.library_id)
        .unwrap()
        .read_row(TABLE, f.address.row_id)
        .unwrap()
        .footprint_ref
}

#[test]
fn browsing_a_footprint_file_of_an_open_library_binds_it() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let wanted = Uuid::now_v7();
    let file = tmp
        .path()
        .join("cells-lib")
        .join("footprints")
        .join(format!("{wanted}.snxfpt"));
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "").unwrap();
    open_footprint_picker(&mut f);
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(file)));
    assert!(f.app.library.primitive_picker.is_none());
    assert_eq!(
        bound_footprint(&f),
        Some(PrimitiveRef::new(f.library_id, wanted))
    );
}

#[test]
fn browsing_a_footprint_file_outside_the_primitive_folders_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut f = fixture(tmp.path());
    let before = bound_footprint(&f);
    let file = tmp
        .path()
        .join("cells-lib")
        .join("drafts")
        .join(format!("{}.snxfpt", Uuid::now_v7()));
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "").unwrap();
    open_footprint_picker(&mut f);
    picker(&mut f.app, PrimitivePickerMsg::BrowseResult(Some(file)));
    let open = f.app.library.primitive_picker.as_ref().expect("still open");
    assert!(open.error.is_some());
    assert_eq!(bound_footprint(&f), before);
}
