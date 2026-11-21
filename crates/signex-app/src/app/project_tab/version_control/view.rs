//! The Version control section's view (mockup row 35): a Repository
//! card with its state pill, then the Commit every save switch with its
//! notes. Pure: the project and its facts in, widgets out.

use iced::widget::{Column, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{ToggleSwitchStyle, text, toggle_switch};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{
    ButtonKind, CardHeader, Glyph, card, kv_row, option_row, pill, text_button, warn_line,
};
use super::super::table::{SP, TS, section};
use super::{Repository, VersionControlFacts, may_toggle_commit_on_save};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, OverlayMsg};
use crate::panels::PanelKind;
use crate::styles::tokens::{on_accent, resolve};
use crate::styles::{PillTone, Radii, ti};

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

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    facts: &VersionControlFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let body = vec![
        repository_card(project.id, facts.repository, i18n, tokens),
        commit_card(project, facts, i18n, tokens),
    ];
    section(
        ids::PROJECT_SECTION_VERSION_CONTROL,
        ids::PROJECT_VCS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The Repository card: the state pill in the header; the status and Open
/// History panel with a repository, Enable Version Control… without one.
fn repository_card<'a>(
    project_id: ProjectId,
    repository: Repository,
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
        Repository::Present => {
            let status = kv_row(
                t!(i18n, ids::PROJECT_VCS_STATUS),
                text(t!(i18n, ids::PROJECT_VCS_REPOSITORY_PRESENT))
                    .size(TS.body)
                    .boxed(),
                tokens,
            );
            let history = text_button(
                Some(Glyph::History),
                t!(i18n, ids::PROJECT_VCS_OPEN_HISTORY),
                Some(open_history_message()),
                ButtonKind::Small,
                tokens,
            );
            Column::new()
                .push(status)
                .push(history)
                .spacing(SP.m + SP.xs)
                .boxed()
        }
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

/// The Commit every save switch with its help under the label, why it may
/// not be turned on, and the commits still running.
fn commit_card<'a>(
    project: &LoadedProject,
    facts: &VersionControlFacts,
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
