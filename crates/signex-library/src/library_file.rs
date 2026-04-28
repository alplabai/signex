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
///
/// `column_types` is the optional typed-sidecar map: per-column type
/// declarations that drive UI sort behaviour (numeric sort vs lexical),
/// validation on edit, and rendering hints (checkboxes for bool, drop-
/// downs for enum). Columns without an entry default to
/// [`ColumnType::String`] so untyped tables keep working unchanged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryTable {
    pub columns: Vec<String>,
    pub rows: Vec<LibraryRow>,
    pub column_types: BTreeMap<String, ColumnType>,
}

/// Per-column type declared in `[tables.<name>.column_types]`.
///
/// Drives:
///  * Numeric sort when a column is `Number` / `Int` (so `10k`-style
///    cells still sort as numbers when the column is typed numeric;
///    raw lexical sort gives `1, 10, 100, 2, 200, 25` which is the
///    Altium pain point we explicitly want to avoid).
///  * Validation in the Edit modal — `Bool` accepts only `true`/`false`,
///    `Enum` accepts only the declared values, etc.
///  * Render hints in the Library Browser — right-align numbers,
///    render `Bool` as checkboxes, render `Url` as clickable links,
///    render `Enum` as inline dropdowns.
///
/// Encoded in TOML as a string token (`"string"`, `"number"`, `"int"`,
/// `"bool"`, `"uuid"`, `"date"`, `"datetime"`, `"url"`, `"version"`,
/// `"tags"`, or `"enum:val1,val2,..."`). Keeping the wire format as
/// plain strings avoids inline-table noise in the `.snxlib`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnType {
    /// Free-form text. Default for columns without an entry.
    String,
    /// IEEE 754 double — covers float and integer numeric data.
    Number,
    /// Signed 64-bit integer — for counts and bare integer parameters.
    Int,
    /// `true` / `false`.
    Bool,
    /// UUID v4 / v7 string.
    Uuid,
    /// ISO 8601 date (`YYYY-MM-DD`).
    Date,
    /// ISO 8601 date-time with timezone.
    DateTime,
    /// HTTPS URL string.
    Url,
    /// Semver-style version (`X.Y.Z`).
    Version,
    /// Comma-separated tag tokens.
    Tags,
    /// One of the declared values (rendered as a dropdown).
    Enum(Vec<String>),
}

impl ColumnType {
    /// Encode the type as the string token written in TOML. Unit
    /// variants emit the bare type name; [`ColumnType::Enum`] emits
    /// `"enum:value1,value2,..."` so the wire format is one cell per
    /// row and human-readable in `git diff`.
    pub fn to_token(&self) -> String {
        match self {
            ColumnType::String => "string".into(),
            ColumnType::Number => "number".into(),
            ColumnType::Int => "int".into(),
            ColumnType::Bool => "bool".into(),
            ColumnType::Uuid => "uuid".into(),
            ColumnType::Date => "date".into(),
            ColumnType::DateTime => "datetime".into(),
            ColumnType::Url => "url".into(),
            ColumnType::Version => "version".into(),
            ColumnType::Tags => "tags".into(),
            ColumnType::Enum(values) => format!("enum:{}", values.join(",")),
        }
    }

    /// Parse a TOML wire token back to the typed enum. Errors on
    /// unknown type names or malformed `enum:` payloads.
    pub fn parse_token(s: &str) -> Result<Self, ColumnTypeParseError> {
        if let Some(rest) = s.strip_prefix("enum:") {
            let values: Vec<String> = rest
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect();
            if values.is_empty() || values.iter().any(String::is_empty) {
                return Err(ColumnTypeParseError::EmptyEnum);
            }
            return Ok(ColumnType::Enum(values));
        }
        Ok(match s {
            "string" => ColumnType::String,
            "number" => ColumnType::Number,
            "int" => ColumnType::Int,
            "bool" => ColumnType::Bool,
            "uuid" => ColumnType::Uuid,
            "date" => ColumnType::Date,
            "datetime" => ColumnType::DateTime,
            "url" => ColumnType::Url,
            "version" => ColumnType::Version,
            "tags" => ColumnType::Tags,
            other => return Err(ColumnTypeParseError::UnknownToken(other.to_string())),
        })
    }
}

/// Errors from [`ColumnType::parse_token`].
#[derive(Debug, thiserror::Error)]
pub enum ColumnTypeParseError {
    #[error("unknown column type token {0:?}")]
    UnknownToken(String),
    #[error("enum column type must list at least one non-empty value")]
    EmptyEnum,
}

impl Serialize for ColumnType {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_token())
    }
}

impl<'de> Deserialize<'de> for ColumnType {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        ColumnType::parse_token(&s).map_err(serde::de::Error::custom)
    }
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
    #[error(
        "table {table:?}: column_types declares {column:?} but the TSV header \
         does not include that column"
    )]
    ColumnTypeForUnknownColumn { table: String, column: String },
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
        LibraryTable {
            columns,
            rows,
            column_types: BTreeMap::new(),
        }
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
                column_types: BTreeMap::new(),
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
                column_types: BTreeMap::new(),
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
                column_types: BTreeMap::new(),
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
                column_types: BTreeMap::new(),
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
                column_types: BTreeMap::new(),
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
            column_types: BTreeMap::new(),
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
        tables.insert("misc".to_string(), LibraryTable {
            columns,
            rows,
            column_types: BTreeMap::new(),
        });
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// Canonical row order on write — rows must emit sorted by `row_id`
    /// regardless of in-memory `rows` Vec order, so on-disk file layout
    /// is stable across UI sort changes / bulk insertion order. This is
    /// the load-bearing fix for the "git blame breaks when rows are
    /// reordered" failure mode the architecture critique called out.
    #[test]
    fn write_emits_canonical_row_order_by_row_id() {
        let columns = vec!["row_id".to_string(), "mpn".to_string()];
        // Insert rows in REVERSE row_id order — write should still emit
        // them sorted ascending.
        let rows = vec![
            make_row(&["row_id", "mpn"], &["zzzz", "Late"]),
            make_row(&["row_id", "mpn"], &["mmmm", "Mid"]),
            make_row(&["row_id", "mpn"], &["aaaa", "Early"]),
        ];
        let mut tables = BTreeMap::new();
        tables.insert(
            "items".to_string(),
            LibraryTable {
                columns,
                rows,
                column_types: BTreeMap::new(),
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();

        // The TSV body should list rows alphabetically by row_id.
        let expected_order = ["aaaa\tEarly", "mmmm\tMid", "zzzz\tLate"];
        let aaaa_pos = s.find("aaaa\tEarly").expect("aaaa row");
        let mmmm_pos = s.find("mmmm\tMid").expect("mmmm row");
        let zzzz_pos = s.find("zzzz\tLate").expect("zzzz row");
        assert!(
            aaaa_pos < mmmm_pos && mmmm_pos < zzzz_pos,
            "expected canonical order {:?} but got: {s}",
            expected_order
        );

        // And the round-trip's rows are in canonical order too.
        let back = LibraryFile::parse(&s).unwrap();
        let row_ids: Vec<&str> = back.tables["items"]
            .rows
            .iter()
            .map(|r| r.cells["row_id"].as_str())
            .collect();
        assert_eq!(row_ids, vec!["aaaa", "mmmm", "zzzz"]);
    }

    /// Tables without a `row_id` column preserve insertion order — no
    /// canonical key to sort by, so we don't reshuffle the user's
    /// arbitrary lookup tables.
    #[test]
    fn write_preserves_insertion_order_when_no_row_id() {
        let columns = vec!["key".to_string(), "value".to_string()];
        let rows = vec![
            make_row(&["key", "value"], &["zebra", "Z"]),
            make_row(&["key", "value"], &["apple", "A"]),
        ];
        let mut tables = BTreeMap::new();
        tables.insert(
            "lookups".to_string(),
            LibraryTable {
                columns,
                rows,
                column_types: BTreeMap::new(),
            },
        );
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        let zebra_pos = s.find("zebra\tZ").unwrap();
        let apple_pos = s.find("apple\tA").unwrap();
        assert!(zebra_pos < apple_pos, "expected insertion order kept");
    }

    /// `column_types` map round-trips through the
    /// `[tables.<name>.column_types]` sidecar — the type tokens
    /// (`number`, `bool`, `enum:active,preferred,...`) survive parse +
    /// write unchanged.
    #[test]
    fn column_types_round_trip() {
        let columns = vec![
            "row_id".to_string(),
            "mpn".to_string(),
            "rated_power_mw".to_string(),
            "released".to_string(),
            "lifecycle".to_string(),
        ];
        let rows = vec![make_row(
            &["row_id", "mpn", "rated_power_mw", "released", "lifecycle"],
            &["abc-123", "RC0603", "100", "true", "preferred"],
        )];
        let mut column_types = BTreeMap::new();
        column_types.insert("row_id".into(), ColumnType::Uuid);
        column_types.insert("rated_power_mw".into(), ColumnType::Number);
        column_types.insert("released".into(), ColumnType::Bool);
        column_types.insert(
            "lifecycle".into(),
            ColumnType::Enum(vec![
                "active".into(),
                "preferred".into(),
                "deprecated".into(),
                "obsolete".into(),
            ]),
        );

        let mut tables = BTreeMap::new();
        tables.insert(
            "resistors".into(),
            LibraryTable {
                columns,
                rows,
                column_types,
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

    /// Untyped tables round-trip without emitting an empty
    /// `[tables.<name>.column_types]` block — keeps the `.snxlib`
    /// clean for the common "user just made a quick lookup table" case.
    #[test]
    fn untyped_tables_skip_column_types_block() {
        let mut tables = BTreeMap::new();
        tables.insert("resistors".to_string(), fixture_table_resistors());
        let lib = LibraryFile {
            manifest: fixture_manifest(),
            tables,
        };
        let s = lib.write().unwrap();
        assert!(
            !s.contains("column_types"),
            "untyped tables must not emit a column_types block; got:\n{s}"
        );
        let back = LibraryFile::parse(&s).unwrap();
        assert_eq!(lib, back);
    }

    /// `column_types` with a key that doesn't exist in the TSV header
    /// is loud — silently rendering dropdowns for ghost columns would
    /// be confusing.
    #[test]
    fn parse_rejects_column_type_for_unknown_column() {
        let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000000\"\n\
                 \n\
                 [library]\n\
                 name = \"X\"\n\
                 \n\
                 [tables.t]\n\
                 tsv = '''\n\
                 row_id\tmpn\n\
                 abc\tLM317\n\
                 '''\n\
                 \n\
                 [tables.t.column_types]\n\
                 row_id = \"uuid\"\n\
                 ghost_col = \"number\"\n";
        let err = LibraryFile::parse(s).unwrap_err();
        assert!(matches!(
            err,
            LibraryFileError::ColumnTypeForUnknownColumn { .. }
        ));
    }

    /// Unknown type tokens fail loudly rather than silently treating
    /// the column as `string`.
    #[test]
    fn parse_rejects_unknown_type_token() {
        let s = "format = \"snxlib/1\"\n\
                 library_id = \"0192a8c0-0000-7000-8000-000000000000\"\n\
                 \n\
                 [library]\n\
                 name = \"X\"\n\
                 \n\
                 [tables.t]\n\
                 tsv = '''\n\
                 row_id\tmpn\n\
                 abc\tLM317\n\
                 '''\n\
                 \n\
                 [tables.t.column_types]\n\
                 row_id = \"banana\"\n";
        let err = LibraryFile::parse(s).unwrap_err();
        // Wrapped through serde, so the inner error is TomlDe.
        assert!(matches!(err, LibraryFileError::TomlDe(_)));
    }

    /// `ColumnType::Enum` requires at least one non-empty value —
    /// `enum:` (empty) or `enum:,` (only blanks) are rejected.
    #[test]
    fn column_type_parse_rejects_empty_enum() {
        let err = ColumnType::parse_token("enum:").unwrap_err();
        assert!(matches!(err, ColumnTypeParseError::EmptyEnum));
        let err = ColumnType::parse_token("enum:,,").unwrap_err();
        assert!(matches!(err, ColumnTypeParseError::EmptyEnum));
    }

    /// Token-level round-trip for every variant — paranoia test that
    /// every enum variant survives `to_token` → `parse_token`.
    #[test]
    fn column_type_token_round_trip_all_variants() {
        let cases = [
            ColumnType::String,
            ColumnType::Number,
            ColumnType::Int,
            ColumnType::Bool,
            ColumnType::Uuid,
            ColumnType::Date,
            ColumnType::DateTime,
            ColumnType::Url,
            ColumnType::Version,
            ColumnType::Tags,
            ColumnType::Enum(vec!["a".into(), "b".into(), "c".into()]),
        ];
        for t in cases {
            let token = t.to_token();
            let back = ColumnType::parse_token(&token).unwrap();
            assert_eq!(t, back, "round-trip failed for {:?}", t);
        }
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
                column_types: BTreeMap::new(),
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
