//! What importing a parameter table file changes on a sheet.

use std::collections::HashMap;

use signex_types::schematic::Symbol;
use uuid::Uuid;

use super::ParameterCsv;

/// One parameter an import writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterChange {
    pub symbol: Uuid,
    pub reference: String,
    pub key: String,
    /// The value now; `None` when the part has no such parameter.
    pub before: Option<String>,
    pub after: String,
}

/// The changes an import makes, and the lines it could not place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportPlan {
    /// In file order: line by line, column by column.
    pub changes: Vec<ParameterChange>,
    /// The references of the lines no listed part takes, each once, in file
    /// order.
    pub unmatched: Vec<String>,
}

impl ImportPlan {
    /// How many parts the changes touch.
    pub fn part_count(&self) -> usize {
        let mut parts: Vec<Uuid> = self.changes.iter().map(|c| c.symbol).collect();
        parts.sort_unstable();
        parts.dedup();
        parts.len()
    }
}

/// What `csv` changes on `parts` (the listed parts, in sheet order).
///
/// A line goes to a part by its reference: the n-th line naming a reference
/// to the n-th listed part with it, so the units of one part, which share a
/// reference, each take their own line as an export wrote them. A cell
/// changes a parameter whose value differs; an empty cell empties a
/// parameter the part has and adds none; a missing cell, a line with no
/// reference and a parameter the file has no column for are left alone.
/// Nothing is ever removed.
pub fn plan_import(parts: &[&Symbol], csv: &ParameterCsv) -> ImportPlan {
    let mut plan = ImportPlan::default();
    let mut taken: HashMap<&str, usize> = HashMap::new();
    for row in &csv.rows {
        if row.reference.is_empty() {
            continue;
        }
        let nth = taken.entry(row.reference.as_str()).or_insert(0);
        let part = parts
            .iter()
            .filter(|p| p.reference == row.reference)
            .nth(*nth);
        *nth += 1;
        let Some(part) = part else {
            if !plan.unmatched.contains(&row.reference) {
                plan.unmatched.push(row.reference.clone());
            }
            continue;
        };
        for (key, value) in csv.keys.iter().zip(&row.values) {
            let Some(after) = value else {
                continue;
            };
            let before = part.fields.get(key);
            let is_same = before.map_or(after.is_empty(), |before| before == after);
            if !is_same {
                plan.changes.push(ParameterChange {
                    symbol: part.uuid,
                    reference: part.reference.clone(),
                    key: key.clone(),
                    before: before.cloned(),
                    after: after.clone(),
                });
            }
        }
    }
    plan
}
