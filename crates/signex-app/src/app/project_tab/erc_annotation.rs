//! ERC and annotation section of the Project tab (#1692): where the project's
//! ERC rules come from (stored in the project, this machine's Preferences, or
//! an unreadable newer set), how many rules run at each severity, "Store ERC
//! rules in this project", and the project's Annotate undo choice.
//!
//! Migration: Preferences values seed a new project and are the default of a
//! project that stores none; they never overwrite a project's own values.
//! Storing the rules is a step of the project's rule history (shared with the
//! ERC dialog and the Rules tab); the annotate choice is a General step.

use iced::widget::column;
use iced::{Element, Length, Widget as _};
use signex_erc::Severity;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::project::AnnotateUndo;
use signex_types::theme::ThemeTokens;
use signex_widgets::radio;

use super::ProjectTabMsg;
use super::general::tab_message;
use super::table::{
    SP, TS, action, heading, heading_row, labelled, note, section, table_row, word_cell,
};
use crate::app::state::{LoadedProject, ProjectId};
use crate::app::{Message, Signex};

/// Where a project's ERC rules come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErcSource {
    /// The project stores its own rule set.
    Project,
    /// The project stores none: this machine's Preferences apply.
    Machine,
    /// The project holds a rule set this build cannot read.
    Unreadable,
}

impl ErcSource {
    pub(crate) fn of(project: &LoadedProject) -> Self {
        if project.data.rules.is_some() {
            Self::Project
        } else if project.data.has_unreadable_rules() {
            Self::Unreadable
        } else {
            Self::Machine
        }
    }

    /// The token of the `project_settings` answer.
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Machine => "preferences",
            Self::Unreadable => "unreadable",
        }
    }
}

fn severity_words(severity: Severity) -> MessageId {
    match severity {
        Severity::Error => ids::PROJECT_ERC_SEVERITY_ERROR,
        Severity::Warning => ids::PROJECT_ERC_SEVERITY_WARNING,
        Severity::Info => ids::PROJECT_ERC_SEVERITY_INFO,
        Severity::Off => ids::PROJECT_ERC_SEVERITY_OFF,
    }
}

fn annotate_words(choice: AnnotateUndo) -> MessageId {
    match choice {
        AnnotateUndo::Project => ids::PROJECT_ANNOTATE_UNDO_PROJECT,
        AnnotateUndo::PerSheet => ids::PROJECT_ANNOTATE_UNDO_PER_SHEET,
    }
}

/// What the section shows, read from the Model by [`Signex::erc_section_facts`].
pub(crate) struct ErcFacts {
    pub source: ErcSource,
    /// Rules per severity, Error, Warning, Info, Off.
    pub counts: [(Severity, usize); 4],
    /// This machine's Annotate undo preference.
    pub machine_annotate_undo: AnnotateUndo,
}

pub(super) fn view<'a>(
    project: &LoadedProject,
    facts: &ErcFacts,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let mut body: Vec<Element<'a, Message>> =
        vec![heading(t!(i18n, ids::PROJECT_ERC_TITLE), tokens)];
    let state = match facts.source {
        ErcSource::Project => ids::PROJECT_ERC_STORED,
        ErcSource::Machine => ids::PROJECT_ERC_MACHINE,
        ErcSource::Unreadable => ids::PROJECT_ERC_UNREADABLE,
    };
    body.push(note(t!(i18n, state), tokens));
    if facts.source == ErcSource::Machine {
        let store = tab_message(ProjectTabMsg::ErcRulesStoreRequested { project_id });
        body.push(action(
            t!(i18n, ids::PROJECT_ERC_STORE),
            Some(store),
            tokens,
        ));
    }
    if facts.source != ErcSource::Unreadable {
        let columns = [
            (ids::PROJECT_ERC_COL_SEVERITY, Length::FillPortion(1)),
            (ids::PROJECT_ERC_COL_RULES, Length::FillPortion(1)),
        ];
        body.push(heading_row(&columns, None, i18n, tokens));
        for (severity, count) in facts.counts {
            body.push(table_row(vec![
                (
                    word_cell(t!(i18n, severity_words(severity)), tokens),
                    columns[0].1,
                ),
                (
                    word_cell(
                        t!(i18n, ids::PROJECT_ERC_RULE_COUNT, "count" => count),
                        tokens,
                    ),
                    columns[1].1,
                ),
            ]));
        }
        body.push(note(t!(i18n, ids::PROJECT_ERC_EDIT_HINT), tokens));
    }
    body.push(heading(t!(i18n, ids::PROJECT_ANNOTATE_TITLE), tokens));
    body.push(annotate_choice(
        project,
        facts.machine_annotate_undo,
        i18n,
        tokens,
    ));
    body.push(note(t!(i18n, ids::PROJECT_ANNOTATE_UNDO_HELP), tokens));
    section(
        ids::PROJECT_SECTION_ERC_ANNOTATION,
        ids::PROJECT_ERC_ANNOTATION_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The three Annotate undo choices: the Preferences default (named with its
/// current value), or one of the two values stored in the project.
fn annotate_choice<'a>(
    project: &LoadedProject,
    machine: AnnotateUndo,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let selected = Some(project.data.annotate_undo());
    let pick = move |choice: Option<AnnotateUndo>| {
        tab_message(ProjectTabMsg::AnnotateUndoPicked { project_id, choice })
    };
    let machine_words = t!(i18n, annotate_words(machine)).into_string();
    let default = t!(i18n, ids::PROJECT_ANNOTATE_UNDO_DEFAULT, "value" => machine_words);
    let mut choices = column![
        radio(default, None, selected, pick)
            .size(TS.body)
            .text_size(TS.body)
    ]
    .spacing(SP.xs);
    for choice in AnnotateUndo::ALL.iter().copied() {
        choices = choices.push(
            radio(
                t!(i18n, annotate_words(choice)),
                Some(choice),
                selected,
                pick,
            )
            .size(TS.body)
            .text_size(TS.body)
            .boxed(),
        );
    }
    labelled(
        t!(i18n, ids::PROJECT_ANNOTATE_UNDO),
        choices.boxed(),
        tokens,
    )
}

impl Signex {
    /// The section's facts: where the rules come from, the severities they
    /// give, and the Preferences default.
    pub(crate) fn erc_section_facts(&self, project: &LoadedProject) -> ErcFacts {
        let source = ErcSource::of(project);
        let overrides = match project.data.rules.as_ref() {
            Some(rules) => signex_erc::project_rules::resolve(&rules.erc).severity,
            None => self.ui_state.erc_severity_override.clone(),
        };
        ErcFacts {
            source,
            counts: signex_erc::project_rules::severity_counts(&overrides),
            machine_annotate_undo: self.ui_state.annotate_undo,
        }
    }

    /// The Annotate undo of the project rooted at `root`: its own choice, else
    /// this machine's Preferences value (#1692). A loose sheet follows
    /// Preferences.
    pub(crate) fn annotate_undo_of(&self, root: Option<&std::path::Path>) -> AnnotateUndo {
        root.and_then(|root| self.document_state.document_project(root))
            .and_then(|project| project.data.annotate_undo())
            .unwrap_or(self.ui_state.annotate_undo)
    }

    /// The Annotate undo radio: one General step; `None` follows Preferences.
    pub(super) fn pick_project_annotate_undo(
        &mut self,
        project_id: ProjectId,
        choice: Option<AnnotateUndo>,
    ) {
        let command = signex_engine::ProjectSectionCommand::SetAnnotateUndo { choice };
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }

    /// "Store ERC rules in this project" for `project_id`: a copy of this
    /// machine's rules, one step of the project's rule history. Refused when
    /// the project already stores rules or holds an unreadable newer set,
    /// which storing would replace. `Ok(false)` when it already stores some.
    pub(crate) fn store_erc_rules_of(&mut self, project_id: ProjectId) -> Result<bool, String> {
        let project = self
            .document_state
            .project_by_id(project_id)
            .ok_or_else(|| "the project is no longer open".to_string())?;
        match ErcSource::of(project) {
            ErcSource::Project => Ok(false),
            ErcSource::Unreadable => Err(
                "this project holds a rule set from a newer Signex, which storing rules would \
                 replace"
                    .to_string(),
            ),
            ErcSource::Machine => {
                let rules = self.seeded_project_rules();
                self.replace_rules_of(project_id, Some(rules), true);
                Ok(true)
            }
        }
    }

    pub(super) fn store_erc_rules_in(&mut self, project_id: ProjectId) {
        if let Err(problem) = self.store_erc_rules_of(project_id) {
            crate::diagnostics::log_warning(format!("Project tab: {problem}"));
        }
    }
}
