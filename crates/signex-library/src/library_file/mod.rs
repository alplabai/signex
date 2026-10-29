//! `LibraryFile` — on-disk representation of a `.snxlib` file.
//!
//! Per `v0.9-snxlib-as-file-plan.md` §1, a Signex component library is a
//! directory whose `.snxlib` file is the user-facing entry point. The file
//! is a TOML document combining a small manifest header with one
//! `[tables.<name>]` block per user-defined component category. Each table
//! holds raw TSV inside a TOML literal multi-line string so the data is
//! line-diffable in git and editable in any spreadsheet.
//!
//! This module owns the storage shape:
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
//! This module is the storage shape and the round-trip contract only.
//! Reserved-core-column validation and typed accessors for fields like
//! `row_id` / `version` / `released` live with the adapters, and the
//! cascade engine in [`crate::cascade`].

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapter::LibraryError;
use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};

pub use layout::SourceLayout;

/// Format token written at the top of every `.snxlib`. Bumping this is a
/// wire-format break — older Signex versions refuse to open the file.
pub const FORMAT_TOKEN: &str = "snxlib/2";

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
    /// The text this file was parsed from, when it holds keys the structs
    /// above do not model; [`LibraryFile::write`] edits that text in place
    /// so those keys survive (#1435). Not part of `==`.
    pub layout: SourceLayout,
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
    /// User-editable class registry — shown in the New Component
    /// modal's Class dropdown for components in this library.
    /// Empty by default; the New Library flow seeds the list from
    /// the user's `prefs.json::component_classes` so freshly-created
    /// libraries start with the user's preferred taxonomy. Edits
    /// land via the (forthcoming) Library Properties pane and
    /// persist as `[[classes]]` entries inside the `.snxlib`.
    #[serde(default)]
    pub classes: Vec<ClassEntry>,
}

/// One row of the per-library class registry. `key` is the canonical
/// machine identifier stored on `ComponentRow.class`; `label` is the
/// human-readable name surfaced in pickers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassEntry {
    pub key: String,
    pub label: String,
}

/// `[library]` block — human-readable name + description. The `library_id`
/// lives at the TOML root, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibrarySection {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Fixes a TSV row whose cell count mismatches its header; see
/// [`LibraryFile::parse_with`].
pub type RowRepair<'a> = &'a dyn Fn(&[String], &[&str]) -> Option<Vec<String>>;

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
///
/// `pricing` is the cached distributor price of each row, keyed by the
/// row's `row_id` text. It is a sidecar like `column_types`: written as
/// `[tables.<name>.pricing.<row_id>.<distributor>.<sku>]` sub-tables only when
/// non-empty, so a library without cached prices is byte-identical to one
/// written before the sidecar existed. See `docs/architecture/snxlib-pricing.md`.
///
/// Rows are shared, copy-on-write: cloning a table copies one pointer per
/// row, and an edit replaces only the rows it touches (`Arc::make_mut` or a
/// new `Arc`). An edit works on a clone of the whole library (#831), so on
/// a large library this is what keeps that clone and the drop of the old
/// copy cheap (#416).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryTable {
    pub columns: Vec<String>,
    pub rows: Vec<Arc<LibraryRow>>,
    pub column_types: BTreeMap<String, ColumnType>,
    pub pricing: BTreeMap<String, crate::pricing::RowPricing>,
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
            let values: Vec<String> = rest.split(',').map(str::trim).map(str::to_string).collect();
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
        "table {table:?}: row {row_index} cell {column:?} contains a tab, newline or other control character; \
         TSV cells must be free of them"
    )]
    DisallowedControlInCell {
        table: String,
        row_index: usize,
        column: String,
    },
    #[error(
        "table {table:?}: column {column:?} name contains a tab, newline or other control character; \
         TSV column names must be free of them"
    )]
    DisallowedControlInColumn { table: String, column: String },
    #[error(
        "table {table:?}: cell {column:?} contains the literal triple single-quote \
         (\"'''\") which would terminate the embedded TOML literal multi-line string"
    )]
    DisallowedTripleQuoteInCell { table: String, column: String },
    #[error(
        "table {table:?}: column name {column:?} contains the literal triple single-quote \
         (\"'''\") which would terminate the embedded TOML literal multi-line string"
    )]
    DisallowedTripleQuoteInColumn { table: String, column: String },
    #[error(
        "table {table:?}: cached price of row {row_id:?} from {distributor:?} is not storable: {reason}"
    )]
    InvalidPricing {
        table: String,
        row_id: String,
        distributor: String,
        reason: String,
    },
    #[error("class key {key:?} is declared more than once; class keys must be unique")]
    DuplicateClassKey { key: String },
    #[error("the written .snxlib document does not parse back: {reason}")]
    WrittenDocumentUnreadable { reason: String },
    #[error(
        "the written .snxlib document does not read back as the same table {table:?}; \
         refusing to overwrite the file with lossy output"
    )]
    WrittenDocumentDiffers { table: String },
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
        match value {
            // Encoding our own value failed: a program bug, not the user's data.
            LibraryFileError::TomlSer(_) => LibraryError::Backend(value.to_string()),
            _ => LibraryError::InvalidData(value.to_string()),
        }
    }
}

mod codec;
mod layout;
#[cfg(test)]
mod layout_tests;
mod row_lines;
pub(crate) use row_lines::RowLines;
#[cfg(test)]
mod pricing_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_equivalence_tests;
