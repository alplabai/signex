//! `LibraryFile` — on-disk representation of a `.snxlib` file.
//!
//! Per `v0.9-snxlib-as-file-plan.md` §1, a Signex component library is a
//! directory whose `.snxlib` file is the user-facing entry point. The file
//! is a TOML document combining a small manifest header with one
//! `[tables.<name>]` block per user-defined component category. Each table
//! holds raw TSV inside a TOML literal multi-line string so the data is
//! line-diffable in git and editable in any spreadsheet.
//!
//! Stage 1 (this module) lands the storage shape only:
//!  * [`SnxlibManifest`] — the manifest header (everything outside the
//!    `[tables.*]` blocks). `library_id` lives at the document root, not
//!    inside `[library]`, matching the v0.9 format.
//!  * [`LibraryTable`] / [`LibraryRow`] — the parsed in-memory view of
//!    one `[tables.<name>]` block. The TSV header row defines the schema;
//!    columns are user-defined per table.
//!  * [`LibraryFile`] — the top-level type combining a manifest header
//!    with a map of parsed tables. [`LibraryFile::parse`] and
//!    [`LibraryFile::write`] are the round-trip entrypoints.
//!
//! Reserved-core-column validation, typed accessors for fields like
//! `row_id` / `version` / `released`, and the cascade engine are later
//! stages of `v0.9-snxlib-as-file-plan.md` — Stage 1 only establishes
//! the storage shape and the round-trip contract.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapter::LibraryError;
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

/// Format token written at the top of every `.snxlib`. Bumping this is a
/// wire-format break — older Signex versions refuse to open the file.
pub const FORMAT_TOKEN: &str = "snxlib/1";

/// Top-level on-disk shape of a `.snxlib` file.
///
/// Construct via [`LibraryFile::parse`]; emit via [`LibraryFile::write`].
/// The two operations form a round-trip: `parse(write(x))` returns a
/// value equal to `x`.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryFile {
    pub manifest: SnxlibManifest,
    /// User-defined component categories — key is the table's name
    /// (`"resistors"` → `[tables.resistors]` on disk). `BTreeMap`'s
    /// sorted iteration drives deterministic write output for clean
    /// git diffs.
    pub tables: BTreeMap<String, LibraryTable>,
}

/// Manifest header — everything in a `.snxlib` *except* the
/// `[tables.*]` blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnxlibManifest {
    /// Wire-format token. Always [`FORMAT_TOKEN`] for files this build can
    /// read; [`LibraryFile::parse`] errors with [`LibraryFileError::UnsupportedFormat`]
    /// otherwise.
    pub format: String,
    /// Library identity. Hidden UUID v7 minted at `init` time.
    pub library_id: Uuid,
    pub library: LibrarySection,
    #[serde(default)]
    pub mode: LibraryMode,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    #[serde(default)]
    pub users: UsersConfig,
}

/// `[library]` block — human-readable name + description. The `library_id`
/// lives at the TOML root, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibrarySection {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Parsed in-memory view of one `[tables.<name>]` block.
///
/// `columns` is the TSV header row, in declaration order (preserved
/// across round-trip so column reordering is a deliberate user action).
/// Each [`LibraryRow`] stores its values keyed by column name; the
/// shared [`LibraryTable::columns`] is the schema.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryTable {
    pub columns: Vec<String>,
    pub rows: Vec<LibraryRow>,
}

/// One row inside a [`LibraryTable`]. Cell lookup is by column name —
/// see [`LibraryTable::cell`] for a schema-aware accessor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryRow {
    pub cells: BTreeMap<String, String>,
}

impl LibraryTable {
    /// Look up `column` in `row.cells`, scoped to this table's schema.
    /// Returns `None` if the column isn't in the table's header — even
    /// if `row` happens to have an entry under that key.
    pub fn cell<'a>(&self, row: &'a LibraryRow, column: &str) -> Option<&'a str> {
        if self.columns.iter().any(|c| c == column) {
            row.cells.get(column).map(String::as_str)
        } else {
            None
        }
    }
}

/// Errors from [`LibraryFile::parse`] / [`LibraryFile::write`].
#[derive(Debug, thiserror::Error)]
pub enum LibraryFileError {
    #[error("toml parse: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error(
        "unsupported .snxlib format token {got:?} (this build expects {expected:?})",
        expected = FORMAT_TOKEN
    )]
    UnsupportedFormat { got: String },
    #[error("table {table:?}: TSV is empty (no header row)")]
    EmptyTable { table: String },
    #[error("table {table:?}: header row contains duplicate column {column:?}")]
    DuplicateColumn { table: String, column: String },
    #[error("table {table:?}: row {row_index} has {got} cells, header declares {expected}")]
    CellCountMismatch {
        table: String,
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error(
        "table {table:?}: row {row_index} cell {column:?} contains a tab or newline; \
         TSV cells must be free of \\t and \\n"
    )]
    DisallowedControlInCell {
        table: String,
        row_index: usize,
        column: String,
    },
    #[error(
        "table {table:?}: column {column:?} name contains a tab or newline; \
         TSV column names must be free of \\t and \\n"
    )]
    DisallowedControlInColumn { table: String, column: String },
    #[error(
        "table {table:?}: cell {column:?} contains the literal triple single-quote \
         (\"'''\") which would terminate the embedded TOML literal multi-line string"
    )]
    DisallowedTripleQuoteInCell { table: String, column: String },
}

impl From<LibraryFileError> for LibraryError {
    /// Funnel `.snxlib` parse / write failures into the adapter's error
    /// channel so callers can `?` through `LibraryAdapter` methods without
    /// matching on the inner enum.
    fn from(value: LibraryFileError) -> Self {
        LibraryError::Backend(value.to_string())
    }
}

impl LibraryFile {
    /// Parse a `.snxlib` TOML document. Validates the format token, then
    /// parses each `[tables.<name>]` block's embedded TSV into rows.
    pub fn parse(text: &str) -> Result<Self, LibraryFileError> {
        // Intermediate shape — captures both the manifest header fields and
        // the raw TSV strings in one TOML deserialization pass. The split
        // back into [`SnxlibManifest`] + parsed `tables` happens after we
        // hand the TSV strings to [`parse_tsv`].
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
            tables: BTreeMap<String, RawTable>,
        }
        #[derive(Deserialize)]
        struct RawTable {
            tsv: String,
        }

        let raw: Raw = toml::from_str(text)?;
        if raw.format != FORMAT_TOKEN {
            return Err(LibraryFileError::UnsupportedFormat { got: raw.format });
        }

        let mut tables = BTreeMap::new();
        for (name, body) in raw.tables {
            let table = parse_tsv(&name, &body.tsv)?;
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

    Ok(LibraryTable { columns, rows })
}

/// Serialize a [`LibraryTable`] back to TSV text. Always ends with `\n`.
fn serialize_tsv(table: &LibraryTable) -> String {
    let mut out = String::new();
    out.push_str(&table.columns.join("\t"));
    out.push('\n');
    for row in &table.rows {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_manifest() -> SnxlibManifest {
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::parse_str("0192a8c0-0000-7000-8000-000000000000").unwrap(),
            library: LibrarySection {
                name: "MyLib".into(),
                description: Some("test library".into()),
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
        }
    }

    fn make_row(columns: &[&str], values: &[&str]) -> LibraryRow {
        let mut row = LibraryRow::default();
        for (col, val) in columns.iter().zip(values) {
            row.cells.insert((*col).to_string(), (*val).to_string());
        }
        row
    }

    fn fixture_table_resistors() -> LibraryTable {
        let columns = vec![
            "row_id".to_string(),
            "internal_pn".to_string(),
            "mpn".to_string(),
            "manufacturer".to_string(),
            "value".to_string(),
            "package".to_string(),
        ];
        let cols_ref: Vec<&str> = columns.iter().map(String::as_str).collect();
        let rows = vec![
            make_row(
                &cols_ref,
                &["r1uuid", "R-001", "RC0603FR-0710KL", "Yageo", "10k", "0603"],
            ),
            make_row(
                &cols_ref,
                &["r2uuid", "R-002", "RC0603FR-074K7L", "Yageo", "4k7", "0603"],
            ),
        ];
        LibraryTable { columns, rows }
    }

    /// The foundational round-trip — a library with no tables still
    /// preserves the manifest header byte-equal under `parse(write())`.
    #[test]
    fn round_trip_no_tables() {
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables: BTreeMap::new(),
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// Single-table round-trip with the columns + rows from the plan
    /// example.
    #[test]
    fn round_trip_single_table() {
        let mut tables = BTreeMap::new();
        tables.insert("resistors".to_string(), fixture_table_resistors());
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// Two tables with *different* column schemas — proves columns are
    /// user-defined per table, not enforced library-wide.
    #[test]
    fn round_trip_multiple_tables_user_defined_columns() {
        let mut tables = BTreeMap::new();
        tables.insert("resistors".to_string(), fixture_table_resistors());

        let regulator_columns = vec![
            "row_id".to_string(),
            "mpn".to_string(),
            "manufacturer".to_string(),
            "package".to_string(),
            "iout_max_a".to_string(),
        ];
        let cols_ref: Vec<&str> = regulator_columns.iter().map(String::as_str).collect();
        let rows = vec![make_row(
            &cols_ref,
            &["u1uuid", "LM7805", "TI", "TO-220", "1.5"],
        )];
        tables.insert(
            "regulators_5v".to_string(),
            LibraryTable {
                columns: regulator_columns,
                rows,
            },
        );

        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// Idempotence — two consecutive writes produce byte-equal output.
    #[test]
    fn write_is_byte_idempotent_under_round_trip() {
        let mut tables = BTreeMap::new();
        tables.insert("resistors".to_string(), fixture_table_resistors());
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let first = lib.write().unwrap();
        let parsed = LibraryFile::parse(&first).unwrap();
        let second = parsed.write().unwrap();
        assert_eq!(first, second);
    }

    /// Format token mismatches are loud (rather than silently parsing as
    /// a current-format file with missing fields).
    #[test]
    fn parse_rejects_unsupported_format_token() {
        let s = r#"format = "snxlib/0"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"
"#;
        let err = LibraryFile::parse(s).unwrap_err();
        assert!(matches!(err, LibraryFileError::UnsupportedFormat { .. }));
    }

    #[test]
    fn parse_rejects_duplicate_columns() {
        let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.dup]
tsv = '''
row_id	mpn	mpn
1	a	b
'''
"#;
        let err = LibraryFile::parse(s).unwrap_err();
        assert!(matches!(err, LibraryFileError::DuplicateColumn { .. }));
    }

    #[test]
    fn parse_rejects_cell_count_mismatch() {
        let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.bad]
tsv = '''
row_id	mpn	manufacturer
only_two_cells	missing
'''
"#;
        let err = LibraryFile::parse(s).unwrap_err();
        assert!(matches!(err, LibraryFileError::CellCountMismatch { .. }));
    }

    /// Header-only TSV (no body rows) parses to an empty `rows` vec.
    /// This is the shape an empty user-created table has on disk
    /// before any rows are added.
    #[test]
    fn header_only_tsv_is_empty_rows() {
        let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.empty]
tsv = '''
row_id	mpn
'''
"#;
        let lib = LibraryFile::parse(s).unwrap();
        let t = &lib.tables["empty"];
        assert_eq!(t.columns, vec!["row_id", "mpn"]);
        assert!(t.rows.is_empty());
    }

    /// Empty TSV (no header at all) is rejected — there's no schema to
    /// hang the table off.
    #[test]
    fn parse_rejects_empty_tsv() {
        let s = r#"format = "snxlib/1"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[library]
name = "X"

[tables.empty]
tsv = '''
'''
"#;
        let err = LibraryFile::parse(s).unwrap_err();
        assert!(matches!(err, LibraryFileError::EmptyTable { .. }));
    }

    #[test]
    fn write_rejects_cell_with_tab() {
        let mut tables = BTreeMap::new();
        tables.insert(
            "t".into(),
            LibraryTable {
                columns: vec!["a".into()],
                rows: vec![make_row(&["a"], &["has\ttab"])],
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let err = lib.write().unwrap_err();
        assert!(matches!(err, LibraryFileError::DisallowedControlInCell { .. }));
    }

    #[test]
    fn write_rejects_cell_with_newline() {
        let mut tables = BTreeMap::new();
        tables.insert(
            "t".into(),
            LibraryTable {
                columns: vec!["a".into()],
                rows: vec![make_row(&["a"], &["has\nnewline"])],
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let err = lib.write().unwrap_err();
        assert!(matches!(err, LibraryFileError::DisallowedControlInCell { .. }));
    }

    #[test]
    fn write_rejects_cell_with_triple_single_quote() {
        let mut tables = BTreeMap::new();
        tables.insert(
            "t".into(),
            LibraryTable {
                columns: vec!["a".into()],
                rows: vec![make_row(&["a"], &["trip''' quoted"])],
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let err = lib.write().unwrap_err();
        assert!(matches!(
            err,
            LibraryFileError::DisallowedTripleQuoteInCell { .. }
        ));
    }

    #[test]
    fn write_rejects_column_name_with_tab() {
        let mut tables = BTreeMap::new();
        tables.insert(
            "t".into(),
            LibraryTable {
                columns: vec!["bad\tcol".into()],
                rows: vec![],
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let err = lib.write().unwrap_err();
        assert!(matches!(
            err,
            LibraryFileError::DisallowedControlInColumn { .. }
        ));
    }

    /// A direct port of the §1 plan example. Anchors the format against
    /// the spec doc so a future schema change has to update both.
    #[test]
    fn parse_example_from_plan() {
        let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000123\"\n\
                 \n\
                 [library]\n\
                 name = \"Loratis-SN-lib\"\n\
                 description = \"\"\n\
                 \n\
                 [tables.resistors]\n\
                 tsv = '''\n\
                 internal_pn\tmpn\tmanufacturer\tvalue\ttolerance\tpackage\n\
                 R-001\tRC0603FR-0710KL\tYageo\t10k\t1%\t0603\n\
                 R-002\tRC0603FR-074K7L\tYageo\t4k7\t1%\t0603\n\
                 '''\n\
                 \n\
                 [tables.regulators_5v]\n\
                 tsv = '''\n\
                 internal_pn\tmpn\tmanufacturer\tpackage\tiout_max_a\n\
                 LIB-005\tLM7805\tTI\tTO-220\t1.5\n\
                 LIB-006\tLM317\tTI\tTO-220\t1.5\n\
                 '''\n";
        let lib = LibraryFile::parse(s).unwrap();
        assert_eq!(lib.manifest.library.name, "Loratis-SN-lib");
        assert_eq!(lib.tables.len(), 2);
        let r = &lib.tables["resistors"];
        assert_eq!(
            r.columns,
            vec!["internal_pn", "mpn", "manufacturer", "value", "tolerance", "package"]
        );
        assert_eq!(r.rows.len(), 2);
        assert_eq!(r.rows[0].cells["mpn"], "RC0603FR-0710KL");
        assert_eq!(r.rows[1].cells["value"], "4k7");

        let u = &lib.tables["regulators_5v"];
        assert_eq!(
            u.columns,
            vec!["internal_pn", "mpn", "manufacturer", "package", "iout_max_a"]
        );
        assert_eq!(u.rows[0].cells["mpn"], "LM7805");
    }

    /// `LibraryTable::cell` returns `None` for out-of-schema columns even
    /// when the row's `cells` map has an entry — protects against silent
    /// schema-drift bugs where a row is written under a column the table
    /// doesn't declare.
    #[test]
    fn cell_accessor_is_schema_scoped() {
        let table = LibraryTable {
            columns: vec!["a".into()],
            rows: vec![],
        };
        let mut row = LibraryRow::default();
        row.cells.insert("a".into(), "alpha".into());
        row.cells.insert("ghost".into(), "should not surface".into());
        assert_eq!(table.cell(&row, "a"), Some("alpha"));
        assert_eq!(table.cell(&row, "ghost"), None);
    }

    /// Empty cells preserve through round-trip — common in optional
    /// columns like `description` or `tags`.
    #[test]
    fn empty_cells_round_trip() {
        let columns = vec!["mpn".to_string(), "tags".to_string()];
        let rows = vec![
            make_row(&["mpn", "tags"], &["LM317", ""]),
            make_row(&["mpn", "tags"], &["", "automotive"]),
        ];
        let mut tables = BTreeMap::new();
        tables.insert("misc".to_string(), LibraryTable { columns, rows });
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// Column order from the header is preserved through round-trip.
    #[test]
    fn column_order_preserved() {
        let columns = vec![
            "z_col".to_string(),
            "a_col".to_string(),
            "m_col".to_string(),
        ];
        let rows = vec![make_row(&["z_col", "a_col", "m_col"], &["1", "2", "3"])];
        let mut tables = BTreeMap::new();
        tables.insert(
            "ordered".to_string(),
            LibraryTable {
                columns: columns.clone(),
                rows,
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(back.tables["ordered"].columns, columns);
    }
}
