//! Export / print-preview menu handlers (PDF, netlist, BOM, print).

use std::path::PathBuf;

use signex_output::{ExportContext, ProjectMetadata, SheetSnapshot};


mod bom;
mod pdf_netlist;
mod print_preview;

/// Snapshot every open engine as a `SheetSnapshot`, active engine first.
/// Returns `None` if there is no active engine.
fn build_export_context(
    document_state: &crate::app::state::DocumentState,
) -> Option<ExportContext> {
    let active_path = document_state.active_path.as_ref()?;
    let active_engine = document_state.engines.get(active_path)?;

    // Project-wide PDF: walk the active project's full sheet list rather
    // than just the open tabs. Sheets currently opened as tabs use the
    // live engine snapshot (so unsaved edits show in the preview);
    // unopened sheets are read straight from disk via the parser. If
    // the active document isn't tied to a project (loose .standard_sch),
    // we fall back to the engines map so a single-sheet preview still
    // works.
    let sheets: Vec<SheetSnapshot> = if let Some(project) = document_state.active_loaded_project() {
        let project_dir = std::path::Path::new(&project.data.dir);
        let mut snapshots: Vec<SheetSnapshot> = Vec::new();
        let total = project.data.sheets.len().max(1);
        for (i, entry) in project.data.sheets.iter().enumerate() {
            let abs_path: PathBuf = project_dir.join(&entry.filename);
            let schematic = match document_state.engines.get(&abs_path) {
                Some(engine) => engine.document().clone(),
                None => {
                    let parse_result = std::fs::read_to_string(&abs_path)
                        .map_err(anyhow::Error::from)
                        .and_then(|text| {
                            signex_types::format::SnxSchematic::parse(&text)
                                .map(|snx| snx.sheet)
                                .map_err(anyhow::Error::from)
                        });
                    match parse_result {
                        Ok(s) => s,
                        Err(e) => {
                            log::warn!(
                                "Print preview: skipping sheet {} ({}): {e}",
                                entry.name,
                                abs_path.display()
                            );
                            continue;
                        }
                    }
                }
            };
            snapshots.push(SheetSnapshot {
                path: abs_path,
                schematic,
                sheet_name: entry.name.clone(),
                sheet_number: i + 1,
                sheet_count: total,
            });
        }
        snapshots
    } else {
        let mut paths: Vec<PathBuf> = document_state.engines.keys().cloned().collect();
        paths.sort_by_key(|p| p != active_path);
        let sheet_count = paths.len();
        paths
            .into_iter()
            .enumerate()
            .filter_map(|(i, path)| {
                let engine = document_state.engines.get(&path)?;
                let sheet_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Sheet")
                    .to_string();
                Some(SheetSnapshot {
                    path: path.clone(),
                    schematic: engine.document().clone(),
                    sheet_name,
                    sheet_number: i + 1,
                    sheet_count,
                })
            })
            .collect()
    };

    let tb = &active_engine.document().title_block;
    let comment = |n: usize| tb.get(&format!("comment{n}")).cloned().unwrap_or_default();
    let mut custom_fields = std::collections::BTreeMap::new();
    let active_variant = document_state
        .active_loaded_project()
        .and_then(|p| p.data.active_variant.clone())
        .unwrap_or_else(|| "Base".to_string());
    if !active_variant.eq_ignore_ascii_case("Base") {
        custom_fields.insert("active_variant".to_string(), active_variant);
    }
    let metadata = ProjectMetadata {
        title: tb.get("title").cloned().unwrap_or_default(),
        revision: tb.get("rev").cloned().unwrap_or_default(),
        date: tb.get("date").cloned().unwrap_or_default(),
        company: tb.get("company").cloned().unwrap_or_default(),
        comments: [comment(1), comment(2), comment(3), comment(4)],
        custom_fields,
    };

    // Derive the authoritative project netlist off the same sheet set, so the
    // netlist exporter reads the contract instead of re-deriving connectivity
    // (ADR-0002 D7). The children map is keyed by the exact `ChildSheet.filename`
    // each parent references — the shared project view (ADR-0002 D8).
    let netlist = {
        let by_path: std::collections::HashMap<PathBuf, signex_types::schematic::SchematicSheet> =
            sheets
                .iter()
                .map(|s| (s.path.clone(), s.schematic.clone()))
                .collect();
        by_path.get(active_path).map(|root| {
            let children = crate::app::project_sheets::project_children_map(&by_path);
            let project_dir = document_state
                .active_loaded_project()
                .map(|p| PathBuf::from(&p.data.dir));
            let root_filename = crate::app::project_sheets::root_reference_name(
                active_path,
                project_dir.as_deref(),
            );
            signex_net::build_project_netlist(root, &children, root_filename.as_deref()).netlist
        })
    };

    Some(ExportContext {
        sheets,
        metadata,
        netlist,
    })
}
