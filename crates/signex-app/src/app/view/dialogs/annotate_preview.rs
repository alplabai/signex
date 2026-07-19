//! Annotate preview — the project-wide proposed-designator list the Annotate
//! modal shows before the user commits.

use std::path::{Path, PathBuf};

use signex_types::schematic::{SchematicSheet, Symbol};

/// One row of the project-wide proposed change list.
#[derive(Debug, Clone)]
pub(super) struct AnnotatePreviewEntry {
    pub sheet: String,
    pub current: String,
    pub proposed: String,
    /// Symbol uuid — lets the row's lock checkbox toggle the global
    /// `ui_state.annotate_locked` set without re-looking-up the symbol.
    pub uuid: uuid::Uuid,
}

/// Power ports (`#PWR`, `#FLG`, `is_power`) aren't designators — they're net
/// anchors whose "reference" is the net name.
fn is_target(sym: &Symbol) -> bool {
    !sym.is_power && !sym.reference.starts_with('#')
}

/// The alphabetic head of a reference — `"R"` for both `"R12"` and `"R?"`.
fn prefix_of(reference: &str) -> String {
    reference
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect()
}

/// Highest number already claimed per prefix across `sheets` — the same seed
/// `handle_annotate`'s pass A builds, from the same sheet set.
fn seed_counters(sheets: &[(String, SchematicSheet)]) -> std::collections::HashMap<String, u32> {
    let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (_, sheet) in sheets {
        for sym in sheet.symbols.iter().filter(|s| is_target(s)) {
            let prefix = prefix_of(&sym.reference);
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
                let e = next.entry(prefix).or_insert(0);
                *e = (*e).max(n);
            }
        }
    }
    next
}

/// Symbol indices in the engine's annotate order (y, then x, then uuid).
fn annotate_order(sheet: &SchematicSheet) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..sheet.symbols.len()).collect();
    idx.sort_by(|a, b| {
        let (sa, sb) = (&sheet.symbols[*a], &sheet.symbols[*b]);
        sa.position
            .y
            .partial_cmp(&sb.position.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                sa.position
                    .x
                    .partial_cmp(&sb.position.x)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(sa.uuid.cmp(&sb.uuid))
    });
    idx
}

impl super::super::super::Signex {
    /// The display title a sheet is shown under: its tab title when it is open,
    /// its file stem otherwise.
    fn sheet_display_title(&self, path: &Path) -> String {
        if let Some(tab) = self
            .document_state
            .tabs
            .iter()
            .find(|t| t.path.as_path() == path)
        {
            return tab.title.clone();
        }
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Sheet")
            .to_string()
    }

    /// The sheets the change list spans, in path order.
    ///
    /// The set comes from the one assembler — the same call `handle_annotate`
    /// makes — rather than a private rule of its own. A preview that disagrees
    /// with the action it previews is worse than no preview, and this one
    /// disagreed in both directions: it listed loose and other-project tabs the
    /// action refuses to touch, and it hid the unlisted hierarchical children
    /// the action renumbers *and writes back to disk* (#406).
    ///
    /// This reads sheets that are not open from disk, and it runs from `view`.
    /// See the module note in `view/dialogs/annotate/mod.rs` on why that is
    /// tolerated rather than cached.
    fn preview_sheets(&self) -> Vec<(String, SchematicSheet)> {
        let (_pages, set) =
            crate::app::project_sheets::assemble_active_project_sheets(&self.document_state);
        let mut sheets: Vec<(PathBuf, SchematicSheet)> = set.sheets.into_iter().collect();
        sheets.sort_by(|a, b| a.0.cmp(&b.0));
        // `handle_annotate`'s last pass annotates the active engine
        // unconditionally, so it belongs in the change list even where the
        // assembler cannot place it — an unsaved document has no path to
        // assemble from.
        let active_covered = self
            .document_state
            .active_path
            .as_ref()
            .is_some_and(|p| sheets.iter().any(|(path, _)| path == p));
        let mut out: Vec<(String, SchematicSheet)> = sheets
            .into_iter()
            .map(|(path, sheet)| (self.sheet_display_title(&path), sheet))
            .collect();
        if !active_covered && let Some(engine) = self.document_state.active_engine() {
            let title = self
                .document_state
                .active_path
                .as_deref()
                .map(|p| self.sheet_display_title(p))
                .unwrap_or_else(|| "(untitled)".to_string());
            out.push((title, engine.document().clone()));
        }
        out
    }

    /// The proposed `(current, new)` reference designators Annotate would hand
    /// out across the project. One row per symbol so the user sees the whole
    /// project; rows where `current == proposed` are "no change".
    pub(super) fn preview_project_annotations(&self) -> Vec<AnnotatePreviewEntry> {
        let sheets = self.preview_sheets();
        let mut next = seed_counters(&sheets);
        let mut out = Vec::new();
        for (title, sheet) in &sheets {
            for i in annotate_order(sheet) {
                let sym = &sheet.symbols[i];
                if sym.reference.is_empty() || !is_target(sym) {
                    continue;
                }
                let prefix = prefix_of(&sym.reference);
                let proposed = if sym.reference.ends_with('?') && !prefix.is_empty() {
                    let n = next.entry(prefix.clone()).or_insert(0);
                    *n += 1;
                    format!("{prefix}{n}")
                } else {
                    sym.reference.clone()
                };
                out.push(AnnotatePreviewEntry {
                    sheet: title.clone(),
                    current: sym.reference.clone(),
                    proposed,
                    uuid: sym.uuid,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
