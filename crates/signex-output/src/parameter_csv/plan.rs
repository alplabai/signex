//! What importing a parameter table file changes on a sheet.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::Symbol;
use uuid::Uuid;

use super::{CsvPart, ParameterCsv};

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

impl ParameterChange {
    /// Whether the change adds a parameter the part does not have yet.
    pub fn is_added(&self) -> bool {
        self.before.is_none()
    }
}

/// The changes an import makes, and the lines it could not place.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportPlan {
    /// In file order: line by line, column by column.
    pub changes: Vec<ParameterChange>,
    /// What names each line no listed part takes (its reference, or its
    /// unique id when the reference is blank), each once, in file order.
    pub unmatched: Vec<String>,
    /// How many lines no listed part takes.
    pub unmatched_lines: usize,
}

impl ImportPlan {
    /// How many parts the changes touch.
    pub fn part_count(&self) -> usize {
        let mut parts: Vec<Uuid> = self.changes.iter().map(|c| c.symbol).collect();
        parts.sort_unstable();
        parts.dedup();
        parts.len()
    }

    /// How many changes add a parameter.
    pub fn added_count(&self) -> usize {
        self.changes.iter().filter(|c| c.is_added()).count()
    }

    /// How many changes write a parameter the part already has.
    pub fn changed_count(&self) -> usize {
        self.changes.len() - self.added_count()
    }
}

/// What `csv` changes on `parts` (the listed parts, in sheet order).
///
/// A line goes to a part by its Unique Id. A line with no unique id goes by
/// its reference to a part no unique id named: the n-th such line naming a
/// reference to the n-th such part with it, so the units of one part each
/// take their own line. A unique id that names no listed part, or names one
/// an earlier line took, leaves its line unmatched. A cell changes a
/// parameter whose value differs; an empty cell empties a parameter the part
/// has and adds none; a missing cell, a line with no key and a parameter the
/// file has no column for are left alone. Nothing is ever removed.
pub fn plan_import(parts: &[&Symbol], csv: &ParameterCsv) -> ImportPlan {
    let by_id: HashMap<Uuid, &Symbol> = parts.iter().map(|p| (p.uuid, *p)).collect();
    let named: HashSet<Uuid> = csv
        .rows
        .iter()
        .filter_map(|row| Uuid::parse_str(&row.unique_id).ok())
        .filter(|id| by_id.contains_key(id))
        .collect();
    let mut plan = ImportPlan::default();
    let mut taken_ids: HashSet<Uuid> = HashSet::new();
    let mut taken_references: HashMap<&str, usize> = HashMap::new();
    for row in &csv.rows {
        let part = if row.unique_id.is_empty() {
            if row.reference.is_empty() {
                continue;
            }
            let nth = taken_references.entry(row.reference.as_str()).or_insert(0);
            let part = parts
                .iter()
                .filter(|p| p.reference == row.reference && !named.contains(&p.uuid))
                .nth(*nth)
                .copied();
            *nth += 1;
            part
        } else {
            Uuid::parse_str(&row.unique_id)
                .ok()
                .filter(|id| taken_ids.insert(*id))
                .and_then(|id| by_id.get(&id).copied())
        };
        match part {
            Some(part) => plan_line(&mut plan, part, csv, row),
            None => plan.note_unmatched(row),
        }
    }
    plan
}

impl ImportPlan {
    fn note_unmatched(&mut self, row: &CsvPart) {
        self.unmatched_lines += 1;
        let label = if row.reference.is_empty() {
            &row.unique_id
        } else {
            &row.reference
        };
        if !self.unmatched.contains(label) {
            self.unmatched.push(label.clone());
        }
    }
}

/// The changes `row` makes on `part`.
fn plan_line(plan: &mut ImportPlan, part: &Symbol, csv: &ParameterCsv, row: &CsvPart) {
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
