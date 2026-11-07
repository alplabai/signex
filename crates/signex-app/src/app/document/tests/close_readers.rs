//! Guards for the Close Project scan, the Close Library list and Save All's
//! per-path routing. They pin today's answers per kind so moving the readers
//! onto the Document model (#661) is provably behaviour-preserving.

use std::path::{Path, PathBuf};

use super::{Kind, fixture, save_in_flight};
use crate::app::Signex;
use crate::app::contracts::{SaveAllNext, SaveAllPlan};

/// One app holding a dirty sheet, symbol and footprint draft, row, project
/// file and library manifest, all under the project directory.
fn everything(dir: &Path) -> Signex {
    let (mut app, _) = fixture(dir, Kind::Schematic, true);
    for kind in [Kind::Symbol, Kind::Footprint, Kind::Row] {
        let (other, _) = fixture(dir, kind, true);
        app.document_state
            .symbol_editors
            .extend(other.document_state.symbol_editors);
        app.document_state
            .footprint_editors
            .extend(other.document_state.footprint_editors);
        app.library.editors.extend(other.library.editors);
    }
    own_sheet_and_library(&mut app.document_state.projects[0]);
    let project_file = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&project_file);
    app.document_state
        .mark_document_dirty(&dir.join("lib.snxlib"));
    app
}

/// The project lists `a.snxsch` as its root sheet and `lib.snxlib` as its own.
fn own_sheet_and_library(project: &mut crate::app::state::LoadedProject) {
    use signex_types::project::{LibraryEntry, LibraryEntryKind, SheetEntry};
    project.data.schematic_root = Some("a.snxsch".into());
    project.data.sheets = vec![SheetEntry {
        name: "a".into(),
        filename: "a.snxsch".into(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }];
    project.data.libraries = vec![LibraryEntry {
        path: PathBuf::from("lib.snxlib"),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
        unknown: Default::default(),
    }];
}

/// A second open project that lists the same sheet and library.
fn with_second_project(app: &mut Signex, dir: &Path) {
    super::project_at(app, dir);
    let second = app.document_state.projects.last_mut().unwrap();
    second.path = dir.join("q.snxprj");
    own_sheet_and_library(second);
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

fn scan_of(app: &Signex) -> (Vec<String>, Vec<String>, usize, usize, Option<String>) {
    let refs = app.document_state.sheet_ref_inputs().resolve(|_| true);
    let scan = app.current_close_scan(0, &refs);
    (
        names(&scan.mine),
        names(&scan.kept),
        scan.mine_rows.len(),
        scan.kept_rows.len(),
        scan.warning("p"),
    )
}

#[test]
fn close_scan_places_every_kind() {
    let dir = tempfile::tempdir().unwrap();
    let app = everything(dir.path());
    let (mine, kept, mine_rows, kept_rows, warning) = scan_of(&app);
    assert_eq!(mine, ["a.snxsch", "f.snxfpt", "s.snxsym", "p.snxprj"]);
    assert_eq!(kept, ["lib.snxlib"]);
    assert_eq!((mine_rows, kept_rows), (1, 0));
    assert_eq!(
        warning.as_deref(),
        Some(
            "Close Project 'p': kept 1 unsaved document(s) open, not provably this \
             project's alone: lib.snxlib"
        )
    );
}

#[test]
fn close_scan_of_a_sheet_and_library_two_projects_share() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = everything(dir.path());
    with_second_project(&mut app, dir.path());
    let (mine, kept, mine_rows, kept_rows, _) = scan_of(&app);
    assert_eq!(mine, ["p.snxprj"]);
    assert_eq!(kept, ["a.snxsch", "lib.snxlib", "f.snxfpt", "s.snxsym"]);
    assert_eq!((mine_rows, kept_rows), (0, 1));
}

#[test]
fn close_scan_of_a_half_typed_row_and_a_save_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    // A half-typed row is unsaved work (#2132), so the scan sees it; its
    // library is not provably this project's alone, so it is kept open.
    let (app, _) = fixture(dir.path(), Kind::HalfTypedRow, true);
    assert_eq!(
        scan_of(&app),
        (
            vec![],
            vec![],
            0,
            1,
            Some(
                "Close Project 'p': kept 1 unsaved document(s) open, not provably this                  project's alone: lib.snxlib"
                    .to_string()
            )
        )
    );
    for kind in [Kind::Symbol, Kind::Footprint] {
        let (app, t, _dir) = save_in_flight(kind);
        let (mine, kept, ..) = scan_of(&app);
        let name = t.path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!((mine.len(), kept), (0, vec![name]), "{kind:?} in flight");
    }
}

#[test]
fn library_prompt_lists_the_drafts_of_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let app = everything(dir.path());
    let lib = dir.path().join("lib.snxlib");
    let other = dir.path().join("other.snxlib");
    assert_eq!(
        names(&app.dirty_primitive_paths_of_library(&lib)),
        ["f.snxfpt", "s.snxsym"]
    );
    assert_eq!(
        names(&app.dirty_primitive_paths_of_library(&other)),
        ["f.snxfpt", "s.snxsym"],
        "libraries of one folder all list its drafts"
    );
    assert_eq!(app.dirty_draft_folds().len(), 2, "one fold per dirty draft");
}

/// What Save All records for `path` alone: `None` when a write started.
fn save_all_failure(app: &mut Signex, path: &Path) -> Option<String> {
    let _ = app.begin_save_all_planned(SaveAllPlan {
        context: "t".into(),
        paths: vec![path.to_path_buf()],
        rows: Vec::new(),
        next: SaveAllNext::Nothing,
        probes: Default::default(),
        undecided: Vec::new(),
    });
    app.document_state
        .document_saves
        .failure(path)
        .map(str::to_owned)
}

const NO_BACKING: &str =
    "no open editor, engine, or project backs this file — it can't be saved automatically";

#[test]
fn save_all_routes_each_kind_by_the_map_that_holds_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = everything(dir.path());
    let d = dir.path();
    let table: Vec<(&str, PathBuf, Option<&str>)> = vec![
        ("sheet", d.join("a.snxsch"), None),
        ("symbol", d.join("lib.snxlib").join("s.snxsym"), None),
        ("footprint", d.join("lib.snxlib").join("f.snxfpt"), None),
        ("project", d.join("p.snxprj"), None),
        ("manifest", d.join("lib.snxlib"), Some(NO_BACKING)),
        ("pcb", d.join("b.snxpcb"), Some(NO_BACKING)),
    ];
    for (name, path, want) in table {
        app.document_state.mark_document_dirty(&path);
        let got = save_all_failure(&mut app, &path);
        assert_eq!(got.as_deref(), want, "{name}");
    }
}
