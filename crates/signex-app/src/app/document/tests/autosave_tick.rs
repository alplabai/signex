//! Guard for what one autosave tick writes: a recovery copy for each dirty
//! sheet (open or parked), PCB (a save in flight counts) and symbol or
//! footprint draft, and for nothing else. Moving the tick onto one
//! enumerator of documents (#661) must keep this list, copy for copy.

use std::path::{Path, PathBuf};

use super::{Kind, fixture, tab};
use crate::app::path_ops::settle_task;
use crate::app::{Message, Signex, TabDocument, TabKind};
use crate::autosave::{AutosaveMsg, sidecar};

struct Fx {
    app: Signex,
    dir: tempfile::TempDir,
    recovery: PathBuf,
    names: Vec<&'static str>,
    /// The dirty Component row's tab path, listed in `names` as `"row"`.
    row_path: PathBuf,
    /// The save job and the autosave flag that mark two documents busy.
    _holds: (Box<dyn std::any::Any>, Box<dyn std::any::Any>),
}

fn engine(app: &mut Signex, path: &Path, dirty: bool, with_tab: bool) {
    let mut e = signex_engine::Engine::new(Default::default()).unwrap();
    e.set_path(Some(path.to_path_buf()));
    app.document_state.insert_engine(path.to_path_buf(), e);
    if with_tab {
        app.document_state.tabs.push(tab(path, TabKind::Schematic));
    }
    if dirty {
        app.document_state.mark_document_dirty(path);
    }
}

fn pcb(app: &mut Signex, path: &Path, dirty: bool) {
    let mut t = tab(path, TabKind::Pcb);
    t.cached_document = Some(TabDocument::Pcb(super::save_routing::board()));
    app.document_state.tabs.push(t);
    if dirty {
        app.document_state.mark_document_dirty(path);
    }
}

fn draft(app: &mut Signex, kind: Kind, path: &Path, dirty: bool) {
    let docs = &mut app.document_state;
    if matches!(kind, Kind::Symbol) {
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        let mut editor = crate::app::SymbolDocument::new(path.to_path_buf(), file);
        if dirty {
            editor.mark_dirty();
        }
        docs.symbol_editors.insert(path.to_path_buf(), editor);
        docs.tabs
            .push(tab(path, TabKind::SymbolEditor(path.to_path_buf())));
    } else {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        let mut editor = crate::app::FootprintDocument::new(path.to_path_buf(), file);
        if dirty {
            editor.mark_dirty();
        }
        docs.footprint_editors.insert(path.to_path_buf(), editor);
        docs.tabs
            .push(tab(path, TabKind::FootprintEditor(path.to_path_buf())));
    }
}

/// One app holding every kind of document in every state the tick tells
/// apart; autosave is on and the recovery directory is set.
fn setup() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let recovery = dir.path().join("recovery");
    let (mut app, _) = fixture(dir.path(), Kind::Project, false);
    let p = |name: &str| dir.path().join(name);
    engine(&mut app, &p("tab.snxsch"), true, true);
    engine(&mut app, &p("parked.snxsch"), true, false);
    engine(&mut app, &p("clean.snxsch"), false, true);
    engine(&mut app, &p("paused.snxsch"), true, true);
    engine(&mut app, &p("inflight.snxsch"), true, true);
    pcb(&mut app, &p("dirty.snxpcb"), true);
    pcb(&mut app, &p("clean.snxpcb"), false);
    pcb(&mut app, &p("saving.snxpcb"), false);
    let save = app
        .document_state
        .document_saves
        .begin(p("saving.snxpcb"), Vec::new());
    draft(&mut app, Kind::Symbol, &p("dirty.snxsym"), true);
    draft(&mut app, Kind::Symbol, &p("clean.snxsym"), false);
    draft(&mut app, Kind::Footprint, &p("dirty.snxfpt"), true);
    draft(&mut app, Kind::Footprint, &p("clean.snxfpt"), false);
    // The project is dirty, and so is a Component row.
    let project = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&project);
    let (row_app, row_tab) = fixture(dir.path(), Kind::Row, true);
    app.library.editors = row_app.library.editors;
    let row_path = app
        .library
        .editors
        .keys()
        .next()
        .unwrap()
        .synthetic_tab_path();
    app.document_state.tabs.push(row_tab);

    app.document_state.autosave.set_dir(Some(recovery.clone()));
    app.ui_state.autosave_enabled = true;
    app.document_state.autosave.defer(&p("paused.snxsch"));
    let in_flight = app.document_state.autosave.begin(&p("inflight.snxsch"));
    let names = vec![
        "tab.snxsch",
        "parked.snxsch",
        "clean.snxsch",
        "paused.snxsch",
        "inflight.snxsch",
        "dirty.snxpcb",
        "clean.snxpcb",
        "saving.snxpcb",
        "dirty.snxsym",
        "clean.snxsym",
        "dirty.snxfpt",
        "clean.snxfpt",
        "p.snxprj",
        "row",
    ];
    Fx {
        app,
        dir,
        recovery,
        names,
        row_path,
        _holds: (Box::new(save), Box::new(in_flight)),
    }
}

impl Fx {
    fn path_of(&self, name: &str) -> PathBuf {
        match name {
            "row" => self.row_path.clone(),
            _ => self.dir.path().join(name),
        }
    }
}

/// The documents that have a recovery copy after one tick.
fn copies_after_tick(f: &mut Fx) -> Vec<&'static str> {
    let task = f.app.update(Message::Autosave(AutosaveMsg::Ticked));
    settle_task(&mut f.app, task);
    f.names
        .iter()
        .copied()
        .filter(|n| sidecar::sidecar_path(&f.recovery, &f.path_of(n)).exists())
        .collect()
}

#[test]
fn a_tick_writes_a_copy_for_each_dirty_document_and_nothing_else() {
    let mut f = setup();
    assert_eq!(
        copies_after_tick(&mut f),
        [
            "tab.snxsch",
            "parked.snxsch",
            "dirty.snxpcb",
            "saving.snxpcb",
            "dirty.snxsym",
            "dirty.snxfpt",
            "p.snxprj",
            "row",
        ]
    );
}

#[test]
fn a_tick_with_autosave_off_writes_nothing() {
    let mut f = setup();
    f.app.ui_state.autosave_enabled = false;
    assert_eq!(copies_after_tick(&mut f), Vec::<&str>::new());
}

#[test]
fn a_second_tick_rewrites_nothing_and_a_save_clears_the_copy() {
    let mut f = setup();
    let first = copies_after_tick(&mut f);
    assert_eq!(copies_after_tick(&mut f), first);
    f.app
        .document_state
        .clear_document_dirty(&f.dir.path().join("tab.snxsch"));
    let after = copies_after_tick(&mut f);
    assert!(!after.contains(&"tab.snxsch"), "{after:?}");
}

#[test]
fn a_path_holds_recoverable_edits_only_while_its_document_is_dirty() {
    let f = setup();
    let holds: Vec<&str> = f
        .names
        .iter()
        .copied()
        .chain(["unknown.snxsch"])
        .filter(|n| f.app.holds_recoverable_edits(&f.path_of(n)))
        .collect();
    assert_eq!(
        holds,
        [
            "tab.snxsch",
            "parked.snxsch",
            "paused.snxsch",
            "inflight.snxsch",
            "dirty.snxpcb",
            "saving.snxpcb",
            "dirty.snxsym",
            "dirty.snxfpt",
            "p.snxprj",
            "row",
        ]
    );
}

/// The sidecar sweep keeps the copy of every document that still holds
/// edits, a project and a Component row included, and removes it once the
/// document is clean.
#[test]
fn the_sweep_keeps_a_dirty_projects_and_rows_copy_and_removes_it_when_clean() {
    let mut f = setup();
    let first = copies_after_tick(&mut f);
    assert!(
        first.contains(&"p.snxprj") && first.contains(&"row"),
        "{first:?}"
    );
    let project = f.path_of("p.snxprj");
    f.app.document_state.clear_document_dirty(&project);
    for editor in f.app.library.editors.values_mut() {
        editor.dirty = false;
    }
    let after = copies_after_tick(&mut f);
    assert!(
        !after.contains(&"p.snxprj") && !after.contains(&"row"),
        "{after:?}"
    );
    assert!(after.contains(&"tab.snxsch"), "{after:?}");
}
