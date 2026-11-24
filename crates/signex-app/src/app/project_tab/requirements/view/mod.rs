//! The Requirements section's view (mockup row 38). Pure: the project
//! and the section's state in, widgets out. Run checks sits in the section
//! header; `summary` draws the counts row under it, `card` one card per
//! entry.

mod card;
mod summary;

use iced::Element;
use serde_json::Value;
use signex_i18n::{I18n, ids, t};
use signex_types::check::{CheckReport, Finding};
use signex_types::project::{DEFAULT_PART_STATES, RequirementEntry, RequirementField};
use signex_types::theme::ThemeTokens;

use super::super::look::{ButtonKind, Glyph, text_button};
use super::super::table::{note, problem, section_with_action};
use super::drafts::keys_of;
use super::words::problem_words;
use super::{RequirementsMsg, RequirementsTabState, message};
use crate::app::Message;
use crate::app::state::{LoadedProject, ProjectId};

#[cfg(test)]
pub(super) use card::is_open;
pub(super) use summary::outcome_counts;
#[cfg(test)]
pub(super) use summary::{OutcomeCounts, summary_words};

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

/// How an entry came out, for its pill and the summary counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::project_tab::requirements) enum Verdict {
    Passed,
    Failed,
    NotChecked,
    /// Turned off: not checked, run or not.
    Off,
}

/// How entry `index` came out of `report` (`None`: no finished run, so
/// every requirement switched on is Not checked). A requirement turned off
/// is Off whether or not a run happened.
pub(in crate::app::project_tab::requirements) fn verdict(
    report: Option<&CheckReport>,
    entry: &RequirementEntry,
    index: usize,
) -> Verdict {
    if entry
        .requirement()
        .is_some_and(|requirement| !requirement.enabled)
    {
        return Verdict::Off;
    }
    let Some(report) = report else {
        return Verdict::NotChecked;
    };
    let rule = signex_req::entry_rule(entry, index);
    if report.findings.iter().any(|f| f.rule == rule) {
        Verdict::Failed
    } else if report.passed.contains(&rule) {
        Verdict::Passed
    } else {
        Verdict::NotChecked
    }
}

/// The findings the last run filed under entry `index`.
fn findings_of<'r>(
    report: Option<&'r CheckReport>,
    entry: &RequirementEntry,
    index: usize,
) -> Vec<&'r Finding> {
    let Some(report) = report else {
        return Vec::new();
    };
    let rule = signex_req::entry_rule(entry, index);
    report.findings.iter().filter(|f| f.rule == rule).collect()
}

pub(in crate::app::project_tab) fn view<'a>(
    project: &LoadedProject,
    tab: Option<&RequirementsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let project_id = project.id;
    let entries = &project.data.requirements;
    let last = tab.and_then(|tab| tab.last_run.as_deref());
    let report = last.and_then(|run| run.report.as_ref().ok());
    let editable = !project.data.has_unreadable_requirements();
    let mut body = vec![summary::summary_row(
        project_id, entries, last, report, editable, i18n, tokens,
    )];
    if editable && let Some((None, refused)) = tab.and_then(|tab| tab.problem.as_ref()) {
        body.push(problem(problem_words(refused, i18n), tokens));
    }
    body.extend(summary::run_notes(last, i18n, tokens));
    if !editable {
        body.push(problem(t!(i18n, ids::REQUIREMENTS_LIST_UNREADABLE), tokens));
    } else if entries.is_empty() {
        body.push(note(t!(i18n, ids::REQUIREMENTS_EMPTY), tokens));
    }
    let keys = keys_of(entries);
    let problem_at = tab.and_then(|tab| tab.problem_shown_at(entries));
    for ((index, entry), key) in entries.iter().enumerate().zip(keys) {
        let card = card::Card {
            project_id,
            index,
            key,
            refused_here: problem_at == Some(index),
            tab,
            report,
            i18n,
            tokens,
        };
        body.push(card.view(entry));
    }
    section_with_action(
        ids::PROJECT_SECTION_REQUIREMENTS,
        ids::REQUIREMENTS_HELP,
        run_button(project_id, tab, i18n, tokens),
        body,
        i18n,
        tokens,
    )
}

/// Run checks, the section's primary action; greyed while a run is going.
fn run_button<'a>(
    project_id: ProjectId,
    tab: Option<&RequirementsTabState>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (words, on_press) = if tab.is_some_and(|tab| tab.running) {
        (ids::REQUIREMENTS_RUNNING, None)
    } else {
        let run = message(project_id, RequirementsMsg::RunRequested);
        (ids::REQUIREMENTS_RUN, Some(run))
    };
    text_button(
        Some(Glyph::Play),
        t!(i18n, words),
        on_press,
        ButtonKind::PrimaryLarge,
        tokens,
    )
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

#[cfg(test)]
mod tests;
