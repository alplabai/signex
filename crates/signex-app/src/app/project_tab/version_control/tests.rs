//! Mockup row 35: the Version control section's repository pill,
//! the Commit every save switch rule and Open History panel.

use signex_i18n::ids;

use super::view::{open_history_message, repository_glyph, repository_pill};
use super::{Repository, may_toggle_commit_on_save};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::state::project_fs::git_marker;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::panels::PanelKind;
use crate::styles::PillTone;

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-vcs-look-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

#[test]
fn the_repository_pill_follows_the_last_probe() {
    assert_eq!(
        repository_pill(Repository::Present),
        (ids::PROJECT_VCS_STATE_PRESENT, PillTone::Ok)
    );
    assert_eq!(
        repository_pill(Repository::Absent),
        (ids::PROJECT_VCS_STATE_ABSENT, PillTone::Neutral)
    );
    assert_eq!(
        repository_pill(Repository::Unknown),
        (ids::PROJECT_VCS_STATE_CHECKING, PillTone::Neutral)
    );
    assert!(repository_glyph(Repository::Present).is_some());
    assert!(repository_glyph(Repository::Absent).is_none());
    assert!(repository_glyph(Repository::Unknown).is_none());
}

#[test]
fn the_switch_turns_on_only_with_a_repository_and_always_turns_off() {
    assert!(may_toggle_commit_on_save(false, Repository::Present));
    assert!(may_toggle_commit_on_save(false, Repository::Unknown));
    assert!(!may_toggle_commit_on_save(false, Repository::Absent));
    assert!(may_toggle_commit_on_save(true, Repository::Absent));
    assert!(may_toggle_commit_on_save(true, Repository::Present));
}

#[test]
fn open_history_panel_opens_the_history_panel() {
    let (mut app, _) = app();
    let _ = app.document_state.dock.close_panel(PanelKind::History);
    assert!(!app.is_panel_open(PanelKind::History));
    let _ = app.update(open_history_message());
    assert!(app.is_panel_open(PanelKind::History));
}

/// Mockup row 35: the last commit reads short id, subject and age.
#[test]
fn the_last_commit_reads_id_subject_and_age() {
    use signex_library::adapters::local_git::project_repo::LastCommit;
    let now = chrono::DateTime::from_timestamp(10_000, 0).unwrap();
    let commit = LastCommit {
        short_sha: "a3bbcc6".into(),
        subject: "Raise L1".into(),
        seconds: 10_000 - 12 * 60,
    };
    let i18n = signex_i18n::I18n::en_us();
    let words = super::view::last_commit_words(&commit, now, &i18n).into_string();
    assert!(words.contains("a3bbcc6"), "{words}");
    assert!(words.contains("Raise L1"), "{words}");
    assert!(words.contains("12"), "{words}");
}

/// Showing the section with the switcher open and a read in hand renders.
#[test]
fn the_section_renders_its_branch_switcher() {
    use crate::app::ProjectGitMsg;
    use signex_library::adapters::local_git::project_repo::{BranchFacts, RepoFacts};
    let (mut app, id) = app();
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: crate::app::ProjectSection::VersionControl,
        },
    )));
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .unwrap()
        .to_path_buf();
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), true);
    let facts = RepoFacts {
        branch: Some("main".into()),
        branches: vec![BranchFacts {
            name: "main".into(),
            is_current: true,
            ahead: 0,
            behind: 0,
        }],
        last_commit: None,
        author: None,
    };
    let generation = 1;
    let _ = app.update(Message::from(ProjectGitMsg::RepoReadRequested {
        project_id: id,
    }));
    let _ = app.update(Message::from(ProjectGitMsg::RepoRead {
        project_id: id,
        generation,
        result: Ok(facts),
    }));
    let _ = app.update(Message::from(ProjectGitMsg::SwitcherToggled {
        project_id: id,
    }));
    assert!(app.ui_state.project_git.is_switcher_open(id));
    let _ = app.view_project_tab(id);
}

#[test]
fn the_section_renders_with_and_without_a_repository() {
    let (mut app, id) = app();
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: crate::app::ProjectSection::VersionControl,
        },
    )));
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .unwrap()
        .to_path_buf();
    for present in [true, false] {
        app.document_state
            .project_fs
            .facts
            .set_exists(git_marker(&folder), present);
        let expected = if present {
            Repository::Present
        } else {
            Repository::Absent
        };
        let project = &app.document_state.projects[0];
        assert_eq!(app.version_control_facts(project).repository, expected);
        let _ = app.view_project_tab(id);
    }
}
