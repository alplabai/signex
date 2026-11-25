//! Mockup rows 35 and 39: one switch for both surfaces, Save and switch, and
//! what a landed switch does to open documents.

use std::path::PathBuf;

use signex_library::adapters::local_git::project_repo::SwitchRefusal;
use signex_library::repo_branches::BranchTarget;
use signex_types::schematic::SchematicSheet;

use super::{ProjectGitMsg, SwitchBlock, SwitchDone, SwitchRequest, SwitchSurface};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectSection, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, TitleBlockField};

fn app() -> (Signex, ProjectId, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-switch-{}", uuid::Uuid::now_v7()));
    let app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    let id = app.document_state.projects[0].id;
    let root = app.document_state.projects[0].dir().to_path_buf();
    (app, id, root)
}

fn request(id: ProjectId, root: &std::path::Path) -> SwitchRequest {
    SwitchRequest {
        surface: SwitchSurface::ProjectTab(id),
        root: root.to_path_buf(),
        target: BranchTarget::Local("other".into()),
    }
}

fn send(app: &mut Signex, msg: ProjectGitMsg) {
    let _ = app.update(Message::from(msg));
}

/// Both surfaces read one block: a commit running blocks the History panel
/// the way it blocks the Project tab.
#[test]
fn both_surfaces_share_one_block() {
    let (mut app, id, root) = app();
    app.document_state
        .inflight_git_commits
        .insert((root.clone(), "a.snxsch".into()));
    let _ = app.request_branch_switch(request(id, &root));
    let _ = app.request_branch_switch(SwitchRequest {
        surface: SwitchSurface::History,
        ..request(id, &root)
    });
    assert_eq!(
        app.ui_state.project_git.repo(id).unwrap().block,
        Some(SwitchBlock::WorkRunning)
    );
    assert_eq!(
        app.document_state.history.ui.switch_block,
        Some(SwitchBlock::WorkRunning)
    );
}

#[test]
fn a_differing_project_file_with_edits_offers_save_and_switch() {
    let (mut app, id, root) = app();
    let sheet = root.join("a.snxsch");
    app.document_state.mark_document_dirty(&sheet);
    send(
        &mut app,
        ProjectGitMsg::SwitchChecked {
            request: Box::new(request(id, &root)),
            pick: 0,
            result: Ok(true),
        },
    );
    assert!(app.ui_state.project_git.offers_save_and_switch(id));
}

/// The same project file: the plain notice stays, nothing is offered.
#[test]
fn the_same_project_file_offers_nothing() {
    let (mut app, id, root) = app();
    app.document_state
        .mark_document_dirty(&root.join("a.snxsch"));
    send(
        &mut app,
        ProjectGitMsg::SwitchChecked {
            request: Box::new(request(id, &root)),
            pick: 0,
            result: Ok(false),
        },
    );
    assert!(!app.ui_state.project_git.offers_save_and_switch(id));
}

/// Edits saved while the read ran leave nothing to ask about.
#[test]
fn no_offer_once_the_edits_are_gone() {
    let (mut app, id, root) = app();
    send(
        &mut app,
        ProjectGitMsg::SwitchChecked {
            request: Box::new(request(id, &root)),
            pick: 0,
            result: Ok(true),
        },
    );
    assert!(!app.ui_state.project_git.offers_save_and_switch(id));
}

/// Signex saves only on the press, and the switch is what follows the save.
#[test]
fn save_and_switch_saves_first_and_starts_no_switch() {
    let (mut app, id, root) = app();
    let sheet = root.join("a.snxsch");
    app.document_state.mark_document_dirty(&sheet);
    app.ui_state.project_git.save_offer = Some(request(id, &root));

    let _ = app.save_then_switch();

    assert!(app.ui_state.project_git.save_offer.is_none());
    assert!(
        app.document_state.branch_switch.is_none(),
        "the switch waits for the save"
    );
}

/// The save's commit still runs: the switch waits, then the tick starts it.
#[test]
fn a_switch_waits_for_the_commit_of_its_save() {
    let (mut app, id, root) = app();
    let commit = (root.clone(), PathBuf::from("a.snxsch"));
    app.document_state
        .inflight_git_commits
        .insert(commit.clone());

    let _ = app.resume_branch_switch(request(id, &root));
    assert!(app.document_state.branch_switch.is_none());
    assert!(app.ui_state.project_git.parked.is_some());

    app.document_state.inflight_git_commits.remove(&commit);
    let _ = app.resume_parked_branch_switch();
    assert!(app.ui_state.project_git.parked.is_none());
    assert_eq!(app.document_state.branch_switch, Some(root));
}

/// A refusal changes nothing: the guard is released and the project stays.
#[test]
fn a_refused_switch_keeps_the_project_open() {
    let (mut app, id, root) = app();
    app.document_state.branch_switch = Some(root);
    let _ = app.finish_branch_switch(
        SwitchSurface::ProjectTab(id),
        Err(SwitchRefusal::ProjectFileUnreadable("p.snxprj".into())),
    );
    assert!(app.document_state.branch_switch.is_none());
    assert!(app.document_state.project_by_id(id).is_some());
}

/// Until the project has reopened, saves of its files stay refused.
#[test]
fn the_guard_holds_while_the_project_reopens() {
    let (mut app, id, root) = app();
    let file = app.document_state.projects[0]
        .path
        .file_name()
        .map(PathBuf::from)
        .unwrap();
    app.document_state.branch_switch = Some(root.clone());
    let done = SwitchDone {
        root: root.clone(),
        branch: "other".into(),
        changed: vec![file.clone()],
        reopen: vec![file],
    };
    let _ = app.finish_branch_switch(SwitchSurface::ProjectTab(id), Ok(done));
    assert_eq!(app.document_state.branch_switch, Some(root));
    assert!(app.document_state.project_by_id(id).is_some());
}

/// A landed switch with no project file change releases the guard and
/// closes the switcher.
#[test]
fn a_landed_switch_releases_the_guard() {
    let (mut app, id, root) = app();
    send(&mut app, ProjectGitMsg::SwitcherToggled { project_id: id });
    app.document_state.branch_switch = Some(root.clone());
    let done = SwitchDone {
        root,
        branch: "other".into(),
        changed: vec![PathBuf::from("a.snxsch")],
        reopen: Vec::new(),
    };
    let _ = app.finish_branch_switch(SwitchSurface::ProjectTab(id), Ok(done));
    assert!(app.document_state.branch_switch.is_none());
    assert!(!app.ui_state.project_git.is_switcher_open(id));
}

/// A project on disk, its Project tab on Version control, mid-switch.
fn reopening() -> (Signex, ProjectId, PathBuf, SwitchDone) {
    let (mut app, id, root) = app();
    let path = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&path, "").unwrap();
    app.open_project_tab(0);
    app.document_state
        .project_tabs
        .get_mut(&id)
        .unwrap()
        .section = ProjectSection::VersionControl;
    app.document_state.branch_switch = Some(root.clone());
    let rel = PathBuf::from(path.file_name().unwrap());
    let done = SwitchDone {
        root: root.clone(),
        branch: "other".into(),
        changed: vec![rel.clone()],
        reopen: vec![rel],
    };
    (app, id, root, done)
}

/// Decision 2026-11-22: a differing project file closes and reopens the
/// project, and the Project tab comes back on its section.
#[test]
fn a_changed_project_file_reopens_the_project_with_its_tab() {
    let (mut app, old, root, done) = reopening();
    let refs = app.resolve_sheet_refs_blocking().unwrap();

    let _ = app.finish_project_reopen(Box::new(done), Ok(refs));

    assert!(app.document_state.project_by_id(old).is_none());
    assert_eq!(app.document_state.projects.len(), 1);
    let new = app.document_state.projects[0].id;
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|t| t.kind.as_project() == Some(new))
    );
    assert_eq!(
        app.document_state.project_tabs[&new].section,
        ProjectSection::VersionControl
    );
    assert!(app.document_state.branch_switch.is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// Data-loss guard: a `.snxprj` edited during the switch is not closed.
#[test]
fn a_project_edited_during_the_switch_stays_open_and_says_so() {
    let (mut app, id, root, done) = reopening();
    let path = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&path);
    let refs = app.resolve_sheet_refs_blocking().unwrap();

    let _ = app.finish_project_reopen(Box::new(done), Ok(refs));

    assert!(app.document_state.project_by_id(id).is_some());
    assert!(app.document_state.is_document_dirty(&path));
    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.branch_switch.is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// A project at `<tmp>/proj` that lists `../outside.snxsch`, open with unsaved
/// edits, mid-switch to a branch with another project file.
fn reopening_with_an_outside_sheet() -> (Signex, ProjectId, PathBuf, PathBuf, SwitchDone) {
    let top = std::env::temp_dir().join(format!("signex-switch-{}", uuid::Uuid::now_v7()));
    let root = top.join("proj");
    let mut app = app_workspace(&root.to_string_lossy(), &["a.snxsch", "../outside.snxsch"]);
    let id = app.document_state.projects[0].id;
    let path = app.document_state.projects[0].path.clone();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&path, "").unwrap();
    let outside = top.join("outside.snxsch");
    open_with(&mut app.document_state, &outside, SchematicSheet::default());
    app.document_state.mark_document_dirty(&outside);
    app.document_state.branch_switch = Some(root.clone());
    let rel = PathBuf::from(path.file_name().unwrap());
    let done = SwitchDone {
        root,
        branch: "other".into(),
        changed: vec![rel.clone()],
        reopen: vec![rel],
    };
    (app, id, top, outside, done)
}

/// Data-loss guard: the close the reopen runs would remove this engine, and
/// no prompt would have asked.
#[test]
fn an_unsaved_sheet_outside_the_project_folder_holds_the_reopen() {
    let (mut app, id, top, outside, done) = reopening_with_an_outside_sheet();
    let refs = app.resolve_sheet_refs_blocking().unwrap();

    let _ = app.finish_project_reopen(Box::new(done), Ok(refs));

    assert!(app.document_state.project_by_id(id).is_some());
    assert!(app.document_state.engines.contains_key(&outside));
    assert!(app.document_state.is_document_dirty(&outside));
    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.branch_switch.is_none());
    let _ = std::fs::remove_dir_all(top);
}

/// The same sheet stops the switch before anything moves.
#[test]
fn an_unsaved_sheet_the_project_reaches_from_outside_blocks_the_switch() {
    let (mut app, _id, top, _outside, done) = reopening_with_an_outside_sheet();
    app.document_state.branch_switch = None;
    assert!(matches!(
        app.branch_switch_block(&done.root),
        Some(SwitchBlock::UnsavedEdits(name)) if name == "outside.snxsch"
    ));
    let _ = std::fs::remove_dir_all(top);
}

/// A field typed and not committed when the switch lands is an edit: it
/// holds the reopen instead of going with the closed tab.
#[test]
fn a_title_block_draft_typed_during_the_switch_holds_the_reopen() {
    let (mut app, id, root, done) = reopening();
    let sheet = root.join("a.snxsch");
    open_with(&mut app.document_state, &sheet, SchematicSheet::default());
    app.document_state.tabs.push(TabInfo {
        title: "a".into(),
        path: sheet.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(sheet.clone());
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::TitleBlockTyping(TitleBlockField::DrawnBy, "X".to_string()),
    )));

    // The commit can make the first read stale; the second acts on it.
    let refs = app.resolve_sheet_refs_blocking().unwrap();
    let _ = app.finish_project_reopen(Box::new(done.clone()), Ok(refs));
    let refs = app.resolve_sheet_refs_blocking().unwrap();
    let _ = app.finish_project_reopen(Box::new(done), Ok(refs));

    assert!(app.document_state.project_by_id(id).is_some());
    assert!(app.document_state.is_document_dirty(&sheet));
    let drawn_by = app.document_state.engines[&sheet]
        .document()
        .title_block
        .get("drawn_by")
        .cloned();
    assert_eq!(drawn_by.as_deref(), Some("X"));
    let _ = std::fs::remove_dir_all(root);
}

/// The Rules tab is one of the tabs the project had.
#[test]
fn a_clean_project_still_reopens_with_its_rules_tab() {
    let (mut app, old, root, done) = reopening();
    app.open_rules_tab(old);
    let refs = app.resolve_sheet_refs_blocking().unwrap();

    let _ = app.finish_project_reopen(Box::new(done), Ok(refs));

    let new = app.document_state.projects[0].id;
    assert_ne!(new, old);
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|tab| tab.kind.as_rules() == Some(new))
    );
    let _ = std::fs::remove_dir_all(root);
}

/// A Layer Stack tab comes back once its board has reopened.
#[test]
fn a_reopened_board_gets_its_layer_stack_tab_back() {
    let (mut app, _id, root) = app();
    let board = root.join("b.snxpcb");
    app.ui_state
        .project_git
        .reopened_stacks
        .insert(board.clone(), true);
    app.document_state.tabs.push(TabInfo {
        title: "b".into(),
        path: board.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Pcb,
    });

    let takes_focus = app.ui_state.project_git.take_reopened_stack(&board);
    assert_eq!(takes_focus, Some(true));
    app.restore_layer_stack_tab(board.clone(), true);

    let active = &app.document_state.tabs[app.document_state.active_tab];
    assert!(active.kind.is_layer_stack());
    assert_eq!(active.path, board);
    assert_eq!(app.ui_state.project_git.take_reopened_stack(&board), None);
}

/// Picking again replaces a switch still waiting for its commit.
#[test]
fn a_new_pick_cancels_a_parked_switch() {
    let (mut app, id, root) = app();
    app.ui_state.project_git.parked = Some(request(id, &root));
    app.document_state
        .inflight_git_commits
        .insert((root.clone(), "a.snxsch".into()));

    let _ = app.request_branch_switch(SwitchRequest {
        target: BranchTarget::Local("third".into()),
        ..request(id, &root)
    });

    assert!(app.ui_state.project_git.parked.is_none());
}

/// Closing the project takes its waiting History switch with it: the tick
/// must not check out a repository nothing has open.
#[test]
fn closing_the_project_cancels_its_parked_history_switch() {
    let (mut app, id, root) = app();
    app.ui_state.project_git.parked = Some(SwitchRequest {
        surface: SwitchSurface::History,
        ..request(id, &root)
    });
    let refs = app.resolve_sheet_refs_blocking().unwrap();

    let _ = app.execute_close_project(id, &refs, &[]);

    assert!(app.ui_state.project_git.parked.is_none());
}

/// Another open project in the repository keeps a History switch valid.
#[test]
fn a_parked_history_switch_outlives_a_sibling_project() {
    let (mut app, id, root) = app();
    let mut sibling = app_workspace(&root.join("sub").to_string_lossy(), &["b.snxsch"]);
    let mut other = sibling.document_state.projects.remove(0);
    other.id = app.document_state.mint_project_id();
    app.document_state.projects.push(other);
    app.ui_state.project_git.parked = Some(SwitchRequest {
        surface: SwitchSurface::History,
        ..request(id, &root)
    });

    app.forget_switch_requests_of(id);

    assert!(app.ui_state.project_git.parked.is_some());
}

/// Two picks read at once: the first one's late answer offers nothing.
#[test]
fn the_read_of_an_earlier_pick_offers_nothing() {
    let (mut app, id, root) = app();
    app.document_state
        .mark_document_dirty(&root.join("a.snxsch"));
    let earlier = app.ui_state.project_git.pick;
    let _ = app.request_branch_switch(SwitchRequest {
        target: BranchTarget::Local("third".into()),
        ..request(id, &root)
    });
    send(
        &mut app,
        ProjectGitMsg::SwitchChecked {
            request: Box::new(request(id, &root)),
            pick: earlier,
            result: Ok(true),
        },
    );
    assert!(!app.ui_state.project_git.offers_save_and_switch(id));
}

/// A Layer Stack tab shares its board's path but holds no document, so a
/// clean board stays free to change with the branch.
#[test]
fn a_layer_stack_tab_does_not_hold_its_board() {
    let (mut app, _id, root) = app();
    app.document_state.tabs.push(TabInfo {
        title: "stack".into(),
        path: root.join("b.snxpcb"),
        cached_document: None,
        project_id: None,
        kind: TabKind::LayerStack,
    });
    assert!(
        !app.branch_switch_held_paths(&root)
            .contains(&PathBuf::from("b.snxpcb"))
    );
}

/// The offer names no branch, so reopening the switcher drops it.
#[test]
fn toggling_the_switcher_drops_the_offer() {
    let (mut app, id, root) = app();
    app.ui_state.project_git.save_offer = Some(request(id, &root));
    send(&mut app, ProjectGitMsg::SwitcherToggled { project_id: id });
    assert!(!app.ui_state.project_git.offers_save_and_switch(id));
}
