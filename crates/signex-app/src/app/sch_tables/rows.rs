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

/// Whether the stored link `old` names the sheet of the fresh link: the same
/// text, or an old bare file name for a link that now has its folder.
fn same_sheet_link(old: &str, fresh: &str) -> bool {
    old == fresh
        || (!old.contains(['/', '\\'])
            && std::path::Path::new(fresh)
                .file_name()
                .is_some_and(|name| name == old))
}

/// For each old row of `table`, whether a fresh row is that row. Each fresh
/// row answers for one old row, so a file placed twice with one placement
/// unread keeps the other's row. The closest match is taken first: same link
/// and Sheet, same link, an old bare link, then Sheet for a row with no link.
fn rows_reached(
    table: &SchTable,
    rows: &[Vec<String>],
    links: &[String],
    sheet: Option<usize>,
) -> Vec<bool> {
    let same_name = |old: usize, fresh: usize| {
        let cell = |row: Option<&Vec<String>>| sheet.and_then(|c| row?.get(c).cloned());
        let name = cell(table.rows.get(old));
        name.is_some() && name == cell(rows.get(fresh))
    };
    let same_link = |old: usize, fresh: usize| table.link(old) == Some(links[fresh].as_str());
    let bare_link = |old: usize, fresh: usize| {
        table
            .link(old)
            .is_some_and(|link| same_sheet_link(link, &links[fresh]))
    };
    let no_link = |old: usize, fresh: usize| table.link(old).is_none() && same_name(old, fresh);
    let exact = |old: usize, fresh: usize| same_link(old, fresh) && same_name(old, fresh);
    let passes: [&dyn Fn(usize, usize) -> bool; 4] = [&exact, &same_link, &bare_link, &no_link];
    let mut reached = vec![false; table.rows.len()];
    let mut free = vec![true; links.len()];
    for accepts in passes {
        for (old, is_reached) in reached.iter_mut().enumerate() {
            if *is_reached {
                continue;
            }
            if let Some(fresh) = (0..free.len()).find(|&f| free[f] && accepts(old, f)) {
                free[fresh] = false;
                *is_reached = true;
            }
        }
    }
    reached
}

/// `rows` and `links` with every row of `table` the compile did not reach put
/// back at its place, unchanged but for its page number. For a compile that
/// could not read a sheet, or met a sheet symbol whose file is missing: a row
/// it did not reach may be that sheet or one below it, so it is not known to
/// be gone.
pub(crate) fn keeping_unreached_rows(
    table: &SchTable,
    mut rows: Vec<Vec<String>>,
    mut links: Vec<String>,
) -> (Vec<Vec<String>>, Vec<String>) {
    let column = |key: &str| table.columns.iter().position(|c| c.key == key);
    let reached = rows_reached(table, &rows, &links, column(sheet_columns::SHEET));
    for (i, old) in table.rows.iter().enumerate() {
        if !reached[i] {
            let at = i.min(rows.len());
            rows.insert(at, old.clone());
            links.insert(at, table.link(i).unwrap_or_default().to_string());
        }
    }
    if let Some(page) = column(sheet_columns::PAGE) {
        for (i, row) in rows.iter_mut().enumerate() {
            if let Some(cell) = row.get_mut(page) {
                *cell = (i + 1).to_string();
            }
        }
    }
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
