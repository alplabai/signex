//! The Requirements section's view. Pure: the project and the section's
//! state in, widgets out.

use iced::widget::{Column, column, container, pick_list, row};
use iced::{Alignment, Element, Length, Widget as _};
use serde_json::Value;
use signex_i18n::{DateLength, I18n, UserText, ids, t};
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::project::{
    DEFAULT_PART_STATES, REQUIREMENT_KINDS, Requirement, RequirementEntry, RequirementField,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, checkbox, text, text_input};

use super::super::table::{SP, TS, action, labelled, note, problem, section};
use super::words::{field_words, kind_words, problem_words};
use super::{RequirementsMsg, RequirementsTabState, message};
use crate::app::Message;
use crate::app::state::{LoadedProject, ProjectId};
use crate::styles::tokens as ink;

/// Widest the Add picker grows, in `body`-size characters.
const PICKER_EMS: f32 = 18.0;

/// A choice of a picker: a value and its words.
#[derive(Debug, Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// How a requirement came out of the last run.
enum Outcome<'r> {
    Passed,
    Failed(Vec<&'r Finding>),
    NotChecked,
}

fn outcome<'r>(report: Option<&'r CheckReport>, id: &str) -> Option<Outcome<'r>> {
    let report = report?;
    let rule = format!("req::{}", id.trim());
    let found: Vec<&Finding> = report.findings.iter().filter(|f| f.rule == rule).collect();
    Some(if !found.is_empty() {
        Outcome::Failed(found)
    } else if report.passed.contains(&rule) {
        Outcome::Passed
    } else {
        Outcome::NotChecked
    })
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    tab: Option<&RequirementsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let data = &project.data;
    let report = tab
        .and_then(|tab| tab.last_run.as_ref())
        .and_then(|run| run.report.as_ref().ok());
    let mut body: Vec<Element<'a, Message>> = Vec::new();
    let editable = !data.has_unreadable_requirements();
    if !editable {
        body.push(problem(t!(i18n, ids::REQUIREMENTS_LIST_UNREADABLE), tokens));
    } else if data.requirements.is_empty() {
        body.push(note(t!(i18n, ids::REQUIREMENTS_EMPTY), tokens));
    }
    for (index, entry) in data.requirements.iter().enumerate() {
        let card = Entry {
            project_id,
            index,
            tab,
            report,
            i18n,
            tokens,
        };
        body.push(card.view(entry));
    }
    if editable {
        body.push(add_picker(project_id, i18n));
        if let Some((None, refused)) = tab.and_then(|tab| tab.problem.as_ref()) {
            body.push(problem(problem_words(refused, i18n), tokens));
        }
    }
    body.push(run_row(project_id, tab, i18n, tokens));
    section(
        ids::PROJECT_SECTION_REQUIREMENTS,
        ids::REQUIREMENTS_HELP,
        body,
        i18n,
        tokens,
    )
}

/// The Add picker: picking a kind adds a requirement of it.
fn add_picker<'a>(project_id: ProjectId, i18n: &I18n) -> Element<'a, Message> {
    let kinds: Vec<Choice<&'static str>> = REQUIREMENT_KINDS
        .iter()
        .map(|kind| Choice {
            value: *kind,
            label: t!(i18n, kind_words(kind)).to_string(),
        })
        .collect();
    pick_list(None::<Choice<&'static str>>, kinds, Choice::to_string)
        .graphite()
        .placeholder(t!(i18n, ids::REQUIREMENTS_ADD).to_string())
        .on_select(move |choice: Choice<&'static str>| {
            message(project_id, RequirementsMsg::AddPicked { kind: choice.value })
        })
        .width(Length::Fixed(TS.body * PICKER_EMS))
        .boxed()
}

/// Run checks, and when the last run finished or why it did not.
fn run_row<'a>(
    project_id: ProjectId,
    tab: Option<&RequirementsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let running = tab.is_some_and(|tab| tab.running);
    let label = if running {
        ids::REQUIREMENTS_RUNNING
    } else {
        ids::REQUIREMENTS_RUN
    };
    let run = (!running).then(|| message(project_id, RequirementsMsg::RunRequested));
    let last = tab.and_then(|tab| tab.last_run.as_ref());
    let when = match last {
        Some(run) => {
            let time = i18n.date_time(run.finished_at, DateLength::Medium);
            t!(i18n, ids::REQUIREMENTS_LAST_RUN, "time" => time)
        }
        None => t!(i18n, ids::REQUIREMENTS_NEVER_RUN),
    };
    let mut col: Column<Element<'a, Message>> = column![
        row![action(t!(i18n, label), run, tokens), note(when, tokens)]
            .spacing(SP.m)
            .align_y(Alignment::Center),
    ]
    .spacing(SP.s);
    match last.map(|run| &run.report) {
        Some(Err(why)) => {
            let words = t!(i18n, ids::REQUIREMENTS_RUN_FAILED, "reason" => why.as_str());
            col = col.push(problem(words, tokens));
        }
        Some(Ok(report)) if !report.warnings.is_empty() => {
            col = col.push(note(t!(i18n, ids::REQUIREMENTS_RUN_NOTES), tokens));
            for warning in &report.warnings {
                col = col.push(user_note(warning.clone(), tokens));
            }
        }
        _ => {}
    }
    col.boxed()
}

/// Secondary user text: a finding, a run note.
fn user_note<'a>(words: String, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(UserText::new(words))
        .size(TS.caption)
        .color(ink::muted_text(tokens))
        .boxed()
}

/// One entry's card.
struct Entry<'s> {
    project_id: ProjectId,
    index: usize,
    tab: Option<&'s RequirementsTabState>,
    report: Option<&'s CheckReport>,
    i18n: &'s I18n,
    tokens: &'s ThemeTokens,
}

impl Entry<'_> {
    fn view<'a>(&self, entry: &RequirementEntry) -> Element<'a, Message> {
        let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
        match entry {
            RequirementEntry::Known { requirement, raw } => {
                col = col.push(self.header(requirement));
                col = col.push(self.text_field(RequirementField::Id, raw));
                col = col.push(self.severity(requirement.severity));
                for field in RequirementField::of_kind(requirement.kind.name()) {
                    col = col.push(self.kind_field(*field, raw));
                }
                if let Some(Outcome::Failed(found)) = outcome(self.report, &requirement.id) {
                    for finding in found {
                        col = col.push(self.evidence(finding));
                    }
                }
            }
            RequirementEntry::Unreadable { reason, .. } => {
                let label = entry
                    .id()
                    .map_or_else(|| format!("#{}", self.index + 1), str::to_string);
                col = col.push(
                    row![
                        text(UserText::new(label))
                            .size(TS.title)
                            .color(crate::styles::ti(self.tokens.text))
                            .width(Length::Fill),
                        self.remove(),
                    ]
                    .align_y(Alignment::Center)
                    .boxed(),
                );
                let words = t!(self.i18n, ids::REQUIREMENTS_UNREADABLE, "reason" => reason.as_str());
                col = col.push(problem(words, self.tokens));
            }
        }
        if let Some((Some(at), refused)) = self.tab.and_then(|tab| tab.problem.as_ref())
            && *at == self.index
        {
            col = col.push(problem(problem_words(refused, self.i18n), self.tokens));
        }
        container(col)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .style(crate::styles::card(self.tokens))
            .boxed()
    }

    /// Enabled, the kind, the last run's outcome and Remove.
    fn header<'a>(&self, requirement: &Requirement) -> Element<'a, Message> {
        let (project_id, index) = (self.project_id, self.index);
        let enabled = checkbox(requirement.enabled)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.title)
            .labelled(t!(self.i18n, ids::REQUIREMENTS_ENABLED))
            .text_size(TS.label)
            .on_toggle(move |on| message(project_id, RequirementsMsg::EnabledToggled { index, on }));
        let kind = text(t!(self.i18n, kind_words(requirement.kind.name())))
            .size(TS.title)
            .color(crate::styles::ti(self.tokens.text))
            .width(Length::Fill);
        let mut line = row![enabled, kind].spacing(SP.m).align_y(Alignment::Center);
        if let Some(status) = outcome(self.report, &requirement.id) {
            let (words, color) = match status {
                Outcome::Passed => (ids::REQUIREMENTS_PASSED, crate::styles::ti(self.tokens.success)),
                Outcome::Failed(_) => (ids::REQUIREMENTS_FAILED, ink::danger(self.tokens)),
                Outcome::NotChecked => (ids::REQUIREMENTS_NOT_CHECKED, ink::muted_text(self.tokens)),
            };
            line = line.push(text(t!(self.i18n, words)).size(TS.label).color(color).boxed());
        }
        line.push(self.remove()).boxed()
    }

    fn remove<'a>(&self) -> Element<'a, Message> {
        let removed = message(self.project_id, RequirementsMsg::Removed { index: self.index });
        action(t!(self.i18n, ids::REQUIREMENTS_REMOVE), Some(removed), self.tokens)
    }

    fn severity<'a>(&self, current: Severity) -> Element<'a, Message> {
        let (project_id, index) = (self.project_id, self.index);
        let choices: Vec<Choice<&'static str>> = [
            ("error", ids::PROJECT_ERC_SEVERITY_ERROR),
            ("warning", ids::PROJECT_ERC_SEVERITY_WARNING),
            ("info", ids::PROJECT_ERC_SEVERITY_INFO),
        ]
        .into_iter()
        .map(|(value, words)| Choice {
            value,
            label: t!(self.i18n, words).to_string(),
        })
        .collect();
        let token = match current {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        let selected = choices.iter().find(|c| c.value == token).cloned();
        let picker = pick_list(selected, choices, Choice::to_string)
            .graphite()
            .on_select(move |choice: Choice<&'static str>| {
                message(
                    project_id,
                    RequirementsMsg::FieldPicked {
                        index,
                        field: RequirementField::Severity,
                        value: choice.value.to_string(),
                    },
                )
            })
            .width(Length::Fill);
        self.labelled(RequirementField::Severity, picker.boxed())
    }

    fn kind_field<'a>(&self, field: RequirementField, raw: &Value) -> Element<'a, Message> {
        if field != RequirementField::AllowWarnings {
            return self.text_field(field, raw);
        }
        let (project_id, index) = (self.project_id, self.index);
        let on = raw.get(field.key()).and_then(Value::as_bool).unwrap_or(false);
        let flag = checkbox(on)
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.title)
            .on_toggle(move |on| {
                let value = on.to_string();
                message(project_id, RequirementsMsg::FieldPicked { index, field, value })
            });
        self.labelled(field, flag.boxed())
    }

    /// A field's box: what was typed, else the stored value; Enter commits.
    fn text_field<'a>(&self, field: RequirementField, raw: &Value) -> Element<'a, Message> {
        let (project_id, index) = (self.project_id, self.index);
        let draft = self
            .tab
            .and_then(|tab| tab.drafts.get(&(index, field)))
            .cloned();
        let shown = draft.unwrap_or_else(|| stored_text(field, raw));
        let hint = if field.is_optional() {
            t!(self.i18n, ids::REQUIREMENTS_NOT_SET)
        } else {
            t!(self.i18n, field_words(field))
        };
        let input = text_input(hint, shown)
            .on_input(move |text| {
                message(project_id, RequirementsMsg::FieldEdited { index, field, text })
            })
            .on_submit(message(
                project_id,
                RequirementsMsg::FieldSubmitted { index, field },
            ))
            .style(crate::panels::form_look::text_input_style)
            .size(TS.body)
            .padding(SP.xs)
            .width(Length::Fill);
        self.labelled(field, input.boxed())
    }

    fn labelled<'a>(&self, field: RequirementField, value: Element<'a, Message>) -> Element<'a, Message> {
        labelled(t!(self.i18n, field_words(field)), value, self.tokens)
    }

    /// A finding of the last run: its message and the measured values.
    fn evidence<'a>(&self, finding: &Finding) -> Element<'a, Message> {
        let mut col: Column<Element<'a, Message>> = column![
            text(UserText::new(finding.message.clone()))
                .size(TS.caption)
                .color(ink::danger(self.tokens)),
        ];
        if let Value::Object(map) = &finding.evidence {
            let measured: Vec<String> = map
                .iter()
                .filter(|(key, _)| key.as_str() != "kind")
                .map(|(key, value)| format!("{key}: {}", plain(value)))
                .collect();
            if !measured.is_empty() {
                col = col.push(user_note(measured.join(" · "), self.tokens));
            }
        }
        col.spacing(SP.xs).boxed()
    }
}

/// The stored value of `field` as the box shows it.
fn stored_text(field: RequirementField, raw: &Value) -> String {
    match raw.get(field.key()) {
        None | Some(Value::Null) if field == RequirementField::States => {
            DEFAULT_PART_STATES.join(", ")
        }
        None | Some(Value::Null) => String::new(),
        Some(value) => plain(value),
    }
}

/// A JSON value as a person reads it: text unquoted, a list comma-separated.
fn plain(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(plain).collect::<Vec<_>>().join(", "),
        other => other.to_string(),
    }
}
