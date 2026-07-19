//! Annotate preview — project-wide proposed-designator computation that
//! feeds the Annotate modal's change list.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change.

use crate::app::state::AnnotateOrder;

/// Compute the proposed (current, new) reference designator pairs that would
/// result from running Annotate Incremental on the active snapshot.
/// Preserves the engine's ordering logic (by y,x,uuid). The dialog
/// currently only offers one order because the engine hard-codes that;
/// order-flag wiring is a v0.7.1 follow-up.
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

impl super::super::super::Signex {
    /// Walk every schematic in the project — open tabs (live engine or
    /// cached session) plus every sheet listed in project_data.sheets that
    /// hasn't been opened yet. Unopened sheets are parsed on-the-fly so
    /// the change list reflects the whole project, not just what the user
    /// has active.
    pub(super) fn preview_project_annotations(&self) -> Vec<AnnotatePreviewEntry> {
        let is_target = |sym: &signex_types::schematic::Symbol| -> bool {
            !sym.is_power && !sym.reference.starts_with('#')
        };

        // Owned sheets (parsed from disk) are boxed so we can hold them in
        // the same vector as the borrowed ones and still use slice APIs.
        let mut owned_sheets: Vec<(String, signex_types::schematic::SchematicSheet)> = Vec::new();
        let mut open_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        // Pass 1: collect open tabs.
        let mut borrowed: Vec<(String, &signex_types::schematic::SchematicSheet)> = Vec::new();
        for tab in self.document_state.tabs.iter() {
            open_paths.insert(tab.path.clone());
            if let Some(engine) = self.document_state.engines.get(&tab.path) {
                borrowed.push((tab.title.clone(), engine.document()));
            }
        }
        // Fallback when no tabs are open but an engine still holds a doc.
        if borrowed.is_empty()
            && open_paths.is_empty()
            && let Some(eng) = self.document_state.active_engine()
        {
            borrowed.push(("(untitled)".to_string(), eng.document()));
        }

        // Pass 2: parse every remaining project sheet from disk so the
        // change list spans sheets the user hasn't opened yet.
        if let Some(loaded) = self.document_state.active_document_project() {
            let project_dir = loaded
                .path
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            for sheet_entry in &loaded.data.sheets {
                let file_path = project_dir.join(&sheet_entry.filename);
                if open_paths.contains(&file_path) {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&file_path)
                    && let Ok(parsed) =
                        signex_types::format::SnxSchematic::parse(&text).map(|snx| snx.sheet)
                {
                    let title = sheet_entry.name.trim_end_matches(".snxsch").to_string();
                    owned_sheets.push((title, parsed));
                }
            }
        }

        // Merge into a single Vec of borrowed references.
        let mut sheets: Vec<(String, &signex_types::schematic::SchematicSheet)> = borrowed;
        for (title, sheet) in &owned_sheets {
            sheets.push((title.clone(), sheet));
        }

        // Pass 1: global max per prefix.
        let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        for (_, sheet) in &sheets {
            for sym in &sheet.symbols {
                if !is_target(sym) {
                    continue;
                }
                let prefix: String = sym
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
                if prefix.is_empty() {
                    continue;
                }
                if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
                    let e = next.entry(prefix).or_insert(0);
                    if n > *e {
                        *e = n;
                    }
                }
            }
        }

        // Pass 2: iterate sheets, assigning proposed designators to '?' tails.
        let mut out = Vec::new();
        for (title, sheet) in &sheets {
            let mut idx: Vec<usize> = (0..sheet.symbols.len()).collect();
            idx.sort_by(|a, b| {
                let sa = &sheet.symbols[*a];
                let sb = &sheet.symbols[*b];
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
            for i in idx {
                let sym = &sheet.symbols[i];
                if sym.reference.is_empty() || !is_target(sym) {
                    continue;
                }
                let prefix: String = sym
                    .reference
                    .chars()
                    .take_while(|c| c.is_ascii_alphabetic())
                    .collect();
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

#[allow(dead_code)]
fn preview_annotations(
    snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
    _order: AnnotateOrder,
) -> Vec<(String, String)> {
    // Power ports (#PWR, #FLG, `is_power`) aren't designators — they're
    // net anchors whose "reference" is the net name. Skip them from the
    // annotate preview and the change list entirely.
    let is_designator_target = |sym: &signex_types::schematic::Symbol| -> bool {
        !sym.is_power && !sym.reference.starts_with('#')
    };
    // Collect existing per-prefix counters.
    let mut next: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for sym in &snapshot.symbols {
        if !is_designator_target(sym) {
            continue;
        }
        let prefix: String = sym
            .reference
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if prefix.is_empty() {
            continue;
        }
        if let Ok(n) = sym.reference[prefix.len()..].parse::<u32>() {
            let e = next.entry(prefix).or_insert(0);
            if n > *e {
                *e = n;
            }
        }
    }
    // Order: y ascending, then x ascending (matches the engine).
    let mut idx: Vec<usize> = (0..snapshot.symbols.len()).collect();
    idx.sort_by(|a, b| {
        let sa = &snapshot.symbols[*a];
        let sb = &snapshot.symbols[*b];
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
    // Emit a row for every symbol so the user sees the full project — rows
    // where current == proposed indicate "no change". Only symbols whose
    // reference ends in '?' will actually be renumbered.
    let mut out = Vec::new();
    for i in idx {
        let sym = &snapshot.symbols[i];
        if sym.reference.is_empty() || !is_designator_target(sym) {
            continue;
        }
        let prefix: String = sym
            .reference
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if sym.reference.ends_with('?') && !prefix.is_empty() {
            let n = next.entry(prefix.clone()).or_insert(0);
            *n += 1;
            out.push((sym.reference.clone(), format!("{prefix}{n}")));
        } else {
            // Already annotated — propose keeping the same designator.
            out.push((sym.reference.clone(), sym.reference.clone()));
        }
    }
    out
}
