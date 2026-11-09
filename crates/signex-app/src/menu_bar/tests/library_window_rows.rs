//! Tools ▸ Library Editor and Window ▸ Close All Documents, the rows #2247
//! wired from stubs.

use super::*;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

/// Run `task` the way the runtime does, feeding every message back.
fn settle(app: &mut Signex, task: Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let next = app.update(message);
        settle(app, next);
    }
}

/// A real library on disk, mounted and listed as open, with no tab.
fn mounted_library(app: &mut Signex, dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let file = dir.join(name).join(format!("{name}.snxlib"));
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&file, manifest, options).expect("init");
    app.library.set.mount(Box::new(adapter)).expect("mount");
    app.library
        .open_libraries
        .push(crate::library::state::OpenLibrary {
            root: file.clone(),
            display_name: name.into(),
            library_id,
            tables: std::collections::HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            pricing: Default::default(),
        });
    file
}

/// The path of the active tab when it is a Library Browser.
fn active_browser(app: &Signex) -> Option<std::path::PathBuf> {
    let document = &app.document_state;
    match &document.tabs.get(document.active_tab)?.kind {
        TabKind::LibraryBrowser(path) => Some(path.clone()),
        _ => None,
    }
}

/// A schematic tab is active and two libraries are open, none in a tab.
fn schematic_and_two_libraries(dir: &std::path::Path) -> (Signex, [std::path::PathBuf; 2]) {
    signex_library::git_identity::test_identity::install();
    let mut app = one_symbol_selected();
    app.library.open_libraries.clear();
    let first = mounted_library(&mut app, dir, "First");
    let second = mounted_library(&mut app, dir, "Second");
    (app, [first, second])
}

#[test]
fn library_editor_is_greyed_with_no_library_open() {
    let mut app = one_symbol_selected();
    app.library.open_libraries.clear();
    assert!(!row_is_live(&app, "Tools/Library Editor"));
}

#[test]
fn library_editor_opens_the_first_open_library_in_a_browser_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, [first, _]) = schematic_and_two_libraries(dir.path());
    assert!(row_is_live(&app, "Tools/Library Editor"));
    let tabs = app.document_state.tabs.len();
    let task = click(&mut app, "Tools/Library Editor");
    settle(&mut app, task);
    assert_eq!(app.document_state.tabs.len(), tabs + 1, "one tab opened");
    assert_eq!(active_browser(&app), Some(first));
}

#[test]
fn library_editor_focuses_an_open_browser_tab_without_opening_another() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, [_, second]) = schematic_and_two_libraries(dir.path());
    let task = app.update(Message::Library(
        crate::library::LibraryMessage::OpenLibraryBrowser(second.clone()),
    ));
    settle(&mut app, task);
    assert_eq!(active_browser(&app), Some(second.clone()));
    // Back to the schematic: the browser tab stays open behind it.
    app.document_state.active_tab = 0;
    app.sync_active_tab();
    assert_eq!(active_browser(&app), None);
    let tabs = app.document_state.tabs.len();

    let task = click(&mut app, "Tools/Library Editor");
    settle(&mut app, task);
    assert_eq!(app.document_state.tabs.len(), tabs, "no new tab");
    assert_eq!(
        active_browser(&app),
        Some(second),
        "the open browser tab is focused, not the first library"
    );
}

fn open_browser(app: &mut Signex, path: &std::path::Path) {
    let task = app.update(Message::Library(
        crate::library::LibraryMessage::OpenLibraryBrowser(path.to_path_buf()),
    ));
    settle(app, task);
}

#[test]
fn library_editor_keeps_the_active_browser_tab_over_a_later_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, [first, second]) = schematic_and_two_libraries(dir.path());
    open_browser(&mut app, &first);
    open_browser(&mut app, &second);
    let first_tab = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == first)
        .expect("first browser tab");
    app.document_state.active_tab = first_tab;
    app.sync_active_tab();
    let tabs = app.document_state.tabs.len();

    let task = click(&mut app, "Tools/Library Editor");
    settle(&mut app, task);
    assert_eq!(app.document_state.tabs.len(), tabs, "no new tab");
    assert_eq!(active_browser(&app), Some(first));
}

#[test]
fn library_editor_opens_the_library_of_the_active_editor_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, [_, second]) = schematic_and_two_libraries(dir.path());
    let symbol = second
        .parent()
        .expect("library folder")
        .join("symbols")
        .join("part.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state.symbol_editors.insert(
        symbol.clone(),
        crate::app::SymbolDocument::new(symbol.clone(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: symbol.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(symbol),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.sync_active_tab();
    assert_eq!(app.active_editor_library_root(), Some(second.clone()));
    let tabs = app.document_state.tabs.len();

    let task = click(&mut app, "Tools/Library Editor");
    settle(&mut app, task);
    assert_eq!(app.document_state.tabs.len(), tabs + 1, "one tab opened");
    assert_eq!(
        active_browser(&app),
        Some(second),
        "the editor's own library, not the first open one"
    );
}

#[test]
fn close_all_documents_is_greyed_with_no_tab() {
    assert!(!row_is_live(&bare_app(), "Window/Close All Documents"));
    assert!(row_is_live(
        &symbol_editor_app(),
        "Window/Close All Documents"
    ));
    assert!(row_is_live(
        &loaded_board_app(),
        "Window/Close All Documents"
    ));
}

#[test]
fn close_all_documents_closes_every_tab() {
    let mut app = one_symbol_selected();
    let extra = std::path::PathBuf::from("/w/menu-rows.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state.symbol_editors.insert(
        extra.clone(),
        crate::app::SymbolDocument::new(extra.clone(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "symbol".into(),
        path: extra.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(extra),
    });
    assert_eq!(app.document_state.tabs.len(), 2);
    let task = click(&mut app, "Window/Close All Documents");
    settle(&mut app, task);
    assert!(app.document_state.tabs.is_empty(), "every tab closed");
    assert!(app.document_state.active_engine().is_none());
}

/// The unsaved-work handling is the tab menu's Close All Documents: a dirty
/// sheet is parked as a draft, not discarded and not prompted for.
#[test]
fn close_all_documents_parks_a_dirty_sheet_like_the_tab_menu() {
    let close = |via_menu: bool| {
        let mut app = one_symbol_selected();
        let path = app.document_state.active_path.clone().expect("path");
        app.document_state.mark_document_dirty(&path);
        let task = if via_menu {
            click(&mut app, "Window/Close All Documents")
        } else {
            app.update(Message::ContextMenu(crate::app::ContextMenuMsg::TabAction(
                crate::app::TabContextAction::CloseAll,
            )))
        };
        settle(&mut app, task);
        (probe(&app), app.unsaved_set().paths())
    };
    let (menu, menu_unsaved) = close(true);
    let (tab_menu, tab_unsaved) = close(false);
    assert_eq!(menu, tab_menu);
    assert_eq!(menu_unsaved, tab_unsaved);
    assert_eq!(menu.tabs, 0);
    assert!(menu.dirty, "the dirty sheet is kept as a parked draft");
}
