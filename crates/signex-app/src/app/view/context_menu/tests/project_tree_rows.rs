//! Mockup row 20: the Projects tree's right-click menus: the exact rows per node
//! role, the Planned flyout, the library's Add New flyout, the Add New to
//! Project flyout, and the strong / danger / planned row looks.

use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};
use signex_widgets::tree_view::{TreeIcon, TreeNode, key_of};

use super::TID;
use crate::app::view::context_menu::project_tree::kind_inks::KindInks;
use crate::app::view::context_menu::project_tree::library::library_add_new_entries;
use crate::app::view::context_menu::project_tree::planned::planned_entries;
use crate::app::view::context_menu::submenu::add_new_entries;
use crate::app::{
    ContextMenuMsg, ContextSubmenu, Message, ProjectTreeAction, ProjectTreeContextMenuState, Signex,
};
use crate::menu_bar::MenuMessage;

const SEP: &str = "---";

/// The file-kind inks of [`TID`]'s tokens.
fn inks() -> KindInks {
    KindInks::of(&signex_types::theme::theme_tokens(TID))
}

/// The text of a row's badge pill, if it has one.
fn badge_of(item: &DropdownItem<Message>) -> Option<&str> {
    item.badge.as_ref().map(signex_widgets::Label::as_str)
}

/// A project with a source document and a library, and the project dir's
/// `.git` probed absent so Enable Version Control is offered.
fn app_with_tree() -> Signex {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    app.document_view.panel_ctx.project_tree = vec![TreeNode::branch(
        "proj",
        TreeIcon::Folder,
        vec![
            TreeNode::branch(
                "Source Documents",
                TreeIcon::Folder,
                vec![TreeNode::leaf("a.snxsch", TreeIcon::SnxSchematic)],
            ),
            TreeNode::branch(
                "Libraries",
                TreeIcon::Folder,
                vec![TreeNode::leaf("Power.snxlib", TreeIcon::SnxLibrary)],
            ),
        ],
    )];
    let dir = std::path::Path::new("/p");
    app.document_state
        .project_fs
        .facts
        .set_exists(crate::app::state::project_fs::git_marker(dir), false);
    app
}

/// The menu of the tree row at `path`.
fn rows_at(app: &Signex, path: &[usize]) -> Vec<DropdownEntry<Message>> {
    let key = key_of(&app.document_view.panel_ctx.project_tree, path).expect("the row exists");
    let ctx = ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(key),
        add_new_only: false,
        window: None,
    };
    app.project_tree_entries(&ctx).expect("the menu has rows")
}

/// Labels in order, a separator as [`SEP`].
fn layout(rows: &[DropdownEntry<Message>]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row {
            DropdownEntry::Item(item) => Some(item.label.clone()),
            DropdownEntry::Separator => Some(SEP.to_string()),
            _ => None,
        })
        .collect()
}

fn item<'r>(rows: &'r [DropdownEntry<Message>], label: &str) -> &'r DropdownItem<Message> {
    rows.iter()
        .find_map(|row| match row {
            DropdownEntry::Item(item) if item.label == label => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing row {label:?}"))
}

/// The project root is grouped add / check and sync / file / settings /
/// close, then the Planned launcher; no planned row sits at the top level.
#[test]
fn the_project_root_menu_has_the_m20_rows_in_order() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0]);
    assert_eq!(
        layout(&rows),
        [
            "Add New to Project",
            "Add Existing to Project...",
            SEP,
            "Validate Project",
            "Update PCB Document",
            SEP,
            "Save",
            "Rename...",
            "Explore",
            SEP,
            "Project Options...",
            "Variant Manager",
            "Design Checklist",
            "Enable Version Control...",
            SEP,
            "Close Project Documents",
            "Close Project",
            SEP,
            "Planned",
        ]
    );
    // Every row but Close Project Documents carries a glyph.
    for row in &rows {
        if let DropdownEntry::Item(it) = row {
            assert_eq!(
                it.icon.is_some(),
                it.label != "Close Project Documents",
                "{:?}",
                it.label
            );
            assert!(!it.danger && !it.strong, "{:?}", it.label);
        }
    }
    let add_new = item(&rows, "Add New to Project");
    assert!(add_new.submenu && add_new.on_hover.is_some());
}

/// With `.git` present the root menu has no Enable Version Control row.
#[test]
fn the_project_root_offers_version_control_only_without_git() {
    let mut app = app_with_tree();
    app.document_state.project_fs.facts.set_exists(
        crate::app::state::project_fs::git_marker(std::path::Path::new("/p")),
        true,
    );
    let rows = rows_at(&app, &[0]);
    assert!(
        !layout(&rows)
            .iter()
            .any(|l| l == "Enable Version Control...")
    );
}

/// The Planned launcher reads muted and says why, yet opens its flyout on
/// hover and on click.
#[test]
fn the_planned_launcher_is_muted_but_opens_its_flyout() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0]);
    let planned = item(&rows, "Planned");
    assert!(planned.submenu);
    assert!(planned.disabled, "the launcher reads muted");
    assert_eq!(
        planned.tooltip.as_deref(),
        Some("Planned — not available yet")
    );
    assert!(matches!(
        planned.on_hover,
        Some((
            Message::ContextMenu(ContextMenuMsg::SubmenuHover(ContextSubmenu::ProjectPlanned)),
            Message::ContextMenu(ContextMenuMsg::SubmenuLeave),
        ))
    ));
    assert!(matches!(
        planned.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
            ContextSubmenu::ProjectPlanned
        )))
    ));
}

/// Mockup row 19: the Projects title bar + opens the Add New rows of the
/// project alone, at the pointer.
#[test]
fn the_title_bar_plus_opens_only_the_add_new_menu() {
    use crate::panels::{PanelMsg, ProjectsMsg};
    let mut app = app_with_tree();
    let main = app.ui_state.main_window_id.expect("the main window");
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::Projects(ProjectsMsg::AddNewPressed),
    )));
    let menu = app
        .interaction_state
        .project_tree_context_menu
        .as_ref()
        .expect("the menu opened");
    assert!(menu.add_new_only);
    let root = key_of(&app.document_view.panel_ctx.project_tree, &[0]);
    assert_eq!(menu.key, root);
    let texts = super::super::placement::tests::lay_out_layers(
        app.project_tree_context_menu_overlay(main),
        iced::Size::new(1000.0, 700.0),
    );
    assert!(texts.iter().any(|t| t.text == "Schematic"), "{texts:?}");
    assert!(
        !texts.iter().any(|t| t.text == "Validate Project"),
        "{texts:?}"
    );
}

/// Decision 2026-11-22: with no project open the + runs New Project.
#[test]
fn with_no_project_the_title_bar_plus_runs_new_project() {
    use crate::panels::{PanelMsg, ProjectsMsg};
    let (mut app, _boot) = Signex::new();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    assert!(app.document_view.panel_ctx.project_tree.is_empty());
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::Projects(ProjectsMsg::AddNewPressed),
    )));
    assert!(app.interaction_state.project_tree_context_menu.is_none());
    assert!(app.ui_state.new_project.is_some());
}

/// Decision 2026-11-22: the Compiled Design and Unions headers keep their
/// one action and have no three-dot menu (mockup rows 154 and 158).
#[test]
fn compiled_design_and_unions_have_no_more_menu() {
    use crate::panels::{PanelKind, title_actions};
    let (app, _boot) = Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.unions.has_sheet = true;
    assert_eq!(title_actions(PanelKind::CompiledDesign, &ctx).len(), 1);
    assert_eq!(title_actions(PanelKind::Unions, &ctx).len(), 1);
}

/// Mockup row 20: Variant Manager is a live row. Since mockup row 32 it
/// opens the project's Variant Manager tab, not the Project tab.
#[test]
fn the_variant_manager_row_opens_the_variants_section() {
    let mut app = app_with_tree();
    let id = app.document_state.projects[0].id;
    let rows = rows_at(&app, &[0]);
    let row = item(&rows, "Variant Manager");
    assert!(!row.disabled);
    let press = row.on_press.clone().expect("a live row");
    let _ = app.update(press);
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(tab.kind, crate::app::TabKind::VariantManager(p) if p == id));
    assert!(
        app.document_state
            .tabs
            .iter()
            .all(|tab| tab.kind.as_project().is_none()),
        "no Project tab opens on the way"
    );
}

/// Mockup row 206: the project row's Design Checklist opens that project's
/// Design Checklist tab, through the catalog command.
#[test]
fn the_design_checklist_row_opens_the_projects_checklist_tab() {
    let mut app = app_with_tree();
    let id = app.document_state.projects[0].id;
    // Who the user is, so opening the tab asks nothing of git.
    app.ui_state.validation_panels.identity = crate::panels::UserIdentity::Known("j.berg".into());
    let rows = rows_at(&app, &[0]);
    let row = item(&rows, "Design Checklist");
    assert!(!row.disabled);
    assert!(row.icon.is_some());
    let press = row.on_press.clone().expect("a live row");
    let _ = app.update(press);
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert!(matches!(tab.kind, crate::app::TabKind::DesignChecklist(p) if p == id));
}

/// The Planned flyout lists the four planned project features, each greyed
/// with the "planned" pill and publishing nothing.
#[test]
fn the_planned_flyout_lists_the_planned_rows_greyed_with_the_pill() {
    let rows = planned_entries(signex_i18n::I18n::en_us_ref(), TID);
    assert_eq!(
        layout(&rows),
        [
            "Make Project Available Online...",
            "Project Packager...",
            "Project Releaser...",
            "Share...",
        ]
    );
    for row in &rows {
        let DropdownEntry::Item(it) = row else {
            panic!("only item rows");
        };
        assert!(it.disabled && it.on_press.is_none(), "{:?}", it.label);
        assert_eq!(badge_of(it), Some("planned"), "{:?}", it.label);
        assert!(it.icon.is_some(), "{:?}", it.label);
    }
}

/// A document leaf: Open (strong) first, Remove from Project... last in the
/// danger ink, live, opening the Remove dialog.
#[test]
fn the_document_leaf_menu_ends_with_a_live_danger_remove_row() {
    let app = app_with_tree();
    let key = key_of(&app.document_view.panel_ctx.project_tree, &[0, 0, 0]).expect("the leaf");
    let rows = rows_at(&app, &[0, 0, 0]);
    assert_eq!(
        layout(&rows),
        [
            "Open",
            "Explore",
            "Rename...",
            SEP,
            "Print...",
            "Show Differences...",
            "Show History",
            SEP,
            "Remove from Project...",
        ]
    );
    let open = item(&rows, "Open");
    assert!(open.strong && !open.danger);
    assert!(matches!(
        &open.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::OpenNode(k)
        ))) if *k == key
    ));
    let remove = item(&rows, "Remove from Project...");
    assert!(remove.danger && !remove.strong && !remove.disabled);
    assert!(matches!(
        &remove.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::OpenRemoveDialog(k)
        ))) if *k == key
    ));
    let diff = item(&rows, "Show Differences...");
    assert!(diff.disabled && diff.on_press.is_none());
    assert_eq!(badge_of(diff), Some("planned"));
    // Only one strong row and one danger row.
    let flagged = |f: fn(&DropdownItem<Message>) -> bool| {
        rows.iter()
            .filter(|row| matches!(row, DropdownEntry::Item(it) if f(it)))
            .count()
    };
    assert_eq!(flagged(|it| it.strong), 1);
    assert_eq!(flagged(|it| it.danger), 1);
}

/// Mockup row 20: Show History is a live row with an icon and no pill; it
/// names the clicked leaf.
#[test]
fn the_document_leaf_show_history_row_is_live() {
    let app = app_with_tree();
    let key = key_of(&app.document_view.panel_ctx.project_tree, &[0, 0, 0]).expect("the leaf");
    let rows = rows_at(&app, &[0, 0, 0]);
    let row = item(&rows, "Show History");
    assert!(!row.disabled && !row.strong && !row.danger);
    assert!(row.icon.is_some());
    assert_eq!(badge_of(row), None);
    assert!(matches!(
        &row.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::ShowHistory(k)
        ))) if *k == key
    ));
}

/// Mockup row 20: Show History opens the History panel filtered to the file
/// (This file only on); a second pick keeps the filter on, it is no toggle.
/// The leaf is already open, so the panel follows it at once; a leaf read on
/// a worker is covered in `project_navigation::tree_history_tests`.
#[test]
fn show_history_opens_the_history_panel_with_this_file_only() {
    use crate::app::handlers::menu::export::tests::{open_with, sheet_with_net};
    use crate::app::{TabInfo, TabKind};
    use crate::panels::PanelKind;
    let mut app = app_with_tree();
    let leaf = std::path::PathBuf::from("/p/a.snxsch");
    let sheet = sheet_with_net("R1", "N", &[]);
    open_with(&mut app.document_state, &leaf, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "a".to_string(),
        path: leaf.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    assert_eq!(app.history_target_path(), Some(leaf));
    let _ = app.document_view.dock.close_panel(PanelKind::History);
    assert!(!app.is_panel_open(PanelKind::History));
    let rows = rows_at(&app, &[0, 0, 0]);
    let press = item(&rows, "Show History")
        .on_press
        .clone()
        .expect("a live row");
    let _ = app.update(press.clone());
    assert!(app.is_panel_open(PanelKind::History));
    assert!(app.document_view.history.ui.filter.this_file_only);
    let shown = &app.document_view.panel_ctx.history;
    assert!(shown.ui.filter.this_file_only, "the panel reads it");
    let _ = app.update(press);
    assert!(app.document_view.history.ui.filter.this_file_only);
}

/// Mockup row 20: Show History runs on the main window like the other leaf
/// rows and names its row, so a rebuilt tree cannot retarget it.
#[test]
fn show_history_targets_its_row_on_the_main_window() {
    let key: signex_widgets::tree_view::TreeKey = vec!["proj".into(), "a.snxsch".into()];
    let action = ProjectTreeAction::ShowHistory(key.clone());
    assert_eq!(action.row_key(), Some(&key));
    assert!(!action.changes_only_the_shown_tree());
}

/// A library: one Add New launcher instead of three flat rows, then
/// pricing, the tree toggles, and Remove from Project... in the danger ink.
#[test]
fn the_library_menu_has_one_add_new_launcher() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0, 1, 0]);
    // Power.snxlib is a collapsed leaf.
    assert_eq!(
        layout(&rows),
        [
            "Add New",
            SEP,
            "Refresh All Pricing",
            SEP,
            "Expand",
            "Refresh",
            SEP,
            "Remove from Project...",
        ]
    );
    let add_new = item(&rows, "Add New");
    assert!(add_new.submenu && !add_new.disabled);
    assert!(matches!(
        add_new.on_hover,
        Some((
            Message::ContextMenu(ContextMenuMsg::SubmenuHover(
                ContextSubmenu::AddNewToLibrary
            )),
            _,
        ))
    ));
    assert!(item(&rows, "Remove from Project...").danger);
}

/// The library's Add New flyout runs the same menu messages the three flat
/// rows ran.
#[test]
fn the_library_add_new_flyout_creates_component_symbol_and_footprint() {
    let rows = library_add_new_entries(signex_i18n::I18n::en_us_ref(), TID, inks());
    let mut expected = vec!["Component", "Symbol Library"];
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        expected.push("Footprint Library");
    }
    assert_eq!(layout(&rows), expected);
    assert!(matches!(
        item(&rows, "Component").on_press,
        Some(Message::Menu(MenuMessage::AddLibraryComponent))
    ));
    assert!(matches!(
        item(&rows, "Symbol Library").on_press,
        Some(Message::Menu(MenuMessage::AddLibrarySymbol))
    ));
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        assert!(matches!(
            item(&rows, "Footprint Library").on_press,
            Some(Message::Menu(MenuMessage::AddLibraryFootprint))
        ));
    }
}

/// Add New to Project: the documents, the libraries, then the project
/// files: Design Notebook live (mockup row 195), Export Job, Constraint File
/// and VHDL File kept as planned rows.
#[test]
fn the_add_new_to_project_flyout_is_grouped_documents_libraries_planned() {
    let rows = add_new_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        inks(),
        vec!["proj".to_string()],
    );
    let mut expected = vec![
        "Schematic",
        "PCB",
        SEP,
        "Component Library",
        "Symbol Library",
    ];
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        expected.push("PCB Library");
    }
    expected.extend([
        SEP,
        "Design Notebook",
        "Export Job",
        SEP,
        "Constraint File",
        "VHDL File",
    ]);
    assert_eq!(layout(&rows), expected);
    let notebook = item(&rows, "Design Notebook");
    assert!(!notebook.disabled);
    assert_eq!(badge_of(notebook), None);
    assert!(matches!(
        &notebook.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::AddNewDesignNotebook(key)
        ))) if key == &vec!["proj".to_string()]
    ));
    for planned in ["Export Job", "Constraint File", "VHDL File"] {
        let it = item(&rows, planned);
        assert!(it.disabled && it.on_press.is_none(), "{planned}");
        assert_eq!(badge_of(it), Some("planned"), "{planned}");
    }
}

/// One icon column: a row without a glyph starts its label where a row
/// with one does.
#[test]
fn a_row_without_an_icon_lines_its_label_up_with_the_others() {
    let app = app_with_tree();
    let key = key_of(&app.document_view.panel_ctx.project_tree, &[0]).expect("the root");
    let ctx = ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(key),
        add_new_only: false,
        window: None,
    };
    let card = app.view_project_tree_context_menu(&ctx);
    let texts = crate::layout_probe::lay_out_texts(card.element, iced::Size::new(1000.0, 900.0));
    let bare = crate::layout_probe::find(&texts, "Close Project Documents").bounds;
    let with_icon = crate::layout_probe::find(&texts, "Close Project").bounds;
    let planned = crate::layout_probe::find(&texts, "Planned").bounds;
    assert!((bare.x - with_icon.x).abs() < 0.5, "{bare:?} {with_icon:?}");
    assert!(
        (planned.x - with_icon.x).abs() < 0.5,
        "{planned:?} {with_icon:?}"
    );
}
