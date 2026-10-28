//! Encoded TSV lines of table rows, kept between row writes (#416).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::{LibraryRow, LibraryTable};

/// The encoded line of every row of each table, from the last write that
/// used this cache. An entry is found by the row's `Arc` address and holds
/// that `Arc`, so while it is cached the row can neither change in place
/// (`Arc::make_mut` copies a shared row) nor be freed and its address
/// reused: a hit is always the line of that exact row. A table whose
/// columns changed starts over.
#[derive(Debug, Default)]
pub(crate) struct RowLines {
    tables: HashMap<String, TableLines>,
}

#[derive(Debug, Default)]
struct TableLines {
    columns: Vec<String>,
    lines: HashMap<usize, (Arc<LibraryRow>, String)>,
}

impl RowLines {
    /// Drop every line, and the rows they keep alive.
    pub(crate) fn clear(&mut self) {
        self.tables.clear();
    }

    /// Append the rows of `table` at `order`, one line each, taking a
    /// row's line from the cache or making it with `encode` and caching it;
    /// lines of rows no longer in the table are dropped.
    pub(super) fn push_rows(
        &mut self,
        name: &str,
        table: &LibraryTable,
        order: &[usize],
        out: &mut String,
        encode: fn(&mut String, &[String], &LibraryRow),
    ) {
        let entry = self.tables.entry(name.to_string()).or_default();
        if entry.columns != table.columns {
            entry.columns.clone_from(&table.columns);
            entry.lines.clear();
        }
        let mut fresh = Vec::new();
        for &i in order {
            let row = &table.rows[i];
            let key = Arc::as_ptr(row) as usize;
            match entry.lines.get(&key) {
                Some((_, line)) => out.push_str(line),
                None => {
                    let mut line = String::new();
                    encode(&mut line, &table.columns, row);
                    out.push_str(&line);
                    fresh.push((key, Arc::clone(row), line));
                }
            }
            out.push('\n');
        }
        for (key, row, line) in fresh {
            entry.lines.insert(key, (row, line));
        }
        // Rows replaced or removed since the last write: let them go.
        if entry.lines.len() > order.len() {
            let live: HashSet<usize> = order
                .iter()
                .map(|&i| Arc::as_ptr(&table.rows[i]) as usize)
                .collect();
            entry.lines.retain(|key, _| live.contains(key));
        }
    }

    /// Keep only the lines of rows `lf` still holds, under unchanged
    /// columns.
    pub(crate) fn retain_rows(&mut self, lf: &super::LibraryFile) {
        self.tables.retain(|name, entry| {
            let Some(table) = lf.tables.get(name) else {
                return false;
            };
            if entry.columns != table.columns {
                return false;
            }
            let live: HashSet<usize> = table.rows.iter().map(|r| Arc::as_ptr(r) as usize).collect();
            entry.lines.retain(|key, _| live.contains(key));
            true
        });
    }

    /// Forget the tables `keep` says the document no longer has.
    pub(super) fn retain_tables(&mut self, keep: impl Fn(&str) -> bool) {
        self.tables.retain(|name, _| keep(name));
    }
}
