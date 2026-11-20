use std::path::{Path, PathBuf};

use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};

use crate::app::Signex;
use crate::app::state::LoadedProject;
use crate::app::state::project_fs::git_marker;

/// #1383: an unchanged engine keeps the symbol lists (no per-edit
/// O(symbols) re-clone); an edit rebuilds them.
#[test]
fn symbol_lists_are_reused_until_the_engine_revision_moves() {
    let (mut app, _task) = Signex::new();
    app.open_schematic_tab(
        PathBuf::from("snx-1383.snxsch"),
        "snx-1383".to_string(),
        signex_types::schematic::SchematicSheet::default(),
    );
    app.refresh_panel_ctx();
    let key = app.document_state.panel_ctx.symbol_lists_key;
    assert!(key.is_some(), "schematic engine must key the lists");

    let sentinel = (
        "Z1".to_string(),
        String::new(),
        String::new(),
        String::new(),
    );
    app.document_state.panel_ctx.placed_symbols = vec![sentinel.clone()];
    app.refresh_panel_ctx();
    assert_eq!(app.document_state.panel_ctx.placed_symbols, vec![sentinel]);

    app.document_state.panel_ctx.symbol_lists_key = Some((0, 0));
    app.refresh_panel_ctx();
    assert!(app.document_state.panel_ctx.placed_symbols.is_empty());
    assert_eq!(app.document_state.panel_ctx.symbol_lists_key, key);
}

/// #2262: a component editor tab, active in a fresh layout, folds the
/// left dock to its rail on the next panel refresh.
#[test]
fn an_active_component_tab_folds_the_fresh_left_dock() {
    use crate::app::{ComponentEditorTab, TabInfo, TabKind};
    use crate::dock::PanelPosition;
    use crate::library::state::EditorAddress;

    let (mut app, _task) = Signex::new();
    app.document_state.dock = crate::dock::graphite_first_run();
    app.refresh_panel_ctx();
    assert!(!app.document_state.dock.is_collapsed(PanelPosition::Left));

    let row = crate::library::component_preview::test_row();
    let library_path = PathBuf::from("/tmp/dock-seed/Lib.snxlib");
    let address = EditorAddress::new(
        library_path.clone(),
        "R".into(),
        signex_library::RowId::from_uuid(row.row_id),
    );
    app.document_state.tabs.push(TabInfo {
        title: "R1".into(),
        path: address.synthetic_tab_path(),
        cached_document: None,
        project_id: None,
        kind: TabKind::ComponentEditor(ComponentEditorTab {
            library_path,
            table: "R".into(),
            row_id: address.row_id,
        }),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.refresh_panel_ctx();
    assert!(app.document_state.dock.is_collapsed(PanelPosition::Left));
}

/// #1383: the project tree is rebuilt only when what it shows changes,
/// and a changed sheet state does rebuild it.
#[test]
fn project_tree_is_reused_until_the_projects_change() {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: PathBuf::from("snx-1383-tree/p.snxprj"),
        data: ProjectData {
            document_id: None,
            name: "p".to_string(),
            description: None,
            dir: "snx-1383-tree".to_string(),
            schematic_root: Some("a.snxsch".to_string()),
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
            agent_journal: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.refresh_panel_ctx();
    let builds = app.document_state.panel_ctx.project_tree_builds;
    assert!(builds > 0 && !app.document_state.panel_ctx.project_tree.is_empty());

    app.refresh_panel_ctx();
    app.refresh_panel_ctx();
    assert_eq!(app.document_state.panel_ctx.project_tree_builds, builds);

    app.document_state.projects[0].data.pcb_file = Some("b.snxpcb".into());
    app.refresh_panel_ctx();
    assert_eq!(app.document_state.panel_ctx.project_tree_builds, builds + 1);
}

/// #1098: the probe request must name `<dir>/.git` for the project dir
/// and each library dir, or the Enable Version Control row never appears.
#[test]
fn fs_request_names_the_git_marker_of_project_and_library_dirs() {
    let dir = Path::new("snx-1098-proj");
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "P".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: vec![LibraryEntry {
                path: PathBuf::from("lib/L.snxlib"),
                kind: LibraryEntryKind::ProjectLocal,
                library_id: None,
                unknown: Default::default(),
            }],
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });

    let request = app.project_fs_request();
    assert!(request.exists.contains(&git_marker(dir)));
    assert!(request.exists.contains(&git_marker(&dir.join("lib"))));
}
