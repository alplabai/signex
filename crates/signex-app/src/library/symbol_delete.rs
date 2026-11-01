//! Who still uses a library symbol that is about to be deleted (#1845).
//!
//! The SCH Library panel's Delete asks first when a component row binds the
//! symbol or an open sheet has parts placed from it. The scan reads only
//! state already in memory (the cached rows of the open libraries and the
//! open sheets), so it is safe on the `update` path.

use std::path::{Path, PathBuf};

use signex_library::PrimitiveRef;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::state::OpenLibrary;
use crate::panels::SymbolDeleteWarning;

/// Names listed per group before the rest is counted.
const MAX_LISTED: usize = 6;

/// A Delete waiting for the user's answer.
#[derive(Debug, Clone)]
pub struct PendingSymbolDelete {
    pub path: PathBuf,
    pub symbol: Uuid,
    /// `(undo, redo)` depths of the editor when the warning was raised; an
    /// edit, undo or redo since then makes the warning stale.
    pub history: (usize, usize),
    pub warning: SymbolDeleteWarning,
}

impl PendingSymbolDelete {
    /// The warning still describes `editor`: same container, same history,
    /// and the symbol it names is still at its slot.
    pub fn is_current(&self, editor: &crate::app::SymbolDocument) -> bool {
        editor.path == self.path
            && (editor.undo_snapshots.len(), editor.redo_snapshots.len()) == self.history
            && editor
                .file
                .symbols
                .get(self.warning.idx)
                .is_some_and(|s| s.uuid == self.symbol)
    }
}

/// The lines naming every row and open sheet that use `symbol`; empty when
/// nothing does.
pub fn usage_lines<'a>(
    libs: &[OpenLibrary],
    sheets: impl Iterator<Item = (&'a Path, &'a SchematicSheet)>,
    symbol: &PrimitiveRef,
) -> Vec<String> {
    let mut rows = Vec::new();
    for lib in libs {
        let mut tables: Vec<_> = lib.tables.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        for (table, list) in tables {
            for row in list.iter().filter(|r| r.binds_symbol(symbol)) {
                rows.push(format!(
                    "{} ({} / {})",
                    row.internal_pn.as_str(),
                    lib.display_name,
                    table
                ));
            }
        }
    }
    let lib_id = super::place::lib_id_of(symbol);
    let mut placed = Vec::new();
    for (path, sheet) in sheets {
        let refs: Vec<&str> = sheet
            .symbols
            .iter()
            .filter(|s| s.lib_id == lib_id)
            .map(|s| s.reference.as_str())
            .collect();
        if !refs.is_empty() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            placed.push(format!("{name}: {}", refs.join(", ")));
        }
    }
    placed.sort();
    let mut lines = Vec::new();
    if !rows.is_empty() {
        lines.push(format!("Component rows: {}", capped(&rows)));
    }
    if !placed.is_empty() {
        lines.push(format!(
            "Open sheets with placed parts: {}",
            capped(&placed)
        ));
    }
    lines
}

fn capped(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    if items.len() > MAX_LISTED {
        text.push_str(&format!("; and {} more", items.len() - MAX_LISTED));
    }
    text
}
