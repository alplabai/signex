//! `parts_state`: every part the BOM buys was placed from a library row in
//! one of the allowed lifecycle states. A part with no library row, or with
//! one no open library holds, has no state and does not pass.

use std::collections::HashMap;

use serde_json::{Value, json};
use signex_bom::{BomRow, BomTable};
use signex_types::check::Finding;

pub use signex_types::project::LIFECYCLE_STATES;

use crate::Miss;

/// What one `parts_state` asks, past its allowed states.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PartsScope<'a> {
    /// The variant whose BOM is checked, as the requirement names it;
    /// `None`: the active variant.
    pub(crate) variant: Option<&'a str>,
    /// Only the parts the variant fits are checked.
    pub(crate) fitted_only: bool,
}

/// `parts_state`. `parts` is the ungrouped BOM (one row per part) of the
/// variant `scope` names.
pub(crate) fn parts_state(
    parts: Option<&Result<BomTable, String>>,
    row_states: &HashMap<String, String>,
    states: &[String],
    scope: PartsScope<'_>,
) -> Vec<Miss> {
    if states.is_empty() {
        return vec![Miss::new(
            "`states` is empty, so no part could pass; the requirement was not checked",
        )];
    }
    let unknown: Vec<&String> = states
        .iter()
        .filter(|s| !LIFECYCLE_STATES.contains(&s.as_str()))
        .collect();
    if !unknown.is_empty() {
        return vec![
            Miss::new(format!(
                "`states` names {unknown:?}, which are no lifecycle states ({}), so the requirement was not checked",
                LIFECYCLE_STATES.join(", ")
            ))
            .with("unknown", json!(unknown)),
        ];
    }
    let table = match parts {
        None => return vec![Miss::new("The parts were not prepared")],
        Some(Err(why)) => {
            let miss = Miss::new(format!("The parts could not be listed: {why}"));
            return vec![miss.with("reason", json!(why))];
        }
        Some(Ok(table)) => table,
    };
    let allowed = states.join(", ");
    let rows: Vec<&BomRow> = table
        .rows
        .iter()
        .filter(|row| !scope.fitted_only || row.fitted_qty > 0)
        .collect();
    let checked: usize = rows.iter().map(|row| part_count(&row.references)).sum();
    let mut misses = Vec::new();
    for row in rows {
        let designator = row.references.join(",");
        let library_row = row.library_row.as_deref();
        let state = library_row.and_then(|key| row_states.get(key));
        if state.is_some_and(|state| states.contains(state)) {
            continue;
        }
        let clause = part_clause(&designator, library_row, state.map(String::as_str));
        let message = match state {
            Some(_) => format!("{clause}, not {allowed}"),
            None => clause,
        };
        let miss = Miss::new(message)
            .with("designator", json!(designator))
            .with("library_row", json!(row.library_row))
            .with("state", json!(state))
            .with("allowed", json!(states))
            .with("count", json!(part_count(&row.references)))
            .with("parts", json!(checked))
            .with("variant", json!(scope.variant))
            .with("fitted_only", json!(scope.fitted_only))
            .about(format!("part:{designator}"));
        misses.push(miss);
    }
    misses
}

/// What is wrong with one part, as a clause: it was placed from no library
/// row, its row is in no open library, or the row's state.
fn part_clause(designator: &str, library_row: Option<&str>, state: Option<&str>) -> String {
    match (library_row, state) {
        (None, _) => format!("{designator} was not placed from a library row"),
        (Some(_), None) => format!("{designator}'s library row is in no open library"),
        (Some(_), Some(state)) => format!("{designator} is {state}"),
    }
}

/// How many parts a BOM line holds: one per reference. A line with no
/// reference still stands for one part.
fn part_count(references: &[String]) -> usize {
    references.len().max(1)
}

/// One BOM line a `parts_state` run found outside the allowed states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartMiss {
    /// The line's references, comma-joined as the BOM writes them.
    pub designator: String,
    /// How many parts the line holds.
    pub count: usize,
    pub problem: PartProblem,
}

/// Why a part has no allowed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartProblem {
    /// It was placed from no library row.
    Unplaced,
    /// Its library row is in no open library.
    NoLibrary,
    /// Its library row is in this state.
    State(String),
}

/// The per-part findings of one `parts_state` run, counted in parts, for
/// the card's one sentence (mockup row 38).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartsShortfall {
    /// Parts outside the allowed states.
    pub missed: usize,
    /// Parts checked.
    pub of: usize,
    pub allowed: Vec<String>,
    pub lines: Vec<PartMiss>,
    /// Only fitted parts were checked, so `of` counts fitted parts.
    pub fitted_only: bool,
}

/// `findings` read as a [`PartsShortfall`]. `None` unless there is at
/// least one finding and every one is a per-part `parts_state` finding.
pub fn parts_shortfall(findings: &[&Finding]) -> Option<PartsShortfall> {
    let first = findings.first()?;
    let lines = findings
        .iter()
        .map(|finding| part_miss(&finding.evidence))
        .collect::<Option<Vec<PartMiss>>>()?;
    let allowed = first
        .evidence
        .get("allowed")
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let missed = lines.iter().map(|line| line.count).sum();
    let of = first
        .evidence
        .get("parts")
        .and_then(Value::as_u64)
        .map_or(missed, |parts| usize::try_from(parts).unwrap_or(usize::MAX))
        .max(missed);
    let fitted_only = first
        .evidence
        .get("fitted_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(PartsShortfall {
        missed,
        of,
        allowed,
        lines,
        fitted_only,
    })
}

/// One per-part finding's evidence as a [`PartMiss`].
fn part_miss(evidence: &Value) -> Option<PartMiss> {
    if evidence.get("kind").and_then(Value::as_str) != Some("parts_state") {
        return None;
    }
    let designator = evidence.get("designator")?.as_str()?.to_string();
    let count = evidence
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(1)
        .max(1);
    let library_row = evidence.get("library_row").and_then(Value::as_str);
    let state = evidence.get("state").and_then(Value::as_str);
    let problem = match (library_row, state) {
        (None, _) => PartProblem::Unplaced,
        (Some(_), None) => PartProblem::NoLibrary,
        (Some(_), Some(state)) => PartProblem::State(state.to_string()),
    };
    Some(PartMiss {
        designator,
        count,
        problem,
    })
}
