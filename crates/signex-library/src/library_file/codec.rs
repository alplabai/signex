//! `.snxlib` TOML+TSV parse / write codec for `LibraryFile`.

use super::*;

impl LibraryFile {
    /// Parse a `.snxlib` TOML document. Validates the format token, then
    /// parses each `[tables.<name>]` block's embedded TSV into rows.
    pub fn parse(text: &str) -> Result<Self, LibraryFileError> {
        // Intermediate shape — captures the manifest header fields, the
        // raw TSV strings, and any `[tables.<name>.column_types]` sidecar
        // map in one TOML deserialization pass. The split back into
        // [`SnxlibManifest`] + parsed `tables` happens after we hand the
        // TSV strings to [`parse_tsv`] and merge the type sidecar.
        #[derive(Deserialize)]
        struct Raw {
            format: String,
            library_id: Uuid,
            library: LibrarySection,
            #[serde(default)]
            mode: LibraryMode,
            #[serde(default)]
            workflow: WorkflowConfig,
            #[serde(default)]
            users: UsersConfig,
            #[serde(default)]
            classes: Vec<ClassEntry>,
            #[serde(default)]
            tables: BTreeMap<String, RawTable>,
        }
        #[derive(Deserialize)]
        struct RawTable {
            tsv: String,
            #[serde(default)]
            column_types: BTreeMap<String, ColumnType>,
        }

        let raw: Raw = toml::from_str(text)?;
        if raw.format != FORMAT_TOKEN {
            return Err(LibraryFileError::UnsupportedFormat { got: raw.format });
        }

        let mut tables = BTreeMap::new();
        for (name, body) in raw.tables {
            // Reject column_types entries naming columns the TSV header
            // doesn't declare — drift between sidecar and schema is a
            // foot-gun (the UI would render dropdowns for ghost columns).
            let mut table = parse_tsv(&name, &body.tsv)?;
            for ctype_col in body.column_types.keys() {
                if !table.columns.iter().any(|c| c == ctype_col) {
                    return Err(LibraryFileError::ColumnTypeForUnknownColumn {
                        table: name.clone(),
                        column: ctype_col.clone(),
                    });
                }
            }
            table.column_types = body.column_types;
            tables.insert(name, table);
        }

        Ok(LibraryFile {
            manifest: SnxlibManifest {
                format: raw.format,
                library_id: raw.library_id,
                library: raw.library,
                mode: raw.mode,
                workflow: raw.workflow,
                users: raw.users,
                classes: raw.classes,
            },
            tables,
        })
    }

    /// Serialize back to a TOML document. Output is deterministic — calling
    /// `parse` on the result yields a `LibraryFile` equal to `self`.
    ///
    /// The header is emitted via `toml::to_string_pretty`; each table is
    /// appended as a `[tables.<name>]` section with the TSV wrapped in a
    /// TOML literal multi-line string (`'''…'''`) so newlines and
    /// backslashes survive without escaping noise. Tables are emitted in
    /// `BTreeMap` order — sorted by name.
    pub fn write(&self) -> Result<String, LibraryFileError> {
        // Validate cells and column names before emitting. Catching here
        // lets the caller see the offending table/row instead of a
        // surprise round-trip mismatch later.
        for (name, table) in &self.tables {
            for column in &table.columns {
                if column.contains('\t') || column.contains('\n') {
                    return Err(LibraryFileError::DisallowedControlInColumn {
                        table: name.clone(),
                        column: column.clone(),
                    });
                }
            }
            for (idx, row) in table.rows.iter().enumerate() {
                for column in &table.columns {
                    let cell = row.cells.get(column).map(String::as_str).unwrap_or("");
                    if cell.contains('\t') || cell.contains('\n') {
                        return Err(LibraryFileError::DisallowedControlInCell {
                            table: name.clone(),
                            row_index: idx,
                            column: column.clone(),
                        });
                    }
                    if cell.contains("'''") {
                        return Err(LibraryFileError::DisallowedTripleQuoteInCell {
                            table: name.clone(),
                            column: column.clone(),
                        });
                    }
                }
            }
        }

        // Header section — toml-rs reorders root scalars before sub-tables
        // automatically so `format` / `library_id` land at the top.
        let mut out = toml::to_string_pretty(&self.manifest)?;
        // Normalize trailing whitespace to exactly one `\n` so the table
        // separator below is predictable.
        let header_len = out.trim_end_matches('\n').len();
        out.truncate(header_len);
        out.push('\n');

        for (name, table) in &self.tables {
            out.push('\n');
            out.push_str("[tables.");
            out.push_str(name);
            out.push_str("]\n");
            out.push_str("tsv = '''\n");
            out.push_str(&serialize_tsv(table));
            out.push_str("'''\n");

            // Optional `[tables.<name>.column_types]` sub-table — only
            // emitted when at least one column declares a non-default
            // type. Untyped tables stay clean.
            if !table.column_types.is_empty() {
                out.push('\n');
                out.push_str("[tables.");
                out.push_str(name);
                out.push_str(".column_types]\n");
                for (col, ty) in &table.column_types {
                    out.push_str(col);
                    out.push_str(" = \"");
                    out.push_str(&ty.to_token());
                    out.push_str("\"\n");
                }
            }
        }

        Ok(out)
    }
}

/// Parse TSV text into a [`LibraryTable`]. The first non-empty line is
/// the header; subsequent lines are rows.
fn parse_tsv(table_name: &str, tsv: &str) -> Result<LibraryTable, LibraryFileError> {
    // Strip the leading/trailing newlines TOML's literal multi-line strings
    // pad with, but preserve interior structure. We never trim spaces —
    // an empty cell at the start/end of a TSV row is meaningful.
    let trimmed = tsv.trim_matches('\n');
    if trimmed.is_empty() {
        return Err(LibraryFileError::EmptyTable {
            table: table_name.to_string(),
        });
    }

    let mut lines = trimmed.split('\n');
    let header_line = lines.next().ok_or_else(|| LibraryFileError::EmptyTable {
        table: table_name.to_string(),
    })?;
    let columns: Vec<String> = header_line.split('\t').map(str::to_string).collect();

    // Reject duplicate column names — `BTreeMap<String, String>` semantics
    // would silently drop one value, which is a foot-gun for "I added an
    // mpn column twice by mistake."
    let mut seen = HashSet::with_capacity(columns.len());
    for c in &columns {
        if !seen.insert(c.as_str()) {
            return Err(LibraryFileError::DuplicateColumn {
                table: table_name.to_string(),
                column: c.clone(),
            });
        }
    }

    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        if line.is_empty() {
            // Blank rows inside the TSV body are skipped — they're a
            // common artefact of editor "ensure final newline" behaviour
            // and don't carry data.
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != columns.len() {
            return Err(LibraryFileError::CellCountMismatch {
                table: table_name.to_string(),
                row_index: idx,
                got: cells.len(),
                expected: columns.len(),
            });
        }
        let mut row = LibraryRow::default();
        for (col, val) in columns.iter().zip(cells) {
            row.cells.insert(col.clone(), val.to_string());
        }
        rows.push(row);
    }

    Ok(LibraryTable {
        columns,
        rows,
        column_types: BTreeMap::new(),
    })
}

/// Serialize a [`LibraryTable`] back to TSV text. Always ends with `\n`.
///
/// **Canonical row order.** When the table declares a `row_id` column,
/// rows are emitted sorted by `row_id` ascending — regardless of the
/// in-memory `rows` Vec order. This keeps the on-disk file layout
/// stable across UI sort changes, insertion order, and bulk-import
/// order, so `git blame` on a row line points at the engineer who last
/// edited *that row's data* rather than whoever last reordered the
/// catalog.
///
/// Tables without a `row_id` column (rare; legitimate for user-defined
/// lookup-only tables) preserve insertion order — there's no canonical
/// key to sort by.
fn serialize_tsv(table: &LibraryTable) -> String {
    let mut out = String::new();
    out.push_str(&table.columns.join("\t"));
    out.push('\n');

    let has_row_id = table.columns.iter().any(|c| c == "row_id");
    let mut order: Vec<usize> = (0..table.rows.len()).collect();
    if has_row_id {
        order.sort_by(|&a, &b| {
            let ra = table.rows[a]
                .cells
                .get("row_id")
                .map(String::as_str)
                .unwrap_or("");
            let rb = table.rows[b]
                .cells
                .get("row_id")
                .map(String::as_str)
                .unwrap_or("");
            ra.cmp(rb)
        });
    }

    for &i in &order {
        let row = &table.rows[i];
        let cells: Vec<String> = table
            .columns
            .iter()
            .map(|c| row.cells.get(c).cloned().unwrap_or_default())
            .collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}
