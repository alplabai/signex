//! What the Tools and Help menus' rows do.

use super::*;
use signex_types::schematic::{SelectedItem, SelectedKind};

#[test]
fn preferences_and_the_calculator_and_shortcuts_open_their_dialogs() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Tools/Preferences...");
    assert_eq!(
        probe(&app),
        Probe {
            preferences: true,
            ..before.clone()
        }
    );

    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Tools/Passive Network Calculator...");
    assert_eq!(
        probe(&app),
        Probe {
            passive_calculator: true,
            ..before.clone()
        }
    );

    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Help/Keyboard Shortcuts");
    assert_eq!(
        probe(&app),
        Probe {
            shortcuts: true,
            ..before.clone()
        }
    );

    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Help/About Signex");
    assert_eq!(
        probe(&app),
        Probe {
            about: true,
            ..before
        }
    );
}

#[test]
fn keyboard_shortcuts_and_about_close_an_open_context_menu() {
    for path in ["Help/Keyboard Shortcuts", "Help/About Signex"] {
        let mut app = one_symbol_selected();
        let _ = app.update(Message::ContextMenu(crate::app::ContextMenuMsg::Show(
            300.0, 300.0,
        )));
        assert!(app.interaction_state.context_menu.is_some());
        let _ = click(&mut app, path);
        assert!(app.interaction_state.context_menu.is_none(), "{path}");
    }
}

#[test]
fn link_rows_and_the_sample_project_start_a_job_and_move_nothing() {
    for path in [
        "Help/Documentation",
        "Help/Report an Issue",
        "Help/Open Sample Project",
    ] {
        let mut app = one_symbol_selected();
        let before = probe(&app);
        let task = click(&mut app, path);
        assert!(has_work(&task), "{path} started nothing");
        assert_eq!(probe(&app), before, "{path}");
    }
}

#[test]
fn new_part_and_remove_part_edit_the_symbols_parts() {
    let mut app = symbol_editor_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxsym");
    let parts = |app: &Signex| {
        app.document_state.symbol_editors[&path]
            .primitive()
            .part_count
    };
    assert!(!row_is_live(&app, "Tools/Remove Part"), "one part only");
    let before = parts(&app);

    let _ = click(&mut app, "Tools/New Part");
    assert_eq!(parts(&app), before + 1);
    assert_eq!(
        app.document_state.symbol_editors[&path].active_part,
        before + 1
    );

    let _ = click(&mut app, "Tools/Remove Part");
    assert_eq!(parts(&app), before);
}

#[test]
fn part_rows_are_greyed_outside_a_symbol_editor() {
    let app = one_symbol_selected();
    assert!(!row_is_live(&app, "Tools/New Part"));
    assert!(!row_is_live(&app, "Tools/Remove Part"));
}

/// #1922: Tools ▸ Document Options is greyed unless the active tab is a symbol
/// or footprint editor inside an open library, the one case its handler acts in.
#[test]
fn document_options_is_greyed_outside_a_library_editor() {
    for app in [
        bare_app(),
        one_symbol_selected(),
        symbol_editor_app(),
        footprint_editor_app(),
    ] {
        assert!(!row_is_live(&app, "Tools/Document Options..."));
    }
}

#[test]
fn document_options_opens_the_modal_for_an_editor_in_a_mounted_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("menu-rows.snxlib");
    crate::library::commands::create_library_file(&root, &Default::default()).expect("library");
    for kind in ["symbol", "footprint"] {
        let mut app = bare_app();
        app.library.open_library(root.clone()).expect("mounts");
        let file = root
            .join(format!("{kind}s"))
            .join(format!("part.snx{}", &kind[..3]));
        let tab_kind = if kind == "symbol" {
            TabKind::SymbolEditor(file.clone())
        } else {
            TabKind::FootprintEditor(file.clone())
        };
        app.document_state.tabs.push(TabInfo {
            title: "part".into(),
            path: file,
            cached_document: None,
            project_id: None,
            kind: tab_kind,
        });
        app.document_state.active_tab = 0;
        assert!(row_is_live(&app, "Tools/Document Options..."), "{kind}");
        assert!(app.library.document_options.is_none());
        let before = probe(&app);
        let _ = click(&mut app, "Tools/Document Options...");
        let modal = app.library.document_options.as_ref().expect("the modal");
        assert_eq!(
            app.library
                .library_at(&modal.library_path)
                .map(|l| l.root.clone()),
            Some(root.clone())
        );
        assert_eq!(probe(&app), before, "nothing else moves");
    }
}

/// Up Hierarchy is live on a child sheet; with the sheet outside a loaded
/// project the handler says so in Messages instead of doing nothing silently.
#[test]
fn up_hierarchy_reports_when_the_sheet_is_not_in_a_loaded_project() {
    let _ = crate::diagnostics::init_logging();
    let needle = "Up Hierarchy: the focused sheet is not part of a loaded project";
    let mut app = one_symbol_selected();
    app.document_state
        .panel_ctx
        .navigator
        .replace_hierarchy(placed_child_hierarchy(), None, &[]);
    app.document_state.tabs[0].path = std::path::PathBuf::from("/p/child.snxsch");
    assert!(row_is_live(&app, "Tools/Up Hierarchy"));
    let before = crate::diagnostics::logged_count(needle);
    let _ = click(&mut app, "Tools/Up Hierarchy");
    assert!(crate::diagnostics::logged_count(needle) > before);
}

fn placed_child_hierarchy() -> crate::app::sheet_hierarchy::SheetHierarchy {
    let root = std::path::PathBuf::from("/p/root.snxsch");
    let child = std::path::PathBuf::from("/p/child.snxsch");
    let mut root_sheet = sheet_of(Vec::new());
    root_sheet.child_sheets = vec![signex_types::schematic::ChildSheet {
        uuid: Uuid::from_u128(9),
        name: "Child".to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: signex_types::schematic::FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }];
    let sheets = std::collections::HashMap::from([
        (root.clone(), root_sheet),
        (child, sheet_of(Vec::new())),
    ]);
    crate::app::sheet_hierarchy::build_sheet_hierarchy(
        &sheets,
        &[],
        &root,
        &[],
        crate::app::CaseFold::host_default(),
    )
}

#[test]
fn up_hierarchy_is_greyed_on_a_top_level_sheet() {
    assert!(!row_is_live(&one_symbol_selected(), "Tools/Up Hierarchy"));
}

#[test]
fn stub_rows_are_not_live() {
    let app = one_symbol_selected();
    for path in [
        "Tools/Assign Footprints",
        "Tools/Library Editor",
        "Tools/Net Inspector",
        "Window/Tile Horizontally",
        "Window/Tile Vertically",
        "Window/Close All Documents",
        "Tools/Annotation/Back Annotate Schematics...",
        "Tools/Annotation/Number Schematic Sheets...",
    ] {
        assert!(!row_is_live(&app, path), "{path}");
    }
    // Selecting something must not wake a stub.
    let mut app = app;
    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(
        uuid::Uuid::from_u128(1),
        SelectedKind::Symbol,
    )];
    assert!(!row_is_live(&app, "Tools/Net Inspector"));
}
