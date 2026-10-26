//! #1185 — autosave and recovery for the symbol and footprint editors.
//!
//! Real files, the real sidecar writer and the real recovery prompt: a draft
//! autosaved by one run is offered, restored or discarded by the next.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::autosave::{AutosaveMsg, RecoveryChoice, sidecar};
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit};

use super::complete;

#[derive(Clone, Copy)]
enum Kind {
    Symbol,
    Footprint,
}

impl Kind {
    fn file_name(self) -> &'static str {
        match self {
            Kind::Symbol => "Part.snxsym",
            Kind::Footprint => "Part.snxfpt",
        }
    }

    fn write_original(self, path: &Path) {
        let text = match self {
            Kind::Symbol => {
                signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Original"))
                    .to_toml_string()
                    .unwrap()
            }
            Kind::Footprint => signex_library::FootprintFile::from_footprint(
                signex_library::Footprint::empty("Original"),
            )
            .to_toml_string()
            .unwrap(),
        };
        std::fs::write(path, text).unwrap();
    }
}

struct Fixture {
    app: Signex,
    kind: Kind,
    doc: PathBuf,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn open(app: &mut Signex, doc: &Path) {
    let task = app.update(Message::File(FileMsg::Opened(Some(doc.to_path_buf()))));
    complete(app, task);
}

fn fresh_run(recovery: &Path, doc: &Path) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.document_state
        .autosave
        .set_dir(Some(recovery.to_path_buf()));
    app.ui_state.autosave_enabled = true;
    open(&mut app, doc);
    app
}

fn name_in_editor(app: &Signex, kind: Kind, doc: &Path) -> String {
    match kind {
        Kind::Symbol => app.document_state.symbol_editors[doc]
            .primitive()
            .name
            .clone(),
        Kind::Footprint => app.document_state.footprint_editors[doc]
            .primitive()
            .name
            .clone(),
    }
}

fn is_dirty(app: &Signex, kind: Kind, doc: &Path) -> bool {
    match kind {
        Kind::Symbol => app.document_state.symbol_editors[doc].dirty,
        Kind::Footprint => app.document_state.footprint_editors[doc].dirty,
    }
}

/// Rename the open primitive and mark the editor dirty, as an edit would.
fn edit(app: &mut Signex, kind: Kind, doc: &Path, name: &str) {
    match kind {
        Kind::Symbol => {
            let editor = app.document_state.symbol_editors.get_mut(doc).unwrap();
            editor.primitive_mut().name = name.to_string();
            editor.dirty = true;
        }
        Kind::Footprint => {
            let editor = app.document_state.footprint_editors.get_mut(doc).unwrap();
            editor.primitive_mut().name = name.to_string();
            editor.dirty = true;
        }
    }
}

fn tick(app: &mut Signex) {
    let task = app.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(app, task);
}

fn answer(app: &mut Signex, choice: RecoveryChoice) {
    let task = app.update(Message::Autosave(AutosaveMsg::Recovery(choice)));
    complete(app, task);
}

fn fixture(kind: Kind) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join(kind.file_name());
    let recovery = tmp.path().join("recovery");
    kind.write_original(&doc);
    let app = fresh_run(&recovery, &doc);
    Fixture {
        app,
        kind,
        doc,
        recovery,
        _tmp: tmp,
    }
}

fn sidecar_of(f: &Fixture) -> PathBuf {
    sidecar::sidecar_path(&f.recovery, &f.doc)
}

/// The first run edited and autosaved; a second run then opens the file.
fn next_run(kind: Kind) -> (Fixture, Signex, Vec<u8>) {
    let mut f = fixture(kind);
    edit(&mut f.app, kind, &f.doc, "Edited");
    tick(&mut f.app);
    assert!(sidecar_of(&f).exists(), "the tick wrote a recovery copy");
    let on_disk = std::fs::read(&f.doc).unwrap();
    let next = fresh_run(&f.recovery, &f.doc);
    (f, next, on_disk)
}

fn each_kind(check: impl Fn(Kind)) {
    check(Kind::Symbol);
    check(Kind::Footprint);
}

#[test]
fn a_dirty_editor_gets_a_sidecar_and_the_real_file_is_untouched() {
    each_kind(|kind| {
        let mut f = fixture(kind);
        let before = std::fs::read(&f.doc).unwrap();
        edit(&mut f.app, kind, &f.doc, "Edited");
        tick(&mut f.app);
        assert!(sidecar_of(&f).exists());
        assert_eq!(std::fs::read(&f.doc).unwrap(), before);
        assert!(is_dirty(&f.app, f.kind, &f.doc), "a copy is not a save");
    });
}

#[test]
fn a_clean_editor_gets_no_sidecar() {
    each_kind(|kind| {
        let mut f = fixture(kind);
        tick(&mut f.app);
        assert!(!sidecar_of(&f).exists());
    });
}

#[test]
fn autosave_off_writes_no_editor_sidecar() {
    each_kind(|kind| {
        let mut f = fixture(kind);
        f.app.ui_state.autosave_enabled = false;
        edit(&mut f.app, kind, &f.doc, "Edited");
        tick(&mut f.app);
        assert!(!sidecar_of(&f).exists());
    });
}

#[test]
fn saving_the_editor_removes_the_sidecar() {
    each_kind(|kind| {
        let mut f = fixture(kind);
        edit(&mut f.app, kind, &f.doc, "Edited");
        tick(&mut f.app);
        assert!(sidecar_of(&f).exists());

        let task = f
            .app
            .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: f.doc.clone(),
                msg: PrimitiveEdit::Save,
            }));
        complete(&mut f.app, task);

        assert!(!is_dirty(&f.app, kind, &f.doc));
        assert!(!sidecar_of(&f).exists(), "a save drops the recovery copy");
    });
}

#[test]
fn opening_a_file_with_an_earlier_copy_offers_it() {
    each_kind(|kind| {
        let (f, next, _) = next_run(kind);
        let offer = next.document_state.autosave.current_offer().expect("offer");
        assert_eq!(offer.path, f.doc);
        assert!(sidecar_of(&f).exists(), "offering deletes nothing");
    });
}

#[test]
fn restore_loads_the_copy_as_unsaved_edits_and_never_writes_the_file() {
    each_kind(|kind| {
        let (f, mut next, on_disk) = next_run(kind);
        assert_eq!(name_in_editor(&next, kind, &f.doc), "Original");
        answer(&mut next, RecoveryChoice::Restore);

        assert_eq!(name_in_editor(&next, kind, &f.doc), "Edited");
        assert!(is_dirty(&next, kind, &f.doc), "restored content is unsaved");
        assert!(next.document_state.dirty_paths.contains(&f.doc));
        assert_eq!(std::fs::read(&f.doc).unwrap(), on_disk);
        assert!(sidecar_of(&f).exists(), "the copy stays until a save");

        let task = next.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: f.doc.clone(),
            msg: PrimitiveEdit::Save,
        }));
        complete(&mut next, task);
        assert!(!sidecar_of(&f).exists());
        assert_ne!(std::fs::read(&f.doc).unwrap(), on_disk);
    });
}

#[test]
fn discard_deletes_the_copy_and_leaves_the_editor_clean() {
    each_kind(|kind| {
        let (f, mut next, on_disk) = next_run(kind);
        answer(&mut next, RecoveryChoice::Discard);
        assert!(!sidecar_of(&f).exists());
        assert!(!is_dirty(&next, kind, &f.doc));
        assert_eq!(name_in_editor(&next, kind, &f.doc), "Original");
        assert_eq!(std::fs::read(&f.doc).unwrap(), on_disk);
    });
}

#[test]
fn decide_later_keeps_the_copy_and_autosave_does_not_overwrite_it() {
    each_kind(|kind| {
        let (f, mut next, _) = next_run(kind);
        let copy = std::fs::read(sidecar_of(&f)).unwrap();
        answer(&mut next, RecoveryChoice::Later);
        edit(&mut next, kind, &f.doc, "Edited again");
        tick(&mut next);
        assert_eq!(std::fs::read(sidecar_of(&f)).unwrap(), copy);
    });
}

#[test]
fn a_copy_identical_to_the_file_is_removed_at_open_and_never_offered() {
    each_kind(|kind| {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join(kind.file_name());
        let recovery = tmp.path().join("recovery");
        kind.write_original(&doc);
        sidecar::write(&recovery, &doc, &std::fs::read(&doc).unwrap()).unwrap();

        let app = fresh_run(&recovery, &doc);
        assert!(app.document_state.autosave.current_offer().is_none());
        assert!(!sidecar::sidecar_path(&recovery, &doc).exists());
    });
}

#[test]
fn a_copy_that_is_not_a_readable_primitive_is_refused_and_kept() {
    each_kind(|kind| {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join(kind.file_name());
        let recovery = tmp.path().join("recovery");
        kind.write_original(&doc);
        sidecar::write(&recovery, &doc, b"not a primitive").unwrap();

        let mut app = fresh_run(&recovery, &doc);
        assert!(app.document_state.autosave.current_offer().is_some());
        answer(&mut app, RecoveryChoice::Restore);
        assert!(app.document_state.error_notice.is_some());
        assert!(sidecar::sidecar_path(&recovery, &doc).exists());
        assert_eq!(name_in_editor(&app, kind, &doc), "Original");
    });
}

#[test]
fn a_tick_during_an_in_flight_save_that_then_fails_keeps_the_sidecar() {
    each_kind(|kind| {
        let mut f = fixture(kind);
        edit(&mut f.app, kind, &f.doc, "Edited");
        tick(&mut f.app);
        assert!(sidecar_of(&f).exists());

        // The save starts: the editor reads clean, but nothing is on disk.
        let save = f
            .app
            .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: f.doc.clone(),
                msg: PrimitiveEdit::Save,
            }));
        assert!(!is_dirty(&f.app, kind, &f.doc));

        // A tick lands before the write completes.
        tick(&mut f.app);
        assert!(sidecar_of(&f).exists(), "a save in flight is not a save");

        // The write fails: a directory sits where the file belongs.
        std::fs::remove_file(&f.doc).unwrap();
        std::fs::create_dir(&f.doc).unwrap();
        for message in super::run(save) {
            let task = f.app.update(message);
            complete(&mut f.app, task);
        }

        assert!(is_dirty(&f.app, kind, &f.doc), "the failed draft is dirty");
        tick(&mut f.app);
        assert!(sidecar_of(&f).exists(), "the recovery copy survives");
    });
}
