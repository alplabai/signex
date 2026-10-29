//! #1149 — one document per file, whatever spelling the path arrives in.
//!
//! A path under a case-insensitive project folds `Lib` and `lib` into one
//! file. The routes that need the file on disk use a `./` spelling instead
//! (`dir/./Lib.snxsch`), which is a second spelling of the same file on every
//! host, so the suite does not depend on the CI filesystem's case rule.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{Message, Signex};

fn sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    }
}

/// A folding project at `/w/proj` with `Lib.snxsch` open in one tab.
fn app_holding_lib() -> (Signex, PathBuf) {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    let held = PathBuf::from("/w/proj/Lib.snxsch");
    app.open_schematic_tab(held.clone(), "Lib".to_string(), sheet());
    focus_another_tab(&mut app);
    (app, held)
}

/// A folding project in a real directory holding `Lib.snxsch`, open in a tab.
fn app_holding_real_lib() -> (Signex, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-1149-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(dir.join("sub")).expect("tempdir");
    let file = dir.join("Lib.snxsch");
    let text = signex_types::format::SnxSchematic::new(sheet())
        .write_string()
        .expect("serialize");
    std::fs::write(&file, text).expect("write sheet");
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    app.open_schematic_tab(file.clone(), "Lib".to_string(), sheet());
    focus_another_tab(&mut app);
    (app, dir, file)
}

/// `dir/sub/../Lib.snxsch`: `Path` equality does not collapse `..`, and the
/// file is reachable under it on every host.
fn dotted(dir: &Path) -> PathBuf {
    dir.join("sub").join("..").join("Lib.snxsch")
}

/// Focus moves to another tab, so a reuse shows up as the focus coming back.
fn focus_another_tab(app: &mut Signex) {
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "Other".to_string(),
        path: PathBuf::from("/w/proj/Other.snxsch"),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

fn assert_one_tab_for(app: &Signex, held: &Path) {
    assert_eq!(app.document_state.tabs.len(), 2, "a second tab was opened");
    let active = &app.document_state.tabs[app.document_state.active_tab];
    assert_eq!(active.path, held, "the held tab was not activated");
    assert!(
        app.document_state.pending_opens.is_empty(),
        "no second open was spawned"
    );
}

#[test]
fn file_open_under_another_case_reuses_the_tab() {
    let (mut app, held) = app_holding_lib();
    let _task: iced::Task<Message> =
        app.handle_document_file_opened(Some(PathBuf::from("/w/proj/lib.snxsch")));
    assert_one_tab_for(&app, &held);
}

#[test]
fn the_pcb_open_under_another_case_reuses_the_tab() {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    let held = PathBuf::from("/w/proj/Board.snxpcb");
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "Board".to_string(),
        path: held.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Pcb,
    });
    focus_another_tab(&mut app);
    let _task: iced::Task<Message> =
        app.handle_document_file_opened(Some(PathBuf::from("/w/proj/BOARD.snxpcb")));
    assert_one_tab_for(&app, &held);
}

#[test]
fn recents_under_another_spelling_reuses_the_tab() {
    let (mut app, dir, file) = app_holding_real_lib();
    let _task: iced::Task<Message> = app.handle_open_recent_project(dotted(&dir));
    assert_one_tab_for(&app, &file);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn child_sheet_double_click_under_another_spelling_reuses_the_tab() {
    let (mut app, dir, file) = app_holding_real_lib();
    app.open_or_focus_sheet_path(dotted(&dir));
    assert_one_tab_for(&app, &file);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn erc_navigation_under_another_case_reuses_the_tab() {
    let (mut app, held) = app_holding_lib();
    app.ensure_sheet_open_and_active(&PathBuf::from("/w/proj/LIB.snxsch"));
    assert_one_tab_for(&app, &held);
}

#[test]
fn a_primitive_opened_under_another_case_reuses_the_tab() {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    let held = PathBuf::from("/w/proj/Comp.snxsym");
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "Comp".to_string(),
        path: held.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::SymbolEditor(held.clone()),
    });
    focus_another_tab(&mut app);
    let _task: iced::Task<Message> =
        app.handle_open_primitive(PathBuf::from("/w/proj/COMP.snxsym"));
    assert_one_tab_for(&app, &held);
}

#[test]
fn an_open_already_in_flight_under_another_case_is_not_spawned_twice() {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    let first = PathBuf::from("/w/proj/Lib.snxsch");
    app.document_state.pending_opens.insert(first.clone());
    let _task: iced::Task<Message> =
        app.handle_document_file_opened(Some(PathBuf::from("/w/proj/lib.snxsch")));
    assert_eq!(app.document_state.pending_opens.len(), 1);
    assert!(app.document_state.pending_opens.contains(&first));
}

/// The #1149 data-loss story: with one tab per file, Ctrl+S on `Lib` and then
/// on `lib` is two saves of one document, so no second clean document can
/// disagree with the bytes on disk.
#[test]
fn opening_both_spellings_never_yields_two_documents_for_one_file() {
    let (mut app, held) = app_holding_lib();
    for spelling in [
        "/w/proj/lib.snxsch",
        "/w/proj/LIB.snxsch",
        "/w/proj/./Lib.snxsch",
    ] {
        let _task: iced::Task<Message> =
            app.handle_document_file_opened(Some(PathBuf::from(spelling)));
    }
    assert_one_tab_for(&app, &held);
}

#[test]
fn a_case_sensitive_project_keeps_two_case_variants_as_two_files() {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::PRESERVE;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    app.open_schematic_tab(
        PathBuf::from("/w/proj/Lib.snxsch"),
        "Lib".to_string(),
        sheet(),
    );
    assert_eq!(
        app.held_spelling(Path::new("/w/proj/lib.snxsch")),
        None,
        "on a case-sensitive volume `lib` is another file"
    );
}

#[test]
fn a_sheet_in_a_subfolder_finds_its_projects_fold() {
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    let held = PathBuf::from("/w/proj/sub/Lib.snxsch");
    app.open_schematic_tab(held.clone(), "Lib".to_string(), sheet());
    assert_eq!(
        app.fold_for_path(Path::new("/w/proj/sub/deeper/lib.snxsch")),
        CaseFold::FOLD,
        "a nested sheet keys under the project's volume rule"
    );
    assert_eq!(
        app.held_spelling(Path::new("/w/proj/sub/lib.snxsch")),
        Some(held),
        "the subfolder case variant is the same file"
    );
}

#[test]
fn save_as_onto_another_case_of_an_open_sheet_is_refused() {
    let (mut app, held) = app_holding_lib();
    // `Other` is the active tab; Save As targets `lib.snxsch`, another
    // spelling of the open `Lib.snxsch`.
    app.document_state.active_path = Some(PathBuf::from("/w/proj/Other.snxsch"));
    assert!(app.save_as_target_is_other_open_document(Path::new("/w/proj/lib.snxsch")));
    assert!(app.save_as_target_is_other_open_document(&held));
    app.document_state.active_path = Some(held.clone());
    assert!(
        !app.save_as_target_is_other_open_document(Path::new("/w/proj/lib.snxsch")),
        "re-spelling the active document itself is not another document"
    );
}

#[test]
fn primitive_save_as_onto_another_case_of_an_open_tab_is_refused() {
    use signex_library::{Symbol, SymbolFile};
    let mut app = app_workspace("/w/proj", &[]);
    app.document_state.projects[0].case_fold = CaseFold::FOLD;
    app.document_state.projects[0].case_probed = true;
    app.document_state.projects[0].case_decided = true;
    for name in ["Lib.snxsym", "Other.snxsym"] {
        let path = PathBuf::from("/w/proj").join(name);
        let file = SymbolFile::from_symbol(Symbol::empty("sym"));
        app.document_state.symbol_editors.insert(
            path.clone(),
            crate::app::SymbolDocument::new(path.clone(), file),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: name.to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::SymbolEditor(path),
        });
    }
    let error = app
        .retarget_primitive(
            Path::new("/w/proj/Other.snxsym"),
            Path::new("/w/proj/lib.snxsym"),
            None,
        )
        .expect_err("lib.snxsym is Lib.snxsym");
    assert!(error.to_string().contains("already open"));
    assert!(
        app.document_state
            .symbol_editors
            .contains_key(Path::new("/w/proj/Other.snxsym")),
        "nothing moved"
    );
    // A case-only rename of the same tab stays allowed.
    app.retarget_primitive(
        Path::new("/w/proj/Other.snxsym"),
        Path::new("/w/proj/OTHER.snxsym"),
        None,
    )
    .expect("case-only rename of one tab");
}

#[test]
fn a_project_folder_spelled_in_another_case_still_finds_its_fold() {
    let (app, held) = app_holding_lib();
    assert_eq!(
        app.held_spelling(Path::new("/W/PROJ/lib.snxsch")),
        Some(held)
    );
}
