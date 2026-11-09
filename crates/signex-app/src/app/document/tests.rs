//! Characterization of the dirty query: per document kind, what the tab
//! dot, the quit guard and the Close Project scan report for a clean and a
//! dirty document, plus `DocumentKey::of`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Document, DocumentKey};
use crate::app::state::LoadedProject;
use crate::app::{ComponentEditorTab, Signex, TabInfo, TabKind};
use crate::library::state::{ComponentPreviewState, EditorAddress};

mod autosave_tick;
mod close_readers;
mod dirty_readers;
mod save_routing;

#[derive(Debug, Clone, Copy)]
enum Kind {
    Schematic,
    Pcb,
    Symbol,
    Footprint,
    Row,
    HalfTypedRow,
    Project,
}

const ALL: [Kind; 7] = [
    Kind::Schematic,
    Kind::Pcb,
    Kind::Symbol,
    Kind::Footprint,
    Kind::Row,
    Kind::HalfTypedRow,
    Kind::Project,
];

fn project_at(app: &mut Signex, dir: &Path) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("p.snxprj"),
        data: signex_types::project::ProjectData {
            document_id: None,
            description: None,
            name: "p".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
}

fn tab(path: &Path, kind: TabKind) -> TabInfo {
    TabInfo {
        title: "t".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    }
}

/// An app with one open project at `dir` and one document of `kind`, dirty
/// or clean, with the tab that shows it.
fn fixture(dir: &Path, kind: Kind, dirty: bool) -> (Signex, TabInfo) {
    let (mut app, _task) = Signex::new();
    project_at(&mut app, dir);
    let project_id = app.document_state.projects[0].id;
    let docs = &mut app.document_state;
    let t = match kind {
        Kind::Schematic => {
            let path = dir.join("a.snxsch");
            docs.insert_engine(
                path.clone(),
                signex_engine::Engine::new(Default::default()).unwrap(),
            );
            if dirty {
                docs.mark_document_dirty(&path);
            }
            tab(&path, TabKind::Schematic)
        }
        Kind::Pcb => {
            let path = dir.join("b.snxpcb");
            if dirty {
                docs.mark_document_dirty(&path);
            }
            tab(&path, TabKind::Pcb)
        }
        Kind::Symbol => {
            let path = dir.join("lib.snxlib").join("s.snxsym");
            let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
            let mut editor = crate::app::SymbolDocument::new(path.clone(), file);
            if dirty {
                editor.mark_dirty();
            }
            docs.symbol_editors.insert(path.clone(), editor);
            tab(&path, TabKind::SymbolEditor(path.clone()))
        }
        Kind::Footprint => {
            let path = dir.join("lib.snxlib").join("f.snxfpt");
            let file = signex_library::FootprintFile::from_footprint(
                signex_library::Footprint::empty("f"),
            );
            let mut editor = crate::app::FootprintDocument::new(path.clone(), file);
            if dirty {
                editor.mark_dirty();
            }
            docs.footprint_editors.insert(path.clone(), editor);
            tab(&path, TabKind::FootprintEditor(path.clone()))
        }
        Kind::Row | Kind::HalfTypedRow => {
            let lib = dir.join("lib.snxlib");
            let mut editor = ComponentPreviewState::from_row(
                lib.clone(),
                "parts".into(),
                crate::library::component_preview::test_row(),
            );
            let row_id = signex_library::RowId(editor.row.row_id);
            match (kind, dirty) {
                (Kind::HalfTypedRow, true) => {
                    editor.params_edit_buf.insert("v".into(), "1.".into());
                }
                (_, true) => editor.mark_dirty(),
                _ => {}
            }
            let address = EditorAddress::new(lib.clone(), "parts".into(), row_id);
            app.library.editors.insert(address, editor);
            tab(
                &lib.join("parts.tsv#row"),
                TabKind::ComponentEditor(ComponentEditorTab {
                    library_path: lib,
                    table: "parts".into(),
                    row_id,
                }),
            )
        }
        Kind::Project => {
            let path = docs.projects[0].path.clone();
            if dirty {
                docs.mark_document_dirty(&path);
            }
            tab(&path, TabKind::Project(project_id))
        }
    };
    (app, t)
}

fn dot(app: &Signex, t: &TabInfo) -> bool {
    crate::tab_bar::dirty_tab_paths(app, std::slice::from_ref(t)).contains(&t.path)
}

fn quit_prompts(app: &mut Signex) -> bool {
    let _ = app.handle_app_quit_requested();
    app.ui_state.app_quit_confirm.is_some()
}

fn close_scan_lists_something(app: &Signex) -> bool {
    let docs = &app.document_state;
    let refs = docs.sheet_ref_inputs().resolve(|_| true);
    let scan = docs.close_scan(&refs, 0, &app.library.dirty_editors(), &[], &HashMap::new());
    !(scan.mine.is_empty()
        && scan.kept.is_empty()
        && scan.mine_rows.is_empty()
        && scan.kept_rows.is_empty())
}

#[test]
fn a_clean_document_reads_clean_on_the_dot_the_quit_guard_and_close_project() {
    for kind in ALL {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, t) = fixture(dir.path(), kind, false);
        assert!(!dot(&app, &t), "{kind:?} dot");
        assert!(!close_scan_lists_something(&app), "{kind:?} close scan");
        assert!(!quit_prompts(&mut app), "{kind:?} quit guard");
    }
}

#[test]
fn a_dirty_document_reads_dirty_on_the_dot_the_quit_guard_and_close_project() {
    for kind in ALL {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, t) = fixture(dir.path(), kind, true);
        // A half-typed value counts at every gate, as on the dot (#2132).
        assert!(dot(&app, &t), "{kind:?} dot");
        assert!(close_scan_lists_something(&app), "{kind:?} close scan");
        assert!(quit_prompts(&mut app), "{kind:?} quit guard");
    }
}

#[test]
fn the_trait_answers_match_the_owner_for_every_kind() {
    for kind in ALL {
        let dir = tempfile::tempdir().unwrap();
        let (app, t) = fixture(dir.path(), kind, true);
        let key = DocumentKey::of(&t).expect("a document tab has a key");
        let doc = app.document(&key).unwrap();
        assert!(doc.has_unsaved_input(), "{kind:?}");
        assert_eq!(
            doc.is_dirty(),
            !matches!(kind, Kind::HalfTypedRow),
            "{kind:?}"
        );
    }
}

#[test]
fn the_key_names_the_document_a_tab_hosts() {
    let p = PathBuf::from("x");
    let kinds = [
        (TabKind::Schematic, Some(DocumentKey::Schematic(p.clone()))),
        (TabKind::Pcb, Some(DocumentKey::Pcb(p.clone()))),
        (
            TabKind::SymbolEditor(p.clone()),
            Some(DocumentKey::Symbol(p.clone())),
        ),
        (
            TabKind::FootprintEditor(p.clone()),
            Some(DocumentKey::Footprint(p.clone())),
        ),
        (TabKind::LibraryBrowser(p.clone()), None),
    ];
    for (kind, want) in kinds {
        assert_eq!(DocumentKey::of(&tab(&p, kind)), want);
    }
    let dir = tempfile::tempdir().unwrap();
    let (_, row) = fixture(dir.path(), Kind::Row, false);
    assert!(matches!(DocumentKey::of(&row), Some(DocumentKey::Row(_))));
    let (_, project) = fixture(dir.path(), Kind::Project, false);
    assert!(matches!(
        DocumentKey::of(&project),
        Some(DocumentKey::Project(_))
    ));
}

/// A symbol or footprint draft whose snapshot cleared its flag while the
/// write has not landed: only the stored set holds it dirty.
fn save_in_flight(kind: Kind) -> (Signex, TabInfo, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, t) = fixture(dir.path(), kind, false);
    app.document_state.keep_document_dirty_until_saved(&t.path);
    (app, t, dir)
}

#[test]
fn a_draft_with_its_save_in_flight_reads_dirty_everywhere() {
    for kind in [Kind::Symbol, Kind::Footprint] {
        let (mut app, t, _dir) = save_in_flight(kind);
        assert!(dot(&app, &t), "{kind:?} dot");
        assert!(close_scan_lists_something(&app), "{kind:?} close scan");
        let key = DocumentKey::of(&t).unwrap();
        assert!(app.document(&key).unwrap().is_dirty(), "{kind:?} trait");
        assert!(quit_prompts(&mut app), "{kind:?} quit guard");
    }
}

#[test]
fn a_tab_without_a_document_owner_keeps_the_path_set_dot() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = fixture(dir.path(), Kind::Project, false);
    let lib = dir.path().join("lib.snxlib");
    let browser = tab(&lib, TabKind::LibraryBrowser(lib.clone()));
    assert!(!dot(&app, &browser));
    app.document_state.mark_document_dirty(&lib);
    assert!(dot(&app, &browser), "library browser");

    let (mut app, project) = fixture(dir.path(), Kind::Project, true);
    assert!(dot(&app, &project));
    app.document_state.projects.clear();
    assert!(dot(&app, &project), "project tab after its project closed");
}

/// #2149 — a board parked without a tab is listed as `kept` by the Close
/// Project scan (it asks, but does not own the board) and prompts at quit.
#[test]
fn a_parked_board_is_kept_by_the_close_scan_and_prompts_at_quit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, t) = fixture(dir.path(), Kind::Pcb, true);
    app.document_state.tabs.clear();
    app.document_state.park_pcb_board(
        t.path.clone(),
        signex_engine::pcb::BoardEngine::new(
            crate::app::handlers::document_files::save_async_tests::empty_board(),
            1,
        ),
    );
    let refs = app.document_state.sheet_ref_inputs().resolve(|_| true);
    let scan =
        app.document_state
            .close_scan(&refs, 0, &app.library.dirty_editors(), &[], &HashMap::new());
    assert_eq!(scan.kept, vec![t.path.clone()]);
    assert!(scan.mine.is_empty());
    assert!(quit_prompts(&mut app));
}
