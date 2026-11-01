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
