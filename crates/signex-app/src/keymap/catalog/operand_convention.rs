//! Test-only views of the catalog operands (#1934): the naming convention a
//! test holds every catalog row to, and the golden column text.

use signex_commands::catalog::{Operand, OperandKind};

/// The allowed operand names and the one kind each has, on every command.
pub const CONVENTION: &[(&str, OperandKind)] = &[
    ("at", OperandKind::Point),
    ("start", OperandKind::Point),
    ("end", OperandKind::Point),
    ("rotation", OperandKind::Angle),
    ("degrees", OperandKind::Turn),
    ("text", OperandKind::Text),
    ("net_name", OperandKind::Text),
    ("class", OperandKind::Text),
    ("lib_id", OperandKind::Text),
    ("path", OperandKind::Path),
    ("overwrite", OperandKind::Bool),
    ("color", OperandKind::Color),
    ("document", OperandKind::Document),
    ("id", OperandKind::Text),
    ("kind", OperandKind::Text),
    ("project", OperandKind::Text),
    ("name", OperandKind::Text),
    ("dirty", OperandKind::Bool),
    ("allow_merge", OperandKind::Bool),
    ("allow_open", OperandKind::Bool),
    ("dry_run", OperandKind::Bool),
    ("designator", OperandKind::Text),
    ("value", OperandKind::Text),
    ("net", OperandKind::Text),
    ("ids", OperandKind::Text),
    ("kinds", OperandKind::Text),
    ("region", OperandKind::Text),
    ("fields", OperandKind::Text),
    ("offset", OperandKind::Text),
    ("limit", OperandKind::Text),
    ("expect_revision", OperandKind::Text),
    ("expect_revisions", OperandKind::Text),
    ("edits", OperandKind::Text),
    ("summary", OperandKind::Text),
    ("group", OperandKind::Text),
    ("documents", OperandKind::Text),
    ("location", OperandKind::Path),
    ("make_folder", OperandKind::Bool),
    ("profile", OperandKind::Text),
    ("overrides", OperandKind::Text),
    ("library", OperandKind::Text),
    ("part", OperandKind::Text),
    ("unit", OperandKind::Text),
    ("mirror", OperandKind::Text),
    ("sheet", OperandKind::Text),
    ("file", OperandKind::Text),
    ("direction", OperandKind::Text),
    ("apply", OperandKind::Bool),
    ("baseline", OperandKind::Text),
];

/// The golden column:`name:kind` with `/unit`, `?` when optional, comma
/// separated; `-` when the command takes none.
pub fn describe(operands: &[Operand]) -> String {
    if operands.is_empty() {
        return "-".into();
    }
    let parts: Vec<String> = operands
        .iter()
        .map(|op| {
            let unit = op.kind.unit().map_or(String::new(), |u| format!("/{u}"));
            let opt = if op.required { "" } else { "?" };
            format!("{}:{}{unit}{opt}", op.name, op.kind.name())
        })
        .collect();
    parts.join(",")
}
