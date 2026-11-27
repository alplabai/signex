//! What importing a parameter table file changes on the listed sheets.

use std::collections::HashSet;

use uuid::Uuid;

use super::{CsvPart, ListedPart, ListedSheet, ParameterCsv};

/// One parameter an import writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterChange {
    /// The sheet the part is on: its index in the listed sheets.
    pub sheet: usize,
    pub symbol: Uuid,
    /// The designator the table names the part by.
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
        let mut parts: Vec<(usize, Uuid)> =
            self.changes.iter().map(|c| (c.sheet, c.symbol)).collect();
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

/// Every listed part, with the index of its sheet, sheet by sheet.
struct Listed<'p, 'a> {
    sheets: &'p [ListedSheet<'a>],
    parts: Vec<(usize, &'p ListedPart<'a>)>,
}

impl<'p, 'a> Listed<'p, 'a> {
    fn new(sheets: &'p [ListedSheet<'a>]) -> Self {
        let parts = (sheets.iter().enumerate())
            .flat_map(|(i, sheet)| sheet.parts.iter().map(move |part| (i, part)))
            .collect();
        Self { sheets, parts }
    }

    /// Whether `row` may go to a part of the sheet at `index`: a line with a
    /// Sheet cell only to that sheet's parts, any other to every sheet's.
    fn on_sheet(&self, row: &CsvPart, index: usize) -> bool {
        row.sheet.is_empty() || self.sheets[index].name == row.sheet
    }

    /// The one part `row`'s unique id names among the parts it may go to.
    /// An id two such parts carry (a sheet file copied, read with no Sheet
    /// cell) names none, so no line is written to a guessed sheet.
    fn named_by_id(&self, row: &CsvPart) -> Option<usize> {
        let id = Uuid::parse_str(&row.unique_id).ok()?;
        let mut hits = (self.parts.iter().enumerate())
            .filter(|(_, (sheet, part))| part.symbol.uuid == id && self.on_sheet(row, *sheet))
            .map(|(at, _)| at);
        let first = hits.next()?;
        hits.next().is_none().then_some(first)
    }
}

/// What `csv` changes on the parts of `sheets` (the listed sheets, each with
/// its listed parts in sheet order).
///
/// A line with a Sheet cell goes only to a part of the listed sheet of that
/// name; a line without one to a part of any listed sheet. Within that, a
/// line goes to a part by its Unique Id. A line with no unique id goes by
/// its reference to a part no unique id named: the first such line naming a
/// reference to the first such part with it, the next to the next, so the
/// units of one part each take their own line. A unique id that names no
/// listed part, names one an earlier line took, or names one part on each of
/// two sheets the line may go to leaves its line unmatched. A cell changes a
/// parameter whose value differs; an empty cell empties a parameter the part
/// has and adds none; a missing cell, a line with no key and a parameter the
/// file has no column for are left alone. Nothing is ever removed.
pub fn plan_import(sheets: &[ListedSheet<'_>], csv: &ParameterCsv) -> ImportPlan {
    let listed = Listed::new(sheets);
    let named: HashSet<usize> = (csv.rows.iter())
        .filter(|row| !row.unique_id.is_empty())
        .filter_map(|row| listed.named_by_id(row))
        .collect();
    let mut plan = ImportPlan::default();
    let mut taken: HashSet<usize> = HashSet::new();
    for row in &csv.rows {
        let at = if row.unique_id.is_empty() {
            if row.reference.is_empty() {
                continue;
            }
            (listed.parts.iter().enumerate())
                .find(|(at, (sheet, part))| {
                    part.reference == row.reference
                        && listed.on_sheet(row, *sheet)
                        && !named.contains(at)
                        && !taken.contains(at)
                })
                .map(|(at, _)| at)
        } else {
            listed.named_by_id(row)
        };
        match at.filter(|at| taken.insert(*at)) {
            Some(at) => {
                let (sheet, part) = listed.parts[at];
                plan_line(&mut plan, sheet, part, csv, row);
            }
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

/// The changes `row` makes on `part`, on the sheet at index `sheet`.
fn plan_line(
    plan: &mut ImportPlan,
    sheet: usize,
    part: &ListedPart<'_>,
    csv: &ParameterCsv,
    row: &CsvPart,
) {
    let symbol = part.symbol;
    for (key, value) in csv.keys.iter().zip(&row.values) {
        let Some(after) = value else {
            continue;
        };
        let before = symbol.fields.get(key);
        let is_same = before.map_or(after.is_empty(), |before| before == after);
        if !is_same {
            plan.changes.push(ParameterChange {
                sheet,
                symbol: symbol.uuid,
                reference: part.reference.to_string(),
                key: key.clone(),
                before: before.cloned(),
                after: after.clone(),
            });
        }
    }
}
