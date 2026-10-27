//! `.snxlib` TOML+TSV parse / write codec for `LibraryFile`.

use super::*;

impl LibraryFile {
    /// Parse a `.snxlib` TOML document. Validates the format token, then
    /// parses each `[tables.<name>]` block's embedded TSV into rows.
    pub fn parse(text: &str) -> Result<Self, LibraryFileError> {
        Self::parse_with(text, &|_, _| None)
    }

    /// [`Self::parse`], where `repair` may fix a row whose cell count does
    /// not match its table header: given the header and the row's cells it
    /// returns the full set of cells, or `None` to keep the row an error.
    pub fn parse_with(text: &str, repair: RowRepair<'_>) -> Result<Self, LibraryFileError> {
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
            #[serde(default)]
            pricing: Option<toml::Value>,
        }

        let raw: Raw = toml::from_str(text)?;
        if raw.format != FORMAT_TOKEN {
            return Err(LibraryFileError::UnsupportedFormat { got: raw.format });
        }
        // Second, untyped read: tells whether the text holds keys the typed
        // structs do not model (#1435).
        let untyped: toml::Table = toml::from_str(text)?;

        let mut tables = BTreeMap::new();
        for (name, body) in raw.tables {
            // Reject column_types entries naming columns the TSV header
            // doesn't declare — drift between sidecar and schema is a
            // foot-gun (the UI would render dropdowns for ghost columns).
            let mut table = parse_tsv(&name, &body.tsv, repair)?;
            for ctype_col in body.column_types.keys() {
                if !table.columns.iter().any(|c| c == ctype_col) {
                    return Err(LibraryFileError::ColumnTypeForUnknownColumn {
                        table: name.clone(),
                        column: ctype_col.clone(),
                    });
                }
            }
            table.column_types = body.column_types;
            table.pricing = read_pricing(&name, pricing_table(&name, body.pricing))?;
            tables.insert(name, table);
        }

        let mut file = LibraryFile {
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
            layout: SourceLayout::default(),
        };
        file.layout = SourceLayout::of(text, &untyped, &file)?;
        Ok(file)
    }

    /// Serialize back to a TOML document. Output is deterministic — calling
    /// `parse` on the result yields a `LibraryFile` equal to `self`.
    ///
    /// A file parsed from text holding keys this build does not model is
    /// written as that text with only the changed content edited in, so the
    /// unknown keys, the key order and the untouched rows stay as they were
    /// (#1435). Any other file is written by [`Self::write_canonical`].
    pub fn write(&self) -> Result<String, LibraryFileError> {
        self.render().map(|(out, _)| out)
    }

    /// [`Self::write`], then take the written text as the layout later writes
    /// start from, so what this write removed (a class, a user) cannot come
    /// back with a later edit.
    pub fn write_and_rebase(&mut self) -> Result<String, LibraryFileError> {
        let (out, layout) = self.render()?;
        self.layout = layout;
        Ok(out)
    }

    /// Rename class `old_key` to `entry`, keeping the unknown keys the class
    /// carries in the kept text.
    pub fn rename_class(&mut self, old_key: &str, entry: ClassEntry) {
        self.layout.rename_class(old_key, &entry.key);
        if let Some(class) = self.manifest.classes.iter_mut().find(|c| c.key == old_key) {
            *class = entry;
        }
    }

    /// Rename table `old` to `new`, keeping the unknown keys the table carries
    /// in the kept text. Returns false, changing nothing, when there is no
    /// table `old`.
    pub fn rename_table(&mut self, old: &str, new: &str) -> bool {
        let Some(table) = self.tables.remove(old) else {
            return false;
        };
        self.tables.insert(new.to_string(), table);
        self.layout.rename_table(old, new);
        true
    }

    fn render(&self) -> Result<(String, SourceLayout), LibraryFileError> {
        self.validate_for_write()?;
        let canonical = self.write_canonical()?;
        let out = self.layout.render(&canonical)?.unwrap_or(canonical);
        // Self-check: refuse a document our own parser cannot read back, or
        // reads back as something other than `self`, so a bad or lossy table
        // never reaches `atomic_write` / `commit_path` (#687).
        let reread = LibraryFile::parse(&out).map_err(|source| {
            LibraryFileError::WrittenDocumentUnreadable {
                reason: source.to_string(),
            }
        })?;
        if reread.manifest != self.manifest {
            return Err(LibraryFileError::WrittenDocumentDiffers {
                table: "(manifest)".to_string(),
            });
        }
        self.check_tables_survive(&reread)?;
        Ok((out, reread.layout))
    }

    /// The content alone, in the canonical layout.
    ///
    /// The header is emitted via `toml::to_string_pretty`; each table is
    /// appended as a `[tables.<name>]` section with the TSV wrapped in a
    /// TOML literal multi-line string (`'''…'''`) so newlines and
    /// backslashes survive without escaping noise. Tables are emitted in
    /// `BTreeMap` order — sorted by name.
    pub(super) fn write_canonical(&self) -> Result<String, LibraryFileError> {
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
            out.push_str(&toml_key(name));
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
                out.push_str(&toml_key(name));
                out.push_str(".column_types]\n");
                for (col, ty) in &table.column_types {
                    out.push_str(&toml_key(col));
                    out.push_str(" = \"");
                    out.push_str(&ty.to_token());
                    out.push_str("\"\n");
                }
            }

            // Optional cached distributor prices — one sub-table per
            // (row, distributor, part number), only when the table has any.
            write_pricing(&mut out, name, &table.pricing);
        }
        Ok(out)
    }

    /// Fail naming the first table that `reread` (the parse of what
    /// [`Self::write`] just produced) does not reproduce: columns, column
    /// types, and every declared-column cell of every row. Parseable-but-lossy
    /// output would otherwise overwrite a good library file with a damaged one.
    /// Row order is not compared when the table has a `row_id` column, since
    /// the writer canonicalises it; an absent cell equals an empty one, which
    /// is how the TSV block stores it.
    fn check_tables_survive(&self, reread: &LibraryFile) -> Result<(), LibraryFileError> {
        let differs = |table: &str| LibraryFileError::WrittenDocumentDiffers {
            table: table.to_string(),
        };
        if reread.tables.len() != self.tables.len() {
            return Err(differs("(table set)"));
        }
        for (name, table) in &self.tables {
            let Some(back) = reread.tables.get(name) else {
                return Err(differs(name));
            };
            if !same_table_content(table, back) {
                return Err(differs(name));
            }
        }
        Ok(())
    }

    /// Reject column names and cells that cannot live in a TSV block inside
    /// a TOML `'''` literal string. Naming the offending table / row here
    /// beats a surprise parse failure after the file is on disk.
    fn validate_for_write(&self) -> Result<(), LibraryFileError> {
        for (name, table) in &self.tables {
            check_pricing(name, &table.pricing)?;
            for column in &table.columns {
                if has_disallowed_char(column) {
                    return Err(LibraryFileError::DisallowedControlInColumn {
                        table: name.clone(),
                        column: column.clone(),
                    });
                }
                if column.contains("'''") {
                    return Err(LibraryFileError::DisallowedTripleQuoteInColumn {
                        table: name.clone(),
                        column: column.clone(),
                    });
                }
            }
            for (idx, row) in table.rows.iter().enumerate() {
                for column in &table.columns {
                    let cell = row.cells.get(column).map(String::as_str).unwrap_or("");
                    if has_disallowed_char(cell) {
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
        Ok(())
    }
}

/// True for tab (the TSV separator) and every control character TOML
/// forbids inside a literal multi-line string: U+0000-U+001F (newline
/// included, a lone `\r` is not allowed either) and U+007F.
fn has_disallowed_char(text: &str) -> bool {
    text.chars().any(|c| c < '\u{20}' || c == '\u{7f}')
}

/// Render `name` as a TOML key: bare when it only uses `A-Za-z0-9_-`,
/// otherwise a basic (double-quoted) string with every character that TOML
/// forbids raw escaped. Table names and column names are user input, so an
/// unquoted key would make the written file unreadable (#687).
fn toml_key(name: &str) -> String {
    let is_bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if is_bare {
        return name.to_string();
    }
    let mut quoted = String::with_capacity(name.len() + 2);
    quoted.push('"');
    for c in name.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Parse TSV text into a [`LibraryTable`]. The first non-empty line is
/// the header; subsequent lines are rows.
fn parse_tsv(
    table_name: &str,
    tsv: &str,
    repair: RowRepair<'_>,
) -> Result<LibraryTable, LibraryFileError> {
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
        let mut cells: Vec<String> = line.split('\t').map(str::to_string).collect();
        if cells.len() != columns.len() {
            let borrowed: Vec<&str> = cells.iter().map(String::as_str).collect();
            if let Some(fixed) = repair(&columns, &borrowed) {
                cells = fixed;
            }
        }
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
            row.cells.insert(col.clone(), val);
        }
        rows.push(row);
    }

    Ok(LibraryTable {
        columns,
        rows,
        column_types: BTreeMap::new(),
        pricing: BTreeMap::new(),
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

/// The declared-column cells of `table`'s rows, in the order the writer
/// emits them (sorted by `row_id` when that column exists).
fn canonical_rows(table: &LibraryTable) -> Vec<Vec<&str>> {
    let mut rows: Vec<Vec<&str>> = table
        .rows
        .iter()
        .map(|row| {
            table
                .columns
                .iter()
                .map(|c| row.cells.get(c).map(String::as_str).unwrap_or(""))
                .collect()
        })
        .collect();
    if let Some(id_col) = table.columns.iter().position(|c| c == "row_id") {
        rows.sort_by(|a, b| a[id_col].cmp(b[id_col]));
    }
    rows
}

fn same_table_content(a: &LibraryTable, b: &LibraryTable) -> bool {
    a.columns == b.columns
        && a.column_types == b.column_types
        && a.pricing == b.pricing
        && canonical_rows(a) == canonical_rows(b)
}

/// Refuse a cached price the finite-number rule rejects (NaN, infinity,
/// negative, empty breaks) or whose key differs from its part number. Applied
/// on write, so the writer never produces a record the reader would drop.
fn check_pricing(
    table: &str,
    pricing: &BTreeMap<String, crate::pricing::RowPricing>,
) -> Result<(), LibraryFileError> {
    for (row_id, per_distributor) in pricing {
        for (distributor, per_sku) in per_distributor {
            for (sku, cached) in per_sku {
                let refuse = |reason: String| LibraryFileError::InvalidPricing {
                    table: table.to_string(),
                    row_id: row_id.clone(),
                    distributor: distributor.clone(),
                    reason,
                };
                if sku != &cached.sku {
                    return Err(refuse(format!(
                        "filed under {sku:?} but is for part {:?}",
                        cached.sku
                    )));
                }
                cached.validate().map_err(|e| refuse(e.to_string()))?;
            }
        }
    }
    Ok(())
}

/// Read a table's cached prices.
///
/// The cache can be fetched again, so an entry that is damaged at any level
/// (a row that is not a table, a distributor that is not a table, a record
/// with a missing field, bad date, empty breaks, negative price, or filed
/// under the wrong part number) is dropped with one warning naming it, and
/// the library still opens. A non-finite price is different: TOML accepts
/// `nan` / `inf`, and the project-wide rule (#924, #828) is that non-finite
/// numbers are refused at load, so those fail the load like every other
/// format, whatever else is wrong with the record.
fn read_pricing(
    table: &str,
    raw: toml::Table,
) -> Result<BTreeMap<String, crate::pricing::RowPricing>, LibraryFileError> {
    let mut out: BTreeMap<String, crate::pricing::RowPricing> = BTreeMap::new();
    for (row_id, per_distributor) in raw {
        let toml::Value::Table(per_distributor) = per_distributor else {
            warn_dropped(table, &row_id, "", "", "row entry is not a table");
            continue;
        };
        for (distributor, per_sku) in per_distributor {
            let toml::Value::Table(per_sku) = per_sku else {
                warn_dropped(
                    table,
                    &row_id,
                    &distributor,
                    "",
                    "distributor entry is not a table",
                );
                continue;
            };
            for (sku, value) in per_sku {
                if let Some(path) = signex_types::numeric::first_non_finite_toml_path(&value) {
                    return Err(LibraryFileError::InvalidPricing {
                        table: table.to_string(),
                        row_id,
                        distributor,
                        reason: format!("part {sku:?} has a non-finite number at {path}"),
                    });
                }
                let cached: crate::pricing::CachedPricing = match value.try_into() {
                    Ok(c) => c,
                    Err(e) => {
                        warn_dropped(table, &row_id, &distributor, &sku, &e.to_string());
                        continue;
                    }
                };
                if sku != cached.sku {
                    warn_dropped(
                        table,
                        &row_id,
                        &distributor,
                        &sku,
                        &format!("filed under {sku:?} but is for part {:?}", cached.sku),
                    );
                    continue;
                }
                if let Err(e) = cached.validate() {
                    warn_dropped(table, &row_id, &distributor, &sku, &e.to_string());
                    continue;
                }
                let per_sku = out
                    .entry(row_id.clone())
                    .or_default()
                    .entry(distributor.clone())
                    .or_default();
                // Two spellings of one part number: keep one record.
                per_sku.retain(|k, _| !crate::pricing::same_part_number(k, &sku));
                per_sku.insert(sku, cached);
            }
        }
    }
    Ok(out)
}

/// The pricing section as a table. A pricing value that is not a table (a
/// hand edit such as `pricing = "x"`) is dropped with a warning, never fatal.
fn pricing_table(table: &str, raw: Option<toml::Value>) -> toml::Table {
    match raw {
        None => toml::Table::new(),
        Some(toml::Value::Table(t)) => t,
        Some(_) => {
            warn_dropped(table, "", "", "", "the pricing section is not a table");
            toml::Table::new()
        }
    }
}

fn warn_dropped(table: &str, row_id: &str, distributor: &str, sku: &str, reason: &str) {
    tracing::warn!(
        target: "signex::library",
        table = %table,
        row = %row_id,
        distributor = %distributor,
        part = %sku,
        reason = %reason,
        "dropped an unusable cached price; it will be fetched again by Refresh All Pricing"
    );
}

/// Append the `[tables.<name>.pricing.<row_id>.<distributor>.<sku>]`
/// sub-tables. Prices use `{:?}` so a whole number keeps its `.0` and stays a
/// TOML float.
fn write_pricing(
    out: &mut String,
    table: &str,
    pricing: &BTreeMap<String, crate::pricing::RowPricing>,
) {
    for (row_id, per_distributor) in pricing {
        for (distributor, per_sku) in per_distributor {
            for (sku, cached) in per_sku {
                out.push('\n');
                out.push_str(&format!(
                    "[tables.{}.pricing.{}.{}.{}]\n",
                    toml_key(table),
                    toml_key(row_id),
                    toml_key(distributor),
                    toml_key(sku)
                ));
                out.push_str(&format!("sku = {}\n", toml_string(&cached.sku)));
                out.push_str(&format!("currency = {}\n", toml_string(&cached.currency)));
                if let Some(stock) = cached.stock {
                    out.push_str(&format!("stock = {stock}\n"));
                }
                out.push_str(&format!(
                    "fetched_at = {}\n",
                    toml_string(&cached.fetched_at_text())
                ));
                let breaks: Vec<String> = cached
                    .breaks
                    .iter()
                    .map(|b| format!("{{ qty = {}, unit_price = {:?} }}", b.qty, b.unit_price))
                    .collect();
                out.push_str(&format!("breaks = [{}]\n", breaks.join(", ")));
            }
        }
    }
}

/// A TOML basic string, escaped.
fn toml_string(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}
