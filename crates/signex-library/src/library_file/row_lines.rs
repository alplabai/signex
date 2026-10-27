//! Encoded TSV lines of table rows, kept between row writes (#416).

use std::collections::HashMap;
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
    /// row's line from the cache or making it with `encode`; then keep
    /// exactly these lines for the next write.
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
        let mut cached = std::mem::take(&mut entry.lines);
        let mut kept = HashMap::with_capacity(order.len());
        for &i in order {
            let row = &table.rows[i];
            let key = Arc::as_ptr(row) as usize;
            let line = match cached.remove(&key) {
                Some((_, line)) => line,
                None => {
                    let mut line = String::new();
                    encode(&mut line, &table.columns, row);
                    line
                }
            };
            out.push_str(&line);
            out.push('\n');
            kept.insert(key, (Arc::clone(row), line));
        }
        entry.lines = kept;
    }

    /// Forget the tables `keep` says the document no longer has.
    pub(super) fn retain_tables(&mut self, keep: impl Fn(&str) -> bool) {
        self.tables.retain(|name, _| keep(name));
    }
}
