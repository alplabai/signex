//! The core half of the load path on its own, through `DocumentState` calls
//! only (#2326 slice 3, PR 3.B). The wrappers and their projection are pinned
//! by `load_seam_tests.rs`, `tests/regression/load_seam.rs` and the open,
//! restore and reload tests.

use std::path::{Path, PathBuf};

use signex_engine::{Command, Engine, EngineError};
use signex_i18n::I18n;
use signex_types::schematic::{Junction, SchematicSheet};

use super::SheetInstall;
use crate::app::Signex;
use crate::app::core::event::{NoticeBody, NoticeLevel};
use crate::app::state::DocumentState;

fn app() -> Signex {
    Signex::new().0
}

fn engine_at(path: &Path) -> Engine {
    Engine::new_with_path(SchematicSheet::default(), Some(path.to_path_buf())).unwrap()
}

/// An engine at `path` with one edit past its save point.
fn edited_engine_at(path: &Path) -> Engine {
    let mut engine = engine_at(path);
    let junction = Junction {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    engine.execute(Command::PlaceJunction { junction }).unwrap();
    assert!(!engine.is_at_save_point(), "fixture: edited");
    engine
}

/// The one notice of `install`, as the Messages panel shows it.
fn only_line(install: &SheetInstall) -> String {
    assert_eq!(install.notices.len(), 1, "{:?}", install.notices);
    let notice = &install.notices[0];
    assert_eq!(notice.level, NoticeLevel::Warning);
    match &notice.body {
        NoticeBody::Text(text) => text.render(&I18n::en_us()),
        NoticeBody::Engine(other) => panic!("not a text notice: {other:?}"),
    }
}

fn live_doc_uuid(docs: &DocumentState, path: &Path) -> uuid::Uuid {
    docs.engines[path].document().uuid
}

#[test]
fn a_failed_active_install_keeps_the_engine_and_returns_one_notice() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/keep.snxsch");
    docs.engines.insert(path.clone(), edited_engine_at(&path));
    docs.active_path = Some(path.clone());
    let old = live_doc_uuid(docs, &path);

    let install = docs.install_active(path.clone(), Err(EngineError::MissingPath));

    assert!(!install.installed);
    assert_eq!(live_doc_uuid(docs, &path), old, "the open engine is kept");
    assert!(!docs.engines[&path].is_at_save_point(), "with its edit");
    assert_eq!(docs.active_path, Some(path));
    assert_eq!(
        only_line(&install),
        "/w/keep.snxsch could not be loaded into the editor (engine document path is not set); \
         the sheet already open was kept"
    );
}

#[test]
fn a_failed_install_at_drops_the_stale_engine_and_returns_one_notice() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/stale.snxsch");
    docs.engines.insert(path.clone(), edited_engine_at(&path));

    let install = docs.replace_engine_at(&path, Err(EngineError::MissingPath));

    assert!(!install.installed);
    assert!(!docs.engines.contains_key(&path), "a save cannot undo it");
    assert_eq!(
        only_line(&install),
        "[git] restore: /w/stale.snxsch was restored but its engine could not start: \
         engine document path is not set"
    );
}

#[test]
fn an_install_behind_a_tab_leaves_the_epoch_and_an_active_one_moves_it() {
    let mut app = app();
    let docs = &mut app.document_state;
    let (a, b) = (PathBuf::from("/w/a.snxsch"), PathBuf::from("/w/b.snxsch"));
    docs.engines.insert(b.clone(), edited_engine_at(&b));
    let before = docs.documents_epoch;

    let behind = docs.replace_engine_at(&b, Ok(engine_at(&b)));
    assert!(behind.installed && behind.notices.is_empty());
    assert!(!docs.engines[&b].can_undo(), "the new engine");
    assert_eq!(docs.documents_epoch, before, "behind a tab");
    assert_eq!(docs.active_path, None, "the active path is not touched");

    let active = docs.install_active(a.clone(), Ok(engine_at(&a)));
    assert!(active.installed && active.notices.is_empty());
    assert_eq!(docs.documents_epoch, before + 1, "active");
    assert_eq!(docs.active_path, Some(a.clone()));
    assert!(docs.engines.contains_key(&a));
}

#[test]
fn an_install_from_a_sheet_goes_to_the_active_tab_and_with_no_tab_drops_the_engine() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/tabbed.snxsch");
    docs.register_schematic_tab(path.clone(), "tabbed".into());

    let install = docs.sync_engine_from_schematic(Some(SchematicSheet::default()));
    assert!(install.installed && install.notices.is_empty());
    assert_eq!(docs.active_path, Some(path.clone()));
    assert_eq!(docs.engines[&path].path(), Some(path.as_path()));

    docs.tabs.clear();
    let none = docs.sync_engine_from_schematic(Some(SchematicSheet::default()));
    assert!(!none.installed && none.notices.is_empty());
    assert_eq!(docs.active_path, None);
    assert!(
        !docs.engines.contains_key(&path),
        "the active engine is dropped"
    );
}

#[test]
fn the_open_settle_marks_saved_holds_autosave_and_queues_recovery() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/opened.snxsch");
    docs.engines.insert(path.clone(), edited_engine_at(&path));

    docs.settle_opened_sheet(&path, true, false);

    assert!(docs.engines[&path].is_at_save_point(), "saved");
    assert!(!docs.is_document_dirty(&path));
    assert!(docs.instance_note_due);
    assert!(docs.autosave.checking_paths().any(|held| *held == path));
    assert_eq!(docs.recovery_check_due, vec![path]);
}

#[test]
fn a_failed_open_install_does_not_mark_the_old_engine_saved_but_still_holds_autosave() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/failed.snxsch");
    docs.engines.insert(path.clone(), edited_engine_at(&path));

    docs.settle_opened_sheet(&path, false, true);

    assert!(!docs.engines[&path].is_at_save_point());
    assert!(
        !docs.is_document_dirty(&path),
        "no repair mark on the old one"
    );
    assert!(docs.autosave.checking_paths().any(|held| *held == path));
    assert!(docs.recovery_check_due.contains(&path));
}

#[test]
fn a_repaired_open_is_dirty_and_an_unrepaired_one_clean() {
    let mut app = app();
    let docs = &mut app.document_state;
    let (fixed, plain) = (
        PathBuf::from("/w/fixed.snxsch"),
        PathBuf::from("/w/plain.snxsch"),
    );
    docs.engines.insert(fixed.clone(), engine_at(&fixed));
    docs.engines.insert(plain.clone(), engine_at(&plain));

    docs.settle_opened_sheet(&fixed, true, true);
    docs.settle_opened_sheet(&plain, true, false);

    assert!(docs.is_document_dirty(&fixed));
    assert!(!docs.is_document_dirty(&plain));
}

#[test]
fn a_reattached_parked_sheet_becomes_the_active_tab_with_its_engine_untouched() {
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/parked.snxsch");
    docs.engines.insert(path.clone(), edited_engine_at(&path));
    let before = docs.documents_epoch;

    docs.reattach_parked_sheet(path.clone(), "parked".into());

    assert_eq!(docs.active_tab, docs.tabs.len() - 1);
    assert_eq!(docs.tabs[docs.active_tab].path, path);
    assert_eq!(docs.tabs[docs.active_tab].title, "parked");
    assert_eq!(docs.active_path, Some(path.clone()));
    assert!(!docs.engines[&path].is_at_save_point(), "the edit survives");
    assert_eq!(docs.documents_epoch, before);
}

#[test]
fn activating_a_tab_points_at_its_engine_only_when_one_exists() {
    let mut app = app();
    let docs = &mut app.document_state;
    let (with, without) = (
        PathBuf::from("/w/with.snxsch"),
        PathBuf::from("/w/without.snxsch"),
    );
    docs.engines.insert(with.clone(), engine_at(&with));
    docs.register_schematic_tab(with.clone(), "with".into());
    assert!(docs.activate_tab_engine());
    assert_eq!(docs.active_path, Some(with.clone()));

    docs.register_schematic_tab(without, "without".into());
    assert!(!docs.activate_tab_engine());
    assert_eq!(docs.active_path, None);
    assert!(docs.engines.contains_key(&with), "no engine is dropped");

    docs.active_tab = 0;
    assert!(docs.activate_tab_engine());
    docs.detach_active_schematic();
    assert_eq!(docs.active_path, None);
    assert!(docs.engines.contains_key(&with), "detached, still resident");
}

#[test]
fn a_restored_copy_keeps_the_live_document_id() {
    use crate::autosave::RestoredDocument;
    let mut app = app();
    let docs = &mut app.document_state;
    let path = PathBuf::from("/w/recovered.snxsch");
    docs.engines.insert(path.clone(), engine_at(&path));
    let live = live_doc_uuid(docs, &path);
    let copy = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    let mut restored = RestoredDocument::Schematic(Box::new(copy));

    docs.keep_live_document_id(&path, &mut restored);

    let RestoredDocument::Schematic(sheet) = restored else {
        unreachable!("built as a schematic above");
    };
    assert_eq!(sheet.uuid, live);
}
