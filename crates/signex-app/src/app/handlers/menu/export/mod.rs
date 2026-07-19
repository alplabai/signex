//! Export / print-preview menu handlers (PDF, netlist, BOM, print).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_output::{ExportContext, ProjectMetadata, SheetSnapshot};
use signex_types::schematic::SchematicSheet;

mod bom;
mod pdf_netlist;
mod print_preview;
#[cfg(test)]
mod tests;

/// Absolute path of a project's root schematic — its declared
/// `schematic_root`, falling back to the first entry in the sheet list.
fn project_root_sheet_path(project: &crate::app::state::LoadedProject) -> Option<PathBuf> {
    let filename = project
        .data
        .schematic_root
        .clone()
        .or_else(|| project.data.sheets.first().map(|s| s.filename.clone()))?;
    Some(project.dir().join(filename))
}

/// Assemble the export context, discarding the stitch issues — for the
/// callers that render an *intermediate* artifact rather than a deliverable
/// (the BOM dialog, every print-preview rerasterize).
///
/// Deliberately silent. Round 3 logged each issue here, and
/// `rerasterize_print_preview` calls this on every settings toggle *and* on
/// every keystroke in the specific-page input; the Messages panel keeps only
/// `MAX_DIAGNOSTIC_ENTRIES` (200) with no dedupe, so those copies evicted the
/// user's ERC results and eventually the earliest copies of the very stitch
/// warnings this path exists to deliver. Surfacing belongs to the user
/// actions — see [`log_stitch_issues`].
fn build_export_context(
    document_state: &crate::app::state::DocumentState,
) -> Option<ExportContext> {
    build_export_scope(document_state).map(|(ctx, _issues)| ctx)
}

/// Whether the derived netlist is missing connectivity rather than merely
/// oddly named.
///
/// A `MissingChild` subtree does not just leave its own nets out: nets that
/// should merge through that sheet's ports stay split, so the surviving nets
/// can carry the *wrong* names. `SheetCycle` truncates the walk with the same
/// effect. The other `StitchIssue` variants (duplicate UUIDs, shared
/// references, name collisions) describe a complete netlist with a naming or
/// annotation problem — loud, but not a hole.
fn netlist_is_incomplete(issues: &[signex_net::StitchIssue]) -> bool {
    issues.iter().any(|issue| {
        matches!(
            issue,
            signex_net::StitchIssue::MissingChild { .. }
                | signex_net::StitchIssue::SheetCycle { .. }
        )
    })
}

/// Put the stitch issues in front of the user — called once per *user action*
/// (print-preview open, PDF written, netlist written), never from the shared
/// context builder.
fn log_stitch_issues(
    document_state: &crate::app::state::DocumentState,
    ctx: &ExportContext,
    issues: &[signex_net::StitchIssue],
) {
    for issue in issues {
        crate::diagnostics::log_warning(format!(
            "Export: {}",
            crate::app::project_sheets::stitch_issue_message(issue)
        ));
    }
    if ctx.netlist.is_none() {
        // No root sheet in the exported set at all — the netlist exporter has
        // nothing to write (it returns `NetlistError::NoNetlist`) and every
        // `NET_NAME()` annotation in the PDF falls back to the literal token.
        // Raised here rather than in `signex-output`: that crate already
        // *errors* on a missing netlist where a netlist is the deliverable;
        // what it cannot do is warn about the silently-degraded PDF, and it is
        // a dependency-light domain crate that neither logs nor knows which
        // document the user was looking at.
        crate::diagnostics::log_warning(format!(
            "Export: no netlist derived for {} — NET_NAME() annotations will be unresolved",
            document_state
                .active_path
                .as_ref()
                .map_or_else(String::new, |p| p.display().to_string())
        ));
    } else if netlist_is_incomplete(issues) {
        crate::diagnostics::log_warning(
            "Export: the project netlist is incomplete — NET_NAME() annotations may be \
             unresolved and may show the WRONG net name, because nets that merge through \
             the missing sheet's ports stay split. Do not use this export as a wiring \
             reference.",
        );
    }
}

/// Read one sheet: the live engine snapshot when the file is open as a tab
/// (so unsaved edits are in the export), a disk parse otherwise.
fn load_sheet(
    document_state: &crate::app::state::DocumentState,
    path: &Path,
) -> anyhow::Result<SchematicSheet> {
    if let Some(engine) = document_state.engines.get(path) {
        return Ok(engine.document().clone());
    }
    let text = std::fs::read_to_string(path)?;
    Ok(signex_types::format::SnxSchematic::parse(&text)?.sheet)
}

/// Every sheet reachable from `root_path` down the `child_sheets` graph, keyed
/// by absolute path.
///
/// The netlist input set is **not** the page set. `data.sheets` lists the
/// pages a project *prints*; a hierarchical child is reached by a
/// `child_sheets` reference and is never added to that list — descending into
/// one opens a tab without registering it. Deriving the netlist from the pages
/// alone therefore drops every such subtree and reports a `MissingChild` for a
/// sheet that is sitting in memory, or right next to its parent on disk. That
/// is where the export's phantom stitch issues came from (#406).
///
/// Cycle-safe via the visited set. A reference that resolves to neither an
/// open tab nor a readable file is simply absent, which is precisely the case
/// `build_project_netlist` reports as a genuine `MissingChild`.
fn reachable_sheets(
    document_state: &crate::app::state::DocumentState,
    root_path: &Path,
) -> HashMap<PathBuf, SchematicSheet> {
    let mut out: HashMap<PathBuf, SchematicSheet> = HashMap::new();
    let mut queue = vec![root_path.to_path_buf()];
    while let Some(path) = queue.pop() {
        if out.contains_key(&path) {
            continue;
        }
        let Ok(sheet) = load_sheet(document_state, &path) else {
            continue;
        };
        let dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        for child in &sheet.child_sheets {
            queue.push(dir.join(&child.filename));
        }
        out.insert(path, sheet);
    }
    out
}

/// The export's page set + metadata + project netlist, alongside the stitch
/// issues raised while deriving that netlist. Returns `None` if there is no
/// active engine.
///
/// The issues are handed back rather than acted on here, because severity is
/// a **per-deliverable policy**, not a property of the scope: the `.net` file
/// is machine-consumed and refuses on a hole, the PDF is human-consumed and
/// degrades loudly. See `handle_export_netlist_finished` /
/// `handle_export_pdf_finished`.
fn build_export_scope(
    document_state: &crate::app::state::DocumentState,
) -> Option<(ExportContext, Vec<signex_net::StitchIssue>)> {
    let active_path = document_state.active_path.as_ref()?;
    let active_engine = document_state.engines.get(active_path)?;

    // Project-wide PDF: walk the owning project's full sheet list rather
    // than just the open tabs. Sheets currently opened as tabs use the
    // live engine snapshot (so unsaved edits show in the preview);
    // unopened sheets are read straight from disk via the parser. If
    // the active document isn't tied to a project (loose .snxsch), we
    // fall back to the engines map so a single-sheet preview still
    // works.
    //
    // Scope comes from `active_document_project()` — the project that owns
    // the active document — NOT `active_loaded_project()`, whose
    // `active_project` pointer is sticky by design and keeps pointing at the
    // last-loaded project while a loose file is focused (#406).
    let owning_project = document_state.active_document_project();
    let sheets: Vec<SheetSnapshot> = if let Some(project) = owning_project {
        let project_dir = project.dir();
        let mut snapshots: Vec<SheetSnapshot> = Vec::new();
        let total = project.data.sheets.len().max(1);
        for (i, entry) in project.data.sheets.iter().enumerate() {
            let abs_path: PathBuf = project_dir.join(&entry.filename);
            let schematic = match load_sheet(document_state, &abs_path) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!(
                        "Print preview: skipping sheet {} ({}): {e}",
                        entry.name,
                        abs_path.display()
                    );
                    continue;
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
        // Loose documents only: an open tab belonging to some *other*
        // loaded project would otherwise ride along as an extra page.
        let mut paths: Vec<PathBuf> = document_state.unowned_engine_paths();
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
    let active_variant = owning_project
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
    let mut issues: Vec<signex_net::StitchIssue> = Vec::new();
    let netlist = {
        // Root a project-wide export at the *project's* root sheet,
        // unconditionally. Rooting at `active_path` made one File > Export
        // Netlist yield either the project netlist or a subtree-only netlist
        // depending on whether the sheet the user happened to be looking at
        // had ever been added to `data.sheets` — same user intent, two
        // different .net files, no way to tell them apart. Active-path
        // rooting stays on the loose-document path, where the active document
        // *is* the whole scope.
        let root_path = match owning_project {
            Some(project) => project_root_sheet_path(project),
            None => Some(active_path.clone()),
        };
        root_path.and_then(|root_path| {
            // Input set = the printed pages *plus* everything reachable down
            // the child-sheet graph. See `reachable_sheets`: the pages alone
            // are an incomplete netlist input, and it is that incompleteness
            // — not the reporting of it — that manufactured the MissingChild
            // issues this export used to emit.
            let mut by_path: HashMap<PathBuf, SchematicSheet> = sheets
                .iter()
                .map(|s| (s.path.clone(), s.schematic.clone()))
                .collect();
            by_path.extend(reachable_sheets(document_state, &root_path));
            let root = by_path.get(&root_path)?.clone();
            let children = crate::app::project_sheets::project_children_map(&by_path);
            let project_dir = owning_project.map(|p| p.dir().to_path_buf());
            let root_filename =
                crate::app::project_sheets::root_reference_name(&root_path, project_dir.as_deref());
            // `build_project_netlist` always produces a netlist and reports
            // what it could not stitch in-band. The issues are returned to the
            // caller rather than dropped: they decide policy — the .net export
            // refuses on them, the PDF proceeds and warns.
            let result =
                signex_net::build_project_netlist(&root, &children, root_filename.as_deref());
            issues = result.issues;
            Some(result.netlist)
        })
    };

    Some((
        ExportContext {
            sheets,
            metadata,
            netlist,
        },
        issues,
    ))
}
