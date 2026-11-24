//! `parts_state`: every part the BOM buys was placed from a library row in
//! one of the allowed lifecycle states. A part with no library row, or with
//! one no open library holds, has no state and does not pass.

use std::collections::HashMap;

use serde_json::{Value, json};
use signex_bom::BomTable;
use signex_types::check::Finding;

pub use signex_types::project::LIFECYCLE_STATES;

use crate::Miss;

/// `parts_state`. `parts` is the ungrouped BOM (one row per part).
pub(crate) fn parts_state(
    parts: Option<&Result<BomTable, String>>,
    row_states: &HashMap<String, String>,
    states: &[String],
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
    let mut misses = Vec::new();
    for row in &table.rows {
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
            .with("parts", json!(table.rows.len()))
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

/// The per-part findings of one `parts_state` run as one sentence: how many
/// of the parts are outside the allowed states, then what each of them is
/// (mockup row 38). `None` unless every finding is a per-part one.
pub(crate) fn parts_sentence(findings: &[&Finding]) -> Option<String> {
    let first = findings.first()?;
    let clauses = findings
        .iter()
        .map(|finding| {
            let evidence = &finding.evidence;
            if evidence.get("kind").and_then(Value::as_str) != Some("parts_state") {
                return None;
            }
            let designator = evidence.get("designator")?.as_str()?;
            let library_row = evidence.get("library_row").and_then(Value::as_str);
            let state = evidence.get("state").and_then(Value::as_str);
            Some(part_clause(designator, library_row, state))
        })
        .collect::<Option<Vec<String>>>()?;
    let allowed: Vec<&str> = first
        .evidence
        .get("allowed")
        .and_then(Value::as_array)
        .map(|states| states.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let missed = clauses.len();
    let of = first
        .evidence
        .get("parts")
        .and_then(Value::as_u64)
        .map_or(missed, |parts| parts as usize)
        .max(missed);
    let verb = if missed == 1 { "is" } else { "are" };
    Some(format!(
        "{missed} of {of} parts {verb} not {}: {}.",
        joined(&allowed, "or"),
        joined(&clauses, "and"),
    ))
}

/// `items` as a list read aloud: `a, b and c`.
fn joined<S: AsRef<str>>(items: &[S], last: &str) -> String {
    match items {
        [] => String::new(),
        [only] => only.as_ref().to_string(),
        [head @ .., tail] => {
            let head: Vec<&str> = head.iter().map(AsRef::as_ref).collect();
            format!("{} {last} {}", head.join(", "), tail.as_ref())
        }
    }
}
