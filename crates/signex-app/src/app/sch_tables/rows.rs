//! The rows a bound table shows, built from the design: a part's pins, or
//! the project's pages. Pure functions of plain data; `refresh` feeds them.

use std::collections::HashMap;

use signex_types::schematic::{
    PinSort, SchTable, SchematicSheet, TableColumn, pin_columns, sheet_columns,
};

/// One page of the contents table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContentsEntry {
    pub page: usize,
    /// The sheet's name, with its channel for a channel page.
    pub sheet: String,
    pub title: String,
    pub revision: String,
    pub last_changed: String,
    /// The sheet file the row opens, relative to the sheet holding the table.
    pub file: String,
}

/// One pin of a pin table, before the columns pick from it.
#[derive(Debug, Clone, PartialEq)]
struct PinRow {
    number: String,
    name: String,
    net: String,
    kind: String,
    unit: String,
    length: String,
}

/// The unit letter of unit `n` (1 = A); empty for a part with one unit.
fn unit_letter(n: u32, units: u32) -> String {
    if units <= 1 || n == 0 {
        return String::new();
    }
    let mut n = n;
    let mut out = Vec::new();
    while n > 0 {
        let d = ((n - 1) % 26) as u8;
        out.push(char::from(b'A' + d));
        n = (n - 1) / 26;
    }
    out.iter().rev().collect()
}

/// Natural order: digit runs compare as numbers, so pin 2 sorts before 10.
pub(crate) fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.cmp(&y);
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// The pins of the part `designator` on `sheet` (every unit placed there),
/// with the net each pin is on in the sheet's own connectivity. `None` when
/// no part on the sheet has that designator.
fn pin_rows(sheet: &SchematicSheet, designator: &str, sort: PinSort) -> Option<Vec<PinRow>> {
    let units: Vec<_> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power && s.reference.trim() == designator.trim())
        .collect();
    if units.is_empty() {
        return None;
    }
    let netlist = signex_net::build_netlist(sheet);
    let mut net_of: HashMap<(uuid::Uuid, &str), &str> = HashMap::new();
    for net in &netlist.nets {
        for t in &net.terminals {
            net_of.insert((t.symbol, t.pin.as_str()), net.name.as_str());
        }
    }
    let mut rows: Vec<PinRow> = Vec::new();
    for symbol in units {
        let Some(lib) = sheet.body_of(symbol) else {
            continue;
        };
        let unit_count = lib.pins.iter().map(|p| p.unit).max().unwrap_or(1);
        for lib_pin in lib
            .pins
            .iter()
            .filter(|p| p.unit == 0 || p.unit == symbol.unit)
        {
            let pin = &lib_pin.pin;
            if rows.iter().any(|r| r.number == pin.number) {
                continue;
            }
            rows.push(PinRow {
                number: pin.number.clone(),
                name: pin.name.clone(),
                net: net_of
                    .get(&(symbol.uuid, pin.number.as_str()))
                    .map_or_else(String::new, |n| (*n).to_string()),
                kind: pin.direction.label().to_string(),
                unit: unit_letter(lib_pin.unit, unit_count),
                length: format!("{:.2}", pin.length),
            });
        }
    }
    match sort {
        PinSort::Number => rows.sort_by(|a, b| natural_cmp(&a.number, &b.number)),
        PinSort::Name => rows.sort_by(|a, b| {
            natural_cmp(&a.name, &b.name).then_with(|| natural_cmp(&a.number, &b.number))
        }),
    }
    Some(rows)
}

/// The cell of `row` for the column `key`.
fn pin_cell(row: &PinRow, key: &str) -> String {
    match key {
        pin_columns::PIN => row.number.clone(),
        pin_columns::NAME => row.name.clone(),
        pin_columns::NET => row.net.clone(),
        pin_columns::TYPE => row.kind.clone(),
        pin_columns::UNIT => row.unit.clone(),
        pin_columns::PIN_LENGTH => row.length.clone(),
        _ => String::new(),
    }
}

fn contents_cell(entry: &ContentsEntry, key: &str) -> String {
    match key {
        sheet_columns::PAGE => entry.page.to_string(),
        sheet_columns::SHEET => entry.sheet.clone(),
        sheet_columns::TITLE => entry.title.clone(),
        sheet_columns::REV => entry.revision.clone(),
        sheet_columns::LAST_CHANGED => entry.last_changed.clone(),
        _ => String::new(),
    }
}

/// The cells of the pin table for `designator`, aligned to `columns`;
/// `None` when the part is not on the sheet.
pub(crate) fn pin_table_rows(
    sheet: &SchematicSheet,
    designator: &str,
    sort: PinSort,
    columns: &[TableColumn],
) -> Option<Vec<Vec<String>>> {
    let rows = pin_rows(sheet, designator, sort)?;
    Some(
        rows.iter()
            .map(|row| columns.iter().map(|c| pin_cell(row, &c.key)).collect())
            .collect(),
    )
}

/// The cells and row links of the contents table, aligned to `columns`.
pub(crate) fn contents_table_rows(
    entries: &[ContentsEntry],
    columns: &[TableColumn],
) -> (Vec<Vec<String>>, Vec<String>) {
    let rows = entries
        .iter()
        .map(|e| columns.iter().map(|c| contents_cell(e, &c.key)).collect())
        .collect();
    let links = entries.iter().map(|e| e.file.clone()).collect();
    (rows, links)
}

/// The designators of the parts on `sheet`, in natural order, each once
/// (power ports left out).
pub(crate) fn designators(sheet: &SchematicSheet) -> Vec<String> {
    let mut out: Vec<String> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power && !s.reference.trim().is_empty())
        .map(|s| s.reference.trim().to_string())
        .collect();
    out.sort_by(|a, b| natural_cmp(a, b));
    out.dedup();
    out
}

/// Whether a bound table's source is gone: its part left the sheet.
pub(crate) fn source_is_missing(sheet: &SchematicSheet, table: &SchTable) -> bool {
    match &table.source {
        signex_types::schematic::TableSource::ComponentPins { designator, .. } => !sheet
            .symbols
            .iter()
            .any(|s| !s.is_power && s.reference.trim() == designator.trim()),
        _ => false,
    }
}
