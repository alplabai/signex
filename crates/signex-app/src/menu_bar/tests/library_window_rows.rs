//! Tools ▸ Library Editor and View ▸ Window ▸ Close All Documents, the rows #2247
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
            ..Default::default()
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
            provenance: Default::default(),
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
    assert!(!row_is_live(&bare_app(), "View/Window/Close All Documents"));
    assert!(row_is_live(
        &symbol_editor_app(),
        "View/Window/Close All Documents"
    ));
    assert!(row_is_live(
        &loaded_board_app(),
        "View/Window/Close All Documents"
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
        project_id: None,
        kind: TabKind::SymbolEditor(extra),
    });
    assert_eq!(app.document_state.tabs.len(), 2);
    let task = click(&mut app, "View/Window/Close All Documents");
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
            click(&mut app, "View/Window/Close All Documents")
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

// ─── View ▸ Window ▸ Layouts (mockup row 166) ────────────────────────────────────

/// The workspace-layout store of these tests: nothing reaches `prefs.json`.
struct NoLayoutWrites;

impl crate::workspace_layouts::WorkspaceLayoutsStore for NoLayoutWrites {
    fn write(&self, _: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}

/// A schematic app with no layouts of its own and no edited built-in.
fn layouts_app() -> Signex {
    let mut app = one_symbol_selected();
    let layouts = &mut app.ui_state.workspace_layouts;
    layouts.layouts = Default::default();
    layouts.store = std::sync::Arc::new(NoLayoutWrites);
    app
}

/// Save the arrangement on screen as `name`.
fn save_layout_as(app: &mut Signex, name: &str) {
    use crate::workspace_layouts::SaveLayoutMsg;
    let _ = click(app, "View/Window/Layouts/Save Layout…");
    let _ = app.update(SaveLayoutMsg::NameEdited(name.into()).into());
    let _ = app.update(SaveLayoutMsg::Confirmed.into());
}

/// The labels of View ▸ Window ▸ Layouts.
fn layouts_rows(app: &Signex) -> Vec<String> {
    let model = menu_bar_model(&menu_context(app));
    let submenu = |menu: &Menu, title: &str| {
        menu.entries.iter().find_map(|entry| match entry {
            MenuEntry::Submenu(sub) if sub.title == title => Some(sub.clone()),
            _ => None,
        })
    };
    let view = model
        .menus
        .iter()
        .find(|m| m.title == "View")
        .expect("View");
    let window = submenu(view, "Window").expect("Window");
    let layouts = submenu(&window, "Layouts").expect("Layouts");
    layouts
        .entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Item(item) => item.label.clone(),
            MenuEntry::Submenu(sub) => format!("{}>", sub.title),
            MenuEntry::Separator => "---".to_owned(),
        })
        .collect()
}

#[test]
fn a_built_in_row_arranges_the_panels_and_takes_the_check_mark() {
    use crate::dock::PanelPosition;
    use crate::panels::PanelKind;
    let mut app = layouts_app();
    assert_eq!(
        with_row(&app, "View/Window/Layouts/Review", |row| row.checked),
        Some(false)
    );
    let _ = click(&mut app, "View/Window/Layouts/Review");
    let dock = &app.document_state.dock;
    assert_eq!(dock.panel_kinds(PanelPosition::Right), [PanelKind::History]);
    assert_eq!(
        dock.panel_kinds(PanelPosition::Bottom),
        [PanelKind::Messages, PanelKind::CommandLog]
    );
    assert_eq!(
        with_row(&app, "View/Window/Layouts/Review", |row| row.checked),
        Some(true)
    );
    assert_eq!(
        with_row(&app, "View/Window/Layouts/Schematic", |row| row.checked),
        Some(false)
    );
}

#[test]
fn save_and_manage_rows_open_their_dialogs() {
    let mut app = layouts_app();
    let _ = click(&mut app, "View/Window/Layouts/Save Layout…");
    assert!(probe(&app).save_layout);
    let mut app = layouts_app();
    let _ = click(&mut app, "View/Window/Layouts/Manage Layouts…");
    assert!(probe(&app).manage_layouts);
}

#[test]
fn your_layouts_are_listed_and_a_row_applies_its_layout() {
    let mut app = layouts_app();
    save_layout_as(&mut app, "Bench");
    // Something else on screen, then back through the row.
    let _ = click(&mut app, "View/Window/Layouts/Notebook");
    assert_eq!(
        layouts_rows(&app),
        [
            "BUILT-IN",
            "Schematic",
            "Layout",
            "Review",
            "Notebook",
            "YOUR LAYOUTS",
            "Bench",
            "---",
            "Save Layout…",
            "Manage Layouts…",
        ]
    );
    assert_eq!(
        with_row(&app, "View/Window/Layouts/Bench", |row| row.checked),
        Some(false)
    );
    let saved = app.ui_state.workspace_layouts.layouts.user()[0]
        .dock
        .clone()
        .expect("panels");
    let _ = click(&mut app, "View/Window/Layouts/Bench");
    assert_eq!(
        crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock),
        saved
    );
    assert_eq!(
        with_row(&app, "View/Window/Layouts/Bench", |row| row.checked),
        Some(true)
    );
}

#[test]
fn the_reset_row_shows_only_while_the_active_built_in_is_edited() {
    use crate::workspace_layouts::{BuiltinLayout, LayoutId, ManageLayoutsMsg};
    let reset = "Reset \u{201c}Schematic\u{201d}";
    let mut app = layouts_app();
    let _ = click(&mut app, "View/Window/Layouts/Schematic");
    assert!(!layouts_rows(&app).iter().any(|row| row == reset));
    let _ = app.update(ManageLayoutsMsg::Requested.into());
    let _ = app
        .update(ManageLayoutsMsg::RowSelected(LayoutId::Builtin(BuiltinLayout::Schematic)).into());
    let _ = app.update(ManageLayoutsMsg::UpdateFromCurrentRequested.into());
    assert_eq!(layouts_rows(&app).last().map(String::as_str), Some(reset));
    let _ = click(&mut app, &format!("View/Window/Layouts/{reset}"));
    assert!(!layouts_rows(&app).iter().any(|row| row == reset));
    assert!(
        !app.ui_state
            .workspace_layouts
            .layouts
            .is_edited(BuiltinLayout::Schematic)
    );
}

/// 2026-11-20 decision: BUILT-IN and YOUR LAYOUTS are header captions, not
/// greyed rows. A header is neither a live row nor a disabled one, carries no
/// command or check mark, and a click on it has no message to send.
#[test]
fn the_layout_group_captions_are_headers() {
    let mut app = layouts_app();
    save_layout_as(&mut app, "Bench");
    for caption in ["BUILT-IN", "YOUR LAYOUTS"] {
        let path = format!("View/Window/Layouts/{caption}");
        with_row(&app, &path, |row| {
            assert!(row.is_header(), "{path}");
            assert!(matches!(row.state, ItemState::Header), "{path}");
            assert!(!row.is_enabled(), "{path}");
            assert!(row.dispatches().is_none(), "{path}");
            assert!(row.shortcut.is_none(), "{path}");
            assert_eq!(row.checked, None, "{path}");
        });
    }
}

/// The two captions are the only headers in the bar, so every other row,
/// live or greyed, stays a command row.
#[test]
fn only_the_layout_captions_are_headers() {
    fn headers(menu: &Menu, parent: &str, out: &mut Vec<String>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) if item.is_header() => {
                    out.push(format!("{parent}/{}", item.label));
                }
                MenuEntry::Submenu(sub) => headers(sub, &format!("{parent}/{}", sub.title), out),
                MenuEntry::Item(_) | MenuEntry::Separator => {}
            }
        }
    }
    let mut app = layouts_app();
    save_layout_as(&mut app, "Bench");
    let mut found = Vec::new();
    for menu in &menu_bar_model(&menu_context(&app)).menus {
        headers(menu, &menu.title, &mut found);
    }
    assert_eq!(
        found,
        [
            "View/Window/Layouts/BUILT-IN",
            "View/Window/Layouts/YOUR LAYOUTS"
        ]
    );
}

/// Mockup row 166: Save Layout… draws the save glyph and Reset the undo glyph in
/// the check column; every other row of the bar keeps that column for its
/// check mark alone.
#[test]
fn save_layout_and_reset_are_the_rows_with_an_icon() {
    use crate::menu_bar::model::MenuIcon;
    use crate::workspace_layouts::{BuiltinLayout, LayoutId, ManageLayoutsMsg};
    fn icons(menu: &Menu, parent: &str, out: &mut Vec<(String, MenuIcon)>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) => {
                    if let Some(icon) = item.icon {
                        out.push((format!("{parent}/{}", item.label), icon));
                    }
                }
                MenuEntry::Submenu(sub) => icons(sub, &format!("{parent}/{}", sub.title), out),
                MenuEntry::Separator => {}
            }
        }
    }
    let mut app = layouts_app();
    let _ = click(&mut app, "View/Window/Layouts/Schematic");
    let _ = app.update(ManageLayoutsMsg::Requested.into());
    let _ = app
        .update(ManageLayoutsMsg::RowSelected(LayoutId::Builtin(BuiltinLayout::Schematic)).into());
    let _ = app.update(ManageLayoutsMsg::UpdateFromCurrentRequested.into());
    let mut found = Vec::new();
    for menu in &menu_bar_model(&menu_context(&app)).menus {
        icons(menu, &menu.title, &mut found);
    }
    assert_eq!(
        found,
        [
            (
                "View/Window/Layouts/Save Layout…".to_owned(),
                MenuIcon::Save
            ),
            (
                "View/Window/Layouts/Reset \u{201c}Schematic\u{201d}".to_owned(),
                MenuIcon::Reset
            ),
        ]
    );
}
