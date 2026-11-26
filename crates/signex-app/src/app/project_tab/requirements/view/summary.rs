//! The summaries of the Requirements section: the row under the section
//! header (outcome counts, when the last run finished, Add requirement…),
//! the run's notes under it, and a card's one-line value summary.

use iced::widget::{Row, Space, pick_list};
use iced::{Alignment, Element, Length, Widget as _};
use serde_json::Value;
use signex_i18n::{DateLength, I18n, ListKind, Localized, UserText, ids, t};
use signex_types::check::CheckReport;
use signex_types::project::{
    REQUIREMENT_KINDS, Requirement, RequirementEntry, RequirementField, RequirementKind,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, text};

use super::super::super::look::pill;
use super::super::super::table::{SP, TS, note, problem};
use super::super::words::kind_words;
use super::super::{RequirementsMsg, RequirementsRun, message};
use super::{Choice, Verdict, stored_text, verdict};
use crate::app::Message;
use crate::app::state::ProjectId;
use crate::styles::PillTone;
use crate::styles::tokens as ink;

/// Width of the Add picker, in `body`-size characters (200 px at the
/// default size, as drawn).
const PICKER_EMS: f32 = 16.7;

/// How many entries came out each way in a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app::project_tab::requirements) struct OutcomeCounts {
    pub passed: usize,
    pub failed: usize,
    pub not_checked: usize,
    /// Turned off: counted apart, never as not checked.
    pub off: usize,
}

/// Count the entries by how they came out of `report` (`None`: no finished
/// run, so every entry switched on counts as not checked).
pub(in crate::app::project_tab::requirements) fn outcome_counts(
    report: Option<&CheckReport>,
    entries: &[RequirementEntry],
) -> OutcomeCounts {
    let mut counts = OutcomeCounts::default();
    for (index, entry) in entries.iter().enumerate() {
        match verdict(report, entry, index) {
            Verdict::Passed => counts.passed += 1,
            Verdict::Failed => counts.failed += 1,
            Verdict::Off => counts.off += 1,
            Verdict::NotChecked => counts.not_checked += 1,
        }
    }
    counts
}

/// The row under the section header: the counts (Passed and Failed after a
/// finished run, Not checked whenever there are any, before the first run
/// too), then when the last run finished, then the Add picker (left out
/// when the list cannot be edited). `report` is `last`'s, when it finished.
pub(super) fn summary_row<'a>(
    project_id: ProjectId,
    entries: &[RequirementEntry],
    last: Option<&RequirementsRun>,
    report: Option<&CheckReport>,
    editable: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    let counts = outcome_counts(report, entries);
    if report.is_some() {
        let passed = t!(i18n, ids::REQUIREMENTS_PASSED_COUNT, "count" => counts.passed);
        line = line.push(pill(passed, PillTone::Ok, None, tokens));
        let failed = t!(i18n, ids::REQUIREMENTS_FAILED_COUNT, "count" => counts.failed);
        line = line.push(pill(failed, PillTone::Err, None, tokens));
    }
    if counts.not_checked > 0 {
        let open = t!(i18n, ids::REQUIREMENTS_NOT_CHECKED_COUNT, "count" => counts.not_checked);
        line = line.push(pill(open, PillTone::Neutral, None, tokens));
    }
    line = line.push(Space::new().width(Length::Fill).boxed());
    let when = match last {
        Some(run) => {
            let time = i18n.date_time(run.finished_at, DateLength::Medium);
            t!(i18n, ids::REQUIREMENTS_LAST_RUN, "time" => time)
        }
        None => t!(i18n, ids::REQUIREMENTS_NEVER_RUN),
    };
    line = line.push(note(when, tokens));
    if editable {
        line = line.push(add_picker(project_id, i18n));
    }
    line.width(Length::Fill).boxed()
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
        .placeholder_label(t!(i18n, ids::REQUIREMENTS_ADD))
        .on_select(move |choice: Choice<&'static str>| {
            message(
                project_id,
                RequirementsMsg::AddPicked { kind: choice.value },
            )
        })
        .width(Length::Fixed(TS.body * PICKER_EMS))
        .boxed()
}

/// Under the summary row: why the last run did not finish, or its notes.
pub(super) fn run_notes<'a>(
    last: Option<&RequirementsRun>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, Message>> {
    match last.map(|run| &run.report) {
        Some(Err(why)) => {
            let words = t!(i18n, ids::REQUIREMENTS_RUN_FAILED, "reason" => why.as_str());
            vec![problem(words, tokens)]
        }
        Some(Ok(report)) if !report.warnings.is_empty() => {
            let mut notes = vec![note(t!(i18n, ids::REQUIREMENTS_RUN_NOTES), tokens)];
            for warning in &report.warnings {
                notes.push(
                    text(UserText::new(warning.clone()))
                        .size(TS.caption)
                        .color(ink::muted_text(tokens))
                        .boxed(),
                );
            }
            notes
        }
        _ => Vec::new(),
    }
}

/// A card's one-line summary of its values, as stored (`raw` keeps the
/// numbers as written). `None` for a `net_class` that asks nothing.
pub(in crate::app::project_tab::requirements) fn summary_words(
    requirement: &Requirement,
    raw: &Value,
    i18n: &I18n,
) -> Option<Localized> {
    let value = |field: RequirementField| stored_text(field, raw);
    let words = match &requirement.kind {
        RequirementKind::ErcClean { allow_warnings }
        | RequirementKind::DrcClean { allow_warnings } => {
            if *allow_warnings {
                t!(i18n, ids::REQUIREMENTS_SUMMARY_CLEAN_WARNINGS_ALLOWED)
            } else {
                t!(i18n, ids::REQUIREMENTS_SUMMARY_CLEAN)
            }
        }
        RequirementKind::BoardSize { .. } => t!(
            i18n,
            ids::REQUIREMENTS_SUMMARY_BOARD_SIZE,
            "width" => value(RequirementField::MaxWidthMm),
            "height" => value(RequirementField::MaxHeightMm),
        ),
        RequirementKind::PartsState { states } => t!(
            i18n,
            ids::REQUIREMENTS_SUMMARY_PARTS_STATE,
            "states" => i18n.list(states.as_slice(), ListKind::Or),
        ),
        RequirementKind::NetClass {
            nets,
            class,
            min_trace_width_mm,
            min_clearance_mm,
        } => {
            let limits = net_class_limits(
                class.as_deref(),
                min_trace_width_mm.is_some(),
                min_clearance_mm.is_some(),
                raw,
                i18n,
            );
            if limits.is_empty() {
                return None;
            }
            t!(
                i18n,
                ids::REQUIREMENTS_SUMMARY_NET_CLASS,
                "nets" => nets.as_str(),
                "limits" => i18n.list(limits.as_slice(), ListKind::And),
            )
        }
        RequirementKind::BomCost { .. } => t!(
            i18n,
            ids::REQUIREMENTS_SUMMARY_BOM_COST,
            "cost" => value(RequirementField::MaxUnitCost),
            "currency" => value(RequirementField::Currency),
            "quantity" => value(RequirementField::Quantity),
        ),
        RequirementKind::RailBudget { net, .. } => {
            let limits = rail_limits(raw, i18n);
            t!(
                i18n,
                ids::REQUIREMENTS_SUMMARY_RAIL_BUDGET,
                "net" => net.as_str(),
                "limits" => i18n.list(limits.as_slice(), ListKind::And),
            )
        }
        RequirementKind::LayerCount {
            min_layers,
            max_layers,
        } => {
            let min = value(RequirementField::MinLayers);
            let max = value(RequirementField::MaxLayers);
            match (min_layers, max_layers) {
                (Some(_), Some(_)) => t!(
                    i18n,
                    ids::REQUIREMENTS_SUMMARY_LAYER_RANGE,
                    "min" => min,
                    "max" => max,
                ),
                (Some(_), None) => t!(i18n, ids::REQUIREMENTS_SUMMARY_LAYER_MIN, "min" => min),
                (None, Some(_)) => t!(i18n, ids::REQUIREMENTS_SUMMARY_LAYER_MAX, "max" => max),
                (None, None) => return None,
            }
        }
    };
    Some(words)
}

/// The limits a `rail_budget` asks, one phrase each, as stored: the
/// voltage band, the load against the supply, the load cap and the margin.
fn rail_limits(raw: &Value, i18n: &I18n) -> Vec<String> {
    let set = |field: RequirementField| {
        let text = stored_text(field, raw);
        (!text.is_empty()).then_some(text)
    };
    let mut limits: Vec<String> = Vec::new();
    match (
        set(RequirementField::Voltage),
        set(RequirementField::TolerancePercent),
    ) {
        (Some(voltage), Some(tolerance)) => limits.push(
            t!(
                i18n,
                ids::REQUIREMENTS_SUMMARY_RAIL_VOLTAGE_TOLERANCE,
                "voltage" => voltage,
                "tolerance" => tolerance,
            )
            .into_string(),
        ),
        (Some(voltage), None) => limits.push(
            t!(i18n, ids::REQUIREMENTS_SUMMARY_RAIL_VOLTAGE, "voltage" => voltage).into_string(),
        ),
        _ => {}
    }
    limits.push(t!(i18n, ids::REQUIREMENTS_SUMMARY_RAIL_SUPPLY).into_string());
    if let Some(current) = set(RequirementField::MaxCurrentA) {
        limits.push(
            t!(i18n, ids::REQUIREMENTS_SUMMARY_RAIL_MAX_CURRENT, "current" => current)
                .into_string(),
        );
    }
    if let Some(margin) = set(RequirementField::MarginPercent) {
        limits.push(
            t!(i18n, ids::REQUIREMENTS_SUMMARY_RAIL_MARGIN, "margin" => margin).into_string(),
        );
    }
    limits
}

/// The limits a `net_class` asks, one phrase each: its class, then the
/// minimum trace width and clearance it sets, as stored.
fn net_class_limits(
    class: Option<&str>,
    has_min_trace: bool,
    has_min_clearance: bool,
    raw: &Value,
    i18n: &I18n,
) -> Vec<String> {
    let mut limits: Vec<String> = Vec::new();
    if let Some(class) = class.map(str::trim).filter(|c| !c.is_empty()) {
        limits.push(t!(i18n, ids::REQUIREMENTS_SUMMARY_CLASS, "class" => class).into_string());
    }
    if has_min_trace {
        let width = stored_text(RequirementField::MinTraceWidthMm, raw);
        limits.push(t!(i18n, ids::REQUIREMENTS_SUMMARY_MIN_TRACE, "width" => width).into_string());
    }
    if has_min_clearance {
        let clearance = stored_text(RequirementField::MinClearanceMm, raw);
        limits.push(
            t!(i18n, ids::REQUIREMENTS_SUMMARY_MIN_CLEARANCE, "clearance" => clearance)
                .into_string(),
        );
    }
    limits
}
