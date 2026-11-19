//! The document tab menu's new rows (m5) and the "+N" overflow menu, driven
//! through `Signex::update`: Close Tabs to the Right, Copy Path, Reveal in
//! Projects and a pick from the hidden-tabs menu.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_library::{Footprint, FootprintFile};
use signex_widgets::tree_view::{TreeIcon, TreeNode, get_node, resolve_path};

use crate::app::{
    CaseFold, ContextMenuMsg, FootprintDocument, LoadedProject, Message, ProjectId, Signex,
    TabContextAction, TabInfo, TabKind,
};
use crate::tab_bar::TabMessage;

fn push_tab(app: &mut Signex, path: &Path) {
    app.document_state.tabs.push(TabInfo {
        title: path.display().to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    let file = FootprintFile::from_footprint(Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        FootprintDocument::new(path.to_path_buf(), file),
    );
}

/// An app with footprint tabs `names`, the tab at `active` active.
fn app_with_tabs(names: &[&str], active: usize) -> Signex {
    let (mut app, _boot) = Signex::new();
    for name in names {
        push_tab(&mut app, Path::new(name));
    }
    app.document_state.active_tab = active;
    app
}

fn tab_names(app: &Signex) -> Vec<String> {
    app.document_state
        .tabs
        .iter()
        .map(|tab| tab.path.display().to_string())
        .collect()
}

fn tab_action(app: &mut Signex, action: TabContextAction) -> iced::Task<Message> {
    app.update(Message::ContextMenu(ContextMenuMsg::TabAction(action)))
}

fn main_tab_message(app: &mut Signex, msg: TabMessage) {
    let window_id = app.ui_state.main_window_id.expect("main window");
    let _ = app.update(Message::Tab { window_id, msg });
}

// ─── Close Tabs to the Right ──────────────────────────────────────

#[test]
fn close_tabs_to_the_right_closes_only_the_tabs_after_the_clicked_one() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt", "c.snxfpt", "d.snxfpt"], 0);
    let _ = tab_action(&mut app, TabContextAction::CloseToTheRight(1));
    assert_eq!(tab_names(&app), ["a.snxfpt", "b.snxfpt"]);
    assert_eq!(
        app.document_state.active_tab, 0,
        "the active tab a stays active"
    );
    assert!(app.interaction_state.tab_context_menu.is_none());
}

#[test]
fn close_tabs_to_the_right_of_the_last_tab_closes_nothing() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt", "c.snxfpt"], 2);
    let _ = tab_action(&mut app, TabContextAction::CloseToTheRight(2));
    assert_eq!(tab_names(&app), ["a.snxfpt", "b.snxfpt", "c.snxfpt"]);
    assert_eq!(app.document_state.active_tab, 2);
}

#[test]
fn closing_the_active_tab_to_the_right_leaves_a_valid_active_tab() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt", "c.snxfpt", "d.snxfpt"], 3);
    let _ = tab_action(&mut app, TabContextAction::CloseToTheRight(0));
    assert_eq!(tab_names(&app), ["a.snxfpt"]);
    assert_eq!(app.document_state.active_tab, 0);
}

#[test]
fn close_tabs_to_the_right_parks_a_dirty_tab_and_drops_a_clean_one() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt", "c.snxfpt"], 0);
    let dirty = PathBuf::from("b.snxfpt");
    app.document_state
        .footprint_editors
        .get_mut(&dirty)
        .expect("editor b")
        .set_dirty(true);
    let _ = tab_action(&mut app, TabContextAction::CloseToTheRight(0));
    assert_eq!(tab_names(&app), ["a.snxfpt"]);
    assert!(
        app.document_state.footprint_editors.contains_key(&dirty),
        "the dirty draft is parked, not dropped"
    );
    assert!(app.document_state.is_document_dirty(&dirty));
    assert!(
        !app.document_state
            .footprint_editors
            .contains_key(Path::new("c.snxfpt")),
        "a clean tab's editor goes with it"
    );
}

// ─── Copy Path ────────────────────────────────────────────────────

#[test]
fn copy_path_writes_the_clipboard_for_a_file_tab_only() {
    let mut app = app_with_tabs(&["a.snxfpt"], 0);
    // A tab with no file behind it has nothing to copy.
    push_tab(&mut app, Path::new(""));
    let copied = tab_action(&mut app, TabContextAction::CopyPath(0));
    assert!(
        copied.units() > 0,
        "a file tab's path goes to the clipboard"
    );
    assert!(app.interaction_state.tab_context_menu.is_none());
    let nothing = tab_action(&mut app, TabContextAction::CopyPath(1));
    assert_eq!(nothing.units(), 0);
    let out_of_range = tab_action(&mut app, TabContextAction::CopyPath(9));
    assert_eq!(out_of_range.units(), 0);
}

// ─── Reveal in Projects ───────────────────────────────────────────

fn project_at(dir: &str, case_fold: CaseFold) -> LoadedProject {
    LoadedProject {
        id: ProjectId::test_id(1),
        path: Path::new(dir).join("proj.snxprj"),
        data: signex_types::project::ProjectData {
            document_id: None,
            description: None,
            name: "proj".to_string(),
            dir: dir.to_string(),
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
            agent_journal: Default::default(),
            fab_outputs: None,
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold,
        case_probed: true,
        case_decided: true,
    }
}

/// One project at `/w/proj` whose Source Documents list `a.snxsch` and
/// `b.snxsch`, every folder collapsed, and a tab on `tab`.
fn app_with_project_tree(tab: &str, case_fold: CaseFold) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.document_state.projects = vec![project_at("/w/proj", case_fold)];
    let mut tree = vec![TreeNode::branch(
        "proj",
        TreeIcon::Folder,
        vec![TreeNode::branch(
            "Source Documents",
            TreeIcon::Folder,
            vec![
                TreeNode::leaf("a.snxsch", TreeIcon::SnxSchematic),
                TreeNode::leaf("b.snxsch", TreeIcon::SnxSchematic),
            ],
        )],
    )];
    tree[0].expanded = false;
    tree[0].children[0].expanded = false;
    app.document_state.panel_ctx.project_tree = tree;
    push_tab(&mut app, Path::new(tab));
    app
}

// Called on the handler, not through `update`: `finish_update` rebuilds the
// hand-made tree from the (sheet-less) project, which is not under test.
#[test]
fn reveal_in_projects_opens_the_folders_above_the_row_and_selects_it() {
    let mut app = app_with_project_tree("/w/proj/b.snxsch", CaseFold::PRESERVE);
    let key = app
        .project_tree_key_of_file(Path::new("/w/proj/b.snxsch"))
        .expect("the project lists b");
    let tree = app.document_state.panel_ctx.project_tree.as_slice();
    let row = resolve_path(tree, &key).expect("key resolves");
    assert_eq!(
        get_node(tree, &row).map(|n| n.label.as_str()),
        Some("b.snxsch")
    );

    let task = app.handle_tab_context_action(TabContextAction::RevealInProjects(0));
    let tree = &app.document_state.panel_ctx.project_tree;
    assert!(tree[0].expanded && tree[0].children[0].expanded);
    assert_eq!(task.units(), 2, "show Projects, then the tree's own Select");
}

#[test]
fn reveal_in_projects_does_nothing_for_a_file_no_project_lists() {
    let mut app = app_with_project_tree("/w/elsewhere/b.snxsch", CaseFold::PRESERVE);
    let task = app.handle_tab_context_action(TabContextAction::RevealInProjects(0));
    assert_eq!(task.units(), 0);
    assert!(!app.document_state.panel_ctx.project_tree[0].expanded);
}

/// The tab and the tree name one file under the project volume's case rule,
/// as saving does: a case-folding volume matches `B.snxsch` to `b.snxsch`,
/// a case-preserving one does not.
#[test]
fn reveal_in_projects_compares_paths_under_the_project_case_rule() {
    let mut folded = app_with_project_tree("/w/proj/B.snxsch", CaseFold::from_probe(true));
    let task = folded.handle_tab_context_action(TabContextAction::RevealInProjects(0));
    assert_eq!(task.units(), 2);

    let mut preserved = app_with_project_tree("/w/proj/B.snxsch", CaseFold::PRESERVE);
    let task = preserved.handle_tab_context_action(TabContextAction::RevealInProjects(0));
    assert_eq!(task.units(), 0);
}

// ─── "+N" overflow menu ───────────────────────────────────────────

#[test]
fn picking_a_hidden_tab_makes_it_active_and_closes_the_menu() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt", "c.snxfpt", "d.snxfpt"], 0);
    main_tab_message(&mut app, TabMessage::OverflowMenuToggled);
    assert!(app.ui_state.tab_overflow_menu_open);
    main_tab_message(&mut app, TabMessage::OverflowTabPicked(3));
    assert_eq!(app.document_state.active_tab, 3);
    assert!(!app.ui_state.tab_overflow_menu_open);
}

#[test]
fn a_pick_past_the_last_tab_only_closes_the_menu() {
    let mut app = app_with_tabs(&["a.snxfpt", "b.snxfpt"], 1);
    main_tab_message(&mut app, TabMessage::OverflowMenuToggled);
    main_tab_message(&mut app, TabMessage::OverflowTabPicked(7));
    assert_eq!(app.document_state.active_tab, 1);
    assert!(!app.ui_state.tab_overflow_menu_open);
}
