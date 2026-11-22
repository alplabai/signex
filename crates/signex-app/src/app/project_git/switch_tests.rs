//! Mockup rows 35 and 39: one switch for both surfaces, Save and switch, and
//! what a landed switch does to open documents.

use std::path::PathBuf;

use signex_library::adapters::local_git::project_repo::SwitchRefusal;
use signex_library::repo_branches::BranchTarget;

use super::{ProjectGitMsg, SwitchBlock, SwitchDone, SwitchRequest, SwitchSurface};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectSection, Signex};

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

/// The offer names no branch, so reopening the switcher drops it.
#[test]
fn toggling_the_switcher_drops_the_offer() {
    let (mut app, id, root) = app();
    app.ui_state.project_git.save_offer = Some(request(id, &root));
    send(&mut app, ProjectGitMsg::SwitcherToggled { project_id: id });
    assert!(!app.ui_state.project_git.offers_save_and_switch(id));
}
