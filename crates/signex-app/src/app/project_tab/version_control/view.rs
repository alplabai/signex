//! The Version control section's view (mockup row 35): a Repository
//! card with its state pill, then the Commit every save switch with its
//! notes. Pure: the project and its facts in, widgets out.

use iced::widget::{Column, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_library::adapters::local_git::project_repo::LastCommit;
use signex_types::theme::ThemeTokens;
use signex_widgets::{ToggleSwitchStyle, text, text_input, toggle_switch};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{
    ButtonKind, CardHeader, Glyph, card, kv_row, option_row, pill, text_button, warn_line,
};
use super::super::table::{SP, TS, section};
use super::{Repository, VersionControlFacts, may_toggle_commit_on_save};
use crate::app::project_git::{self, ProjectGitState, RepoView};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, OverlayMsg, ProjectGitMsg};
use crate::panels::PanelKind;
use crate::styles::tokens::{danger, on_accent, resolve};
use crate::styles::{PillTone, Radii, ti};

/// Width of the Author field (240 px).
const AUTHOR_W: f32 = TS.body * 20.0;

/// The words and tone of the repository pill in the Repository card header.
pub(super) fn repository_pill(repository: Repository) -> (MessageId, PillTone) {
    match repository {
        Repository::Present => (ids::PROJECT_VCS_STATE_PRESENT, PillTone::Ok),
        Repository::Absent => (ids::PROJECT_VCS_STATE_ABSENT, PillTone::Neutral),
        Repository::Unknown => (ids::PROJECT_VCS_STATE_CHECKING, PillTone::Neutral),
    }
}

/// The glyph before the repository pill's words: a check when present.
pub(super) fn repository_glyph(repository: Repository) -> Option<Glyph> {
    (repository == Repository::Present).then_some(Glyph::Check)
}

/// Open History panel: the History panel, opened or brought to the front.
pub(super) fn open_history_message() -> Message {
    Message::Overlay(OverlayMsg::OpenPanel(PanelKind::History))
}

/// What the section reads from the `project_git` slice, and the clock the
/// last commit's age is told from.
#[derive(Clone, Copy)]
pub(in crate::app::project_tab) struct GitView<'g> {
    pub state: &'g ProjectGitState,
    pub now: chrono::DateTime<chrono::Utc>,
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    facts: &VersionControlFacts,
    git: GitView<'_>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let body = vec![
        repository_card(project.id, facts.repository, git, i18n, tokens),
        commit_card(project, facts, git.state.repo(project.id), i18n, tokens),
    ];
    section(
        ids::PROJECT_SECTION_VERSION_CONTROL,
        ids::PROJECT_VCS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The last commit in one line: short id, subject and age.
pub(super) fn last_commit_words(
    commit: &LastCommit,
    now: chrono::DateTime<chrono::Utc>,
    i18n: &I18n,
) -> Localized {
    let time = chrono::DateTime::from_timestamp(commit.seconds, 0).unwrap_or(now);
    let age = crate::panels::history::format_relative_simple(i18n, time, now).into_string();
    t!(
        i18n,
        ids::PROJECT_VCS_LAST_COMMIT_LINE,
        "sha" => commit.short_sha.as_str(),
        "subject" => commit.subject.as_str(),
        "age" => age
    )
}

/// The Last commit value, or why the repository could not be read.
fn last_commit_value<'a>(
    repo: Option<&RepoView>,
    now: chrono::DateTime<chrono::Utc>,
    i18n: &I18n,
) -> Element<'a, Message> {
    let words = match repo {
        Some(RepoView {
            read_error: Some(why),
            ..
        }) => t!(i18n, ids::PROJECT_VCS_READ_FAILED, "why" => why.as_str()),
        Some(RepoView {
            facts: Some(facts), ..
        }) => match &facts.last_commit {
            Some(commit) => last_commit_words(commit, now, i18n),
            None => t!(i18n, ids::PROJECT_VCS_NO_COMMITS),
        },
        _ => t!(i18n, ids::PROJECT_VCS_STATE_CHECKING),
    };
    text(words)
        .size(TS.body)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .boxed()
}

/// The Branch value: the switcher's button and how many branches there are.
fn branch_value<'a>(
    project_id: ProjectId,
    repo: Option<&RepoView>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let button = project_git::view::branch_button(project_id, repo, i18n, tokens)
        .map(Message::from)
        .boxed();
    let count = repo
        .and_then(|repo| repo.facts.as_ref())
        .map_or(0, |facts| facts.branches.len());
    let note = text(t!(i18n, ids::PROJECT_VCS_BRANCH_COUNT, "count" => count))
        .size(TS.caption)
        .color(resolve(tokens).ink3)
        .boxed();
    row![button, note]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// A repository's rows: status, branch (with the open switcher), last
/// commit and Open History panel.
fn present_body<'a>(
    project_id: ProjectId,
    git: GitView<'_>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let repo = git.state.repo(project_id);
    let status = kv_row(
        t!(i18n, ids::PROJECT_VCS_STATUS),
        text(t!(i18n, ids::PROJECT_VCS_REPOSITORY_PRESENT))
            .size(TS.body)
            .boxed(),
        tokens,
    );
    let branch = kv_row(
        t!(i18n, ids::PROJECT_VCS_BRANCH),
        branch_value(project_id, repo, i18n, tokens),
        tokens,
    );
    let last = kv_row(
        t!(i18n, ids::PROJECT_VCS_LAST_COMMIT),
        last_commit_value(repo, git.now, i18n),
        tokens,
    );
    let history = text_button(
        Some(Glyph::History),
        t!(i18n, ids::PROJECT_VCS_OPEN_HISTORY),
        Some(open_history_message()),
        ButtonKind::Small,
        tokens,
    );
    let mut rows: Column<Element<'a, Message>> = Column::new().push(status).push(branch);
    if git.state.is_switcher_open(project_id) {
        let offers_save = git.state.offers_save_and_switch(project_id);
        rows = rows.push(
            project_git::view::switcher(project_id, repo, offers_save, i18n, tokens)
                .map(Message::from)
                .boxed(),
        );
    }
    rows.push(last)
        .push(history)
        .spacing(SP.m + SP.xs)
        .boxed()
}

/// The Repository card: the state pill in the header; the rows of
/// [`present_body`] with a repository, Enable Version Control… without one.
fn repository_card<'a>(
    project_id: ProjectId,
    repository: Repository,
    git: GitView<'_>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (words, tone) = repository_pill(repository);
    let header = CardHeader {
        title: t!(i18n, ids::PROJECT_VCS_REPOSITORY),
        trailing: vec![pill(
            t!(i18n, words),
            tone,
            repository_glyph(repository),
            tokens,
        )],
    };
    let muted = |id: MessageId| text(t!(i18n, id)).size(TS.body).color(resolve(tokens).ink3);
    let body = match repository {
        Repository::Present => present_body(project_id, git, i18n, tokens),
        Repository::Absent => {
            let enable = tab_message(ProjectTabMsg::EnableVersionControlRequested { project_id });
            row![
                muted(ids::PROJECT_VCS_REPOSITORY_ABSENT).width(Length::Fill),
                text_button(
                    None,
                    t!(i18n, ids::PROJECT_VCS_ENABLE),
                    Some(enable),
                    ButtonKind::Primary,
                    tokens,
                ),
            ]
            .spacing(SP.l)
            .align_y(Alignment::Center)
            .boxed()
        }
        Repository::Unknown => muted(ids::PROJECT_VCS_REPOSITORY_CHECKING).boxed(),
    };
    card(Some(header), body, tokens)
}

/// The Author row: who commits are written as, read from and written to the
/// repository's git config (decision 2026-11-22). Greyed without a
/// repository; Enter writes it.
fn author_row<'a>(
    project_id: ProjectId,
    repository: Repository,
    repo: Option<&RepoView>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let shown = repo.map_or("", RepoView::author_text).to_string();
    let mut input = text_input(t!(i18n, ids::PROJECT_VCS_AUTHOR_PLACEHOLDER), shown)
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fixed(AUTHOR_W));
    if repository == Repository::Present {
        input = input
            .on_input(move |text| {
                Message::from(ProjectGitMsg::AuthorEdited { project_id, text })
            })
            .on_submit(Message::from(ProjectGitMsg::AuthorSubmitted { project_id }));
    }
    let line = option_row(
        t!(i18n, ids::PROJECT_VCS_AUTHOR),
        t!(i18n, ids::PROJECT_VCS_AUTHOR_HELP),
        input.boxed(),
        tokens,
    );
    let Some(problem) = repo.and_then(|repo| repo.author_problem.as_ref()) else {
        return line;
    };
    let why = text(project_git::view::author_problem_words(problem, i18n))
        .size(TS.caption)
        .color(danger(tokens))
        .boxed();
    Column::new().push(line).push(why).spacing(SP.xs).boxed()
}

/// The Commit every save switch with its help under the label, why it may
/// not be turned on, the commits still running, and the Author row.
fn commit_card<'a>(
    project: &LoadedProject,
    facts: &VersionControlFacts,
    repo: Option<&RepoView>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let on = project.data.enable_git;
    let may_toggle = may_toggle_commit_on_save(on, facts.repository);
    let r = resolve(tokens);
    let style = ToggleSwitchStyle::new(ti(tokens.accent), r.line_strong, on_accent(tokens));
    let switch = toggle_switch(
        on,
        style,
        may_toggle.then(|| {
            tab_message(ProjectTabMsg::VersionControlToggled {
                project_id,
                on: !on,
            })
        }),
    );
    let mut body: Column<Element<'a, Message>> = Column::new()
        .push(option_row(
            t!(i18n, ids::PROJECT_VCS_AUTO_COMMIT),
            t!(i18n, ids::PROJECT_VCS_AUTO_COMMIT_HELP),
            switch,
            tokens,
        ))
        .spacing(SP.m + SP.xs);
    if !may_toggle {
        body = body.push(warn_line(
            Glyph::Info,
            text(t!(i18n, ids::PROJECT_VCS_AUTO_COMMIT_NEEDS_REPO))
                .size(TS.body)
                .boxed(),
            None,
            tokens,
        ));
    }
    if facts.pending > 0 {
        let pending = row![
            pending_dot(r.info),
            text(t!(i18n, ids::PROJECT_VCS_PENDING, "count" => facts.pending))
                .size(TS.caption)
                .color(r.ink4),
        ]
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center);
        body = body.push(pending.boxed());
    }
    body = body.push(author_row(project_id, facts.repository, repo, i18n, tokens));
    card(None, body.boxed(), tokens)
}

/// The 6 px dot before "Commits in progress".
fn pending_dot<'a>(colour: Color) -> Element<'a, Message> {
    let size = SP.s + SP.xs;
    container(Space::new().width(size).height(size))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
