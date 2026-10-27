//! #780 — two leftovers from the #773 review.
//!
//! 1. Reloading a symbol from disk (History ▸ Restore) left its
//!    `dirty_paths` entry behind, so Quit asked to save a file that
//!    already matched disk. The footprint reload already cleared it.
//! 2. Edit coalescing is keyed on `(field, active footprint)`, so
//!    editing A, visiting B and returning to A with no edit in between
//!    merged the two A edits into one undo step.

use signex_app::app::{
    FootprintDocument, LoadedProject, Message, Signex, SymbolDocument, TabInfo, TabKind, WindowMsg,
};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_app::panels::PanelMsg;
use signex_library::adapters::local_git::project::LocalGitProjectAdapter;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use signex_types::project::ProjectData;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn push_tab(app: &mut Signex, path: &Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

fn push_project(app: &mut Signex, dir: &Path) {
    let prj = dir.join("p.snxprj");
    std::fs::write(&prj, b"{}").unwrap();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj,
        data: ProjectData {
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
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
}

#[test]
fn symbol_reload_from_disk_clears_the_dirty_marker_so_quit_does_not_prompt() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("s.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("orig"));
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();
    let adapter = LocalGitProjectAdapter::open_or_init(tmp.path().to_path_buf()).unwrap();
    let sha = adapter
        .commit_path(Path::new("s.snxsym"), "add symbol")
        .unwrap()
        .expect("the symbol file changed, so a commit is made")
        .to_string();

    let (mut app, _t) = Signex::new();
    push_project(&mut app, tmp.path());
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));

    panel(&mut app, PanelMsg::SymEditorSetSymbolName("edited".into()));
    assert!(app.document_state.dirty_paths.contains(&path));

    // A dirty draft makes Restore a two-click confirm: the first click
    // only arms it, the second reloads.
    panel(
        &mut app,
        PanelMsg::HistoryRestoreClicked { sha: sha.clone() },
    );
    assert_eq!(
        app.document_state.history.pending_restore.as_deref(),
        Some(sha.as_str()),
        "the first click arms the confirm"
    );
    panel(&mut app, PanelMsg::HistoryRestoreClicked { sha });

    let editor = &app.document_state.symbol_editors[&path];
    assert_eq!(editor.primitive().name, "orig", "the reload took effect");
    assert!(
        !app.document_state.dirty_paths.contains(&path),
        "a reloaded symbol matches disk, so it is not dirty"
    );
    assert!(!app.document_state.tabs[0].dirty);

    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "Quit must not ask to save a file that matches disk"
    );
}

#[test]
fn footprint_edits_around_a_sibling_switch_are_two_undo_steps() {
    let path = PathBuf::from("two.snxfpt");
    let mut file = FootprintFile::from_footprint(Footprint::empty("a"));
    file.footprints.push(Footprint::empty("b"));
    let (mut app, _t) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));

    let select = |app: &mut Signex, idx: usize| {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectActiveIdx(idx)),
        }));
    };

    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("one".into()),
    );
    select(&mut app, 1);
    select(&mut app, 0);
    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("two".into()),
    );

    let editor = &app.document_state.footprint_editors[&path];
    assert_eq!(editor.active_idx, 0);
    assert_eq!(
        editor.history.len(),
        2,
        "the second edit of A must be its own undo step"
    );
}

#[test]
fn footprint_typing_in_one_field_still_coalesces() {
    let path = PathBuf::from("one.snxfpt");
    let file = FootprintFile::from_footprint(Footprint::empty("a"));
    let (mut app, _t) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));

    for text in ["o", "on", "one"] {
        panel(
            &mut app,
            PanelMsg::FpEditorSetFootprintDescription(text.into()),
        );
    }

    assert_eq!(app.document_state.footprint_editors[&path].history.len(), 1);
}
