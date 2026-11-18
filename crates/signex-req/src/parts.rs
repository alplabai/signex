//! `parts_state`: every part the BOM buys was placed from a library row in
//! one of the allowed lifecycle states. A part with no library row, or with
//! one no open library holds, has no state and does not pass.

use std::collections::HashMap;

use serde_json::json;
use signex_bom::BomTable;

use crate::Miss;

/// The lifecycle state names a requirement may list.
pub const LIFECYCLE_STATES: [&str; 5] =
    ["draft", "in_review", "released", "deprecated", "obsolete"];

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
        let state = row.library_row.as_ref().and_then(|key| row_states.get(key));
        let message = match (&row.library_row, state) {
            (None, _) => format!("{designator} was not placed from a library row"),
            (Some(_), None) => format!("{designator}'s library row is in no open library"),
            (Some(_), Some(state)) if states.contains(state) => continue,
            (Some(_), Some(state)) => format!("{designator} is {state}, not {allowed}"),
        };
        let miss = Miss::new(message)
            .with("designator", json!(designator))
            .with("library_row", json!(row.library_row))
            .with("state", json!(state))
            .with("allowed", json!(states))
            .about(format!("part:{designator}"));
        misses.push(miss);
    }
    misses
}
