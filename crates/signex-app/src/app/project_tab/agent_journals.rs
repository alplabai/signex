//! The "Agent journals" group of the Project setup section (#2253): keep the
//! journals in git, summarise agent changes in the commit message, and how
//! many lines each journal keeps. An edit is a General-history step on the
//! project's data and marks the `.snxprj` dirty; the file, and the managed
//! `.gitignore` block that follows "keep in git", are written by the normal
//! Save. See `docs/AGENT_JOURNALS.md`.

use iced::widget::{column, row};
use iced::{Element, Length, Widget as _};
use signex_engine::ProjectGeneralCommand;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::agent_journal::{
    AgentJournalSettings, MAX_JOURNAL_MAX_LINES, MIN_JOURNAL_MAX_LINES,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text, text_input};

use super::ProjectTabMsg;
use super::general::tab_message;
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};
use crate::styles::{Spacing, TypeScale};

/// The group, under the section's other rows. `lines_draft` is the cap as
/// typed until the next undo, redo or section switch shows the stored value.
pub(super) fn view<'a>(
    project: &LoadedProject,
    lines_draft: Option<&'a str>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ts = TypeScale::default();
    let spacing = Spacing::default();
    let project_id = project.id;
    let settings = project.data.agent_journal;
    let flag = |on: bool,
                label: Localized,
                set: fn(AgentJournalSettings, bool) -> AgentJournalSettings| {
        checkbox(on)
            .style(crate::panels::form_look::checkbox_style)
            .size(ts.title)
            .labelled(label)
            .text_size(ts.label)
            .on_toggle(move |on| {
                tab_message(ProjectTabMsg::AgentJournalPicked {
                    project_id,
                    settings: set(settings, on),
                })
            })
    };
    let shown = lines_draft.map_or_else(|| settings.max_lines.to_string(), str::to_string);
    let lines = text_input(t!(i18n, ids::PROJECT_AGENT_JOURNALS_LINES), shown)
        .on_input(move |text| {
            tab_message(ProjectTabMsg::AgentJournalLinesEdited { project_id, text })
        })
        .style(crate::panels::form_look::text_input_style)
        .size(ts.label)
        .padding(spacing.s)
        .width(Length::Fill);
    let note = |words: Localized| {
        text(words)
            .size(ts.caption)
            .color(crate::styles::ti(tokens.text_secondary))
            .boxed()
    };
    let mut body = column![
        text(t!(i18n, ids::PROJECT_AGENT_JOURNALS_TITLE))
            .size(ts.heading)
            .color(crate::styles::ti(tokens.text))
            .boxed(),
        flag(
            settings.keep_in_git,
            t!(i18n, ids::PROJECT_AGENT_JOURNALS_KEEP_IN_GIT),
            |s, on| AgentJournalSettings {
                keep_in_git: on,
                ..s
            },
        ),
        note(t!(i18n, ids::PROJECT_AGENT_JOURNALS_KEEP_IN_GIT_HELP)),
        flag(
            settings.summarise_in_commit,
            t!(i18n, ids::PROJECT_AGENT_JOURNALS_SUMMARISE),
            |s, on| AgentJournalSettings {
                summarise_in_commit: on,
                ..s
            },
        ),
        note(t!(i18n, ids::PROJECT_AGENT_JOURNALS_SUMMARISE_HELP)),
        row![
            text(t!(i18n, ids::PROJECT_AGENT_JOURNALS_LINES))
                .size(ts.body)
                .color(crate::styles::ti(tokens.text_secondary)),
            lines.boxed(),
        ]
        .spacing(spacing.m),
        note(t!(i18n, ids::PROJECT_AGENT_JOURNALS_LINES_HELP)),
    ]
    .spacing(spacing.m);
    if lines_draft.is_some_and(|text| parse_lines(text).is_none()) {
        body = body.push(note(t!(
            i18n,
            ids::PROJECT_AGENT_JOURNALS_LINES_INVALID,
            "min" => i64::from(MIN_JOURNAL_MAX_LINES),
            "max" => i64::from(MAX_JOURNAL_MAX_LINES)
        )));
    }
    body.boxed()
}

/// The cap typed, when it is a whole number in range.
pub(super) fn parse_lines(text: &str) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|lines| (MIN_JOURNAL_MAX_LINES..=MAX_JOURNAL_MAX_LINES).contains(lines))
}

impl Signex {
    /// A checkbox of the group: a step of its own.
    pub(super) fn pick_agent_journal(
        &mut self,
        project_id: ProjectId,
        settings: AgentJournalSettings,
    ) {
        self.close_general_run(project_id);
        self.apply_project_general(
            project_id,
            ProjectGeneralCommand::SetAgentJournal {
                settings,
                typing: false,
            },
        );
    }

    /// The line cap as typed: kept as typed, and stored once it is a number in
    /// range. Typing joins one undo step.
    pub(super) fn edit_agent_journal_lines(&mut self, project_id: ProjectId, text: String) {
        let Some(project) = self.document_state.project_by_id(project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return;
        };
        let stored = project.data.agent_journal;
        let parsed = parse_lines(&text);
        self.document_state
            .project_tabs
            .entry(project_id)
            .or_default()
            .journal_lines_draft = Some(text);
        if let Some(max_lines) = parsed {
            self.apply_project_general(
                project_id,
                ProjectGeneralCommand::SetAgentJournal {
                    settings: AgentJournalSettings {
                        max_lines,
                        ..stored
                    },
                    typing: true,
                },
            );
        }
    }
}
