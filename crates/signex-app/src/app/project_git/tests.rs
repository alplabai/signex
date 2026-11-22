//! Mockup rows 35 and 39: the switcher state, the refusals and the author.

use signex_i18n::I18n;
use signex_library::adapters::local_git::project_repo::{BranchFacts, RepoFacts, SwitchRefusal};

use super::view::{block_words, distance_words};
use super::{AuthorProblem, ProjectGitMsg, SwitchBlock};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-git-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn send(app: &mut Signex, msg: ProjectGitMsg) {
    let _ = app.update(Message::from(msg));
}

fn facts(branch: &str) -> RepoFacts {
    RepoFacts {
        branch: Some(branch.into()),
        branches: vec![BranchFacts {
            name: branch.into(),
            is_current: true,
            ahead: 0,
            behind: 0,
        }],
        last_commit: None,
        author: Some("J. Berg <j.berg@example.com>".into()),
    }
}

fn install(app: &mut Signex, id: ProjectId, facts: RepoFacts) {
    let generation = app.ui_state.project_git.repo_mut(id).generation;
    send(
        app,
        ProjectGitMsg::RepoRead {
            project_id: id,
            generation,
            result: Ok(facts),
        },
    );
}

#[test]
fn a_read_for_an_older_generation_is_dropped() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    let stale = app.ui_state.project_git.repo_mut(id).generation;
    app.ui_state.project_git.repo_mut(id).generation += 1;
    send(
        &mut app,
        ProjectGitMsg::RepoRead {
            project_id: id,
            generation: stale,
            result: Ok(facts("old")),
        },
    );
    let repo = app.ui_state.project_git.repo(id).unwrap();
    assert_eq!(repo.facts.as_ref().unwrap().branch.as_deref(), Some("main"));
}

/// Signex never saves for the user: unsaved edits refuse the switch and
/// name the file, and no switch starts.
#[test]
fn unsaved_edits_block_a_switch() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    let path = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&path);
    send(
        &mut app,
        ProjectGitMsg::BranchPicked {
            project_id: id,
            name: "other".into(),
        },
    );
    let repo = app.ui_state.project_git.repo(id).unwrap();
    assert!(repo.switching.is_none(), "no switch started");
    assert!(matches!(repo.block, Some(SwitchBlock::UnsavedEdits(_))));
}

#[test]
fn a_commit_still_running_blocks_a_switch() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    let root = std::path::PathBuf::from(&app.document_state.projects[0].data.dir);
    app.document_state
        .inflight_git_commits
        .insert((root, "a.snxsch".into()));
    send(
        &mut app,
        ProjectGitMsg::BranchPicked {
            project_id: id,
            name: "other".into(),
        },
    );
    let repo = app.ui_state.project_git.repo(id).unwrap();
    assert_eq!(repo.block, Some(SwitchBlock::WorkRunning));
}

#[test]
fn picking_the_current_branch_closes_the_switcher() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    send(&mut app, ProjectGitMsg::SwitcherToggled { project_id: id });
    assert!(app.ui_state.project_git.is_switcher_open(id));
    send(
        &mut app,
        ProjectGitMsg::BranchPicked {
            project_id: id,
            name: "main".into(),
        },
    );
    assert!(!app.ui_state.project_git.is_switcher_open(id));
    assert!(app.ui_state.project_git.repo(id).unwrap().switching.is_none());
}

#[test]
fn a_refused_switch_says_why_and_keeps_the_switcher_open() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    send(&mut app, ProjectGitMsg::SwitcherToggled { project_id: id });
    send(
        &mut app,
        ProjectGitMsg::BranchSwitched {
            project_id: id,
            name: "other".into(),
            result: Err(SwitchRefusal::WouldOverwrite),
        },
    );
    let git = &app.ui_state.project_git;
    assert!(git.is_switcher_open(id));
    assert_eq!(
        git.repo(id).unwrap().block,
        Some(SwitchBlock::Refused(SwitchRefusal::WouldOverwrite))
    );
}

#[test]
fn an_unreadable_author_is_refused_and_stays_typed() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    send(
        &mut app,
        ProjectGitMsg::AuthorEdited {
            project_id: id,
            text: "no email".into(),
        },
    );
    send(&mut app, ProjectGitMsg::AuthorSubmitted { project_id: id });
    let repo = app.ui_state.project_git.repo(id).unwrap();
    assert_eq!(repo.author_problem, Some(AuthorProblem::Unreadable));
    assert_eq!(repo.author_text(), "no email");
}

#[test]
fn the_author_field_shows_the_identity_git_resolves_until_typed_over() {
    let (mut app, id) = app();
    install(&mut app, id, facts("main"));
    let repo = app.ui_state.project_git.repo(id).unwrap();
    assert_eq!(repo.author_text(), "J. Berg <j.berg@example.com>");
}

#[test]
fn every_refusal_has_words() {
    let i18n = I18n::en_us();
    for block in [
        SwitchBlock::UnsavedEdits("a.snxsch".into()),
        SwitchBlock::WorkRunning,
        SwitchBlock::Refused(SwitchRefusal::UnknownBranch),
        SwitchBlock::Refused(SwitchRefusal::NoCommits),
        SwitchBlock::Refused(SwitchRefusal::ProjectFileDiffers),
        SwitchBlock::Refused(SwitchRefusal::LfsFileDiffers("m.step".into())),
        SwitchBlock::Refused(SwitchRefusal::WouldOverwrite),
        SwitchBlock::Refused(SwitchRefusal::Failed("why".into())),
    ] {
        assert!(!block_words(&block, &i18n).into_string().is_empty());
    }
}

#[test]
fn a_branch_reads_its_distance_from_the_current_one() {
    let i18n = I18n::en_us();
    let branch = |ahead, behind| BranchFacts {
        name: "b".into(),
        is_current: false,
        ahead,
        behind,
    };
    assert!(distance_words(&branch(0, 0), &i18n).is_none());
    let both = distance_words(&branch(5, 3), &i18n).unwrap().into_string();
    assert!(both.contains('5') && both.contains('3'), "{both}");
}
