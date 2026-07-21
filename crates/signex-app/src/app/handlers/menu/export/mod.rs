//! Export / print-preview menu handlers (PDF, netlist, BOM, print).

use std::path::PathBuf;

use signex_output::{ExportContext, ProjectMetadata, SheetSnapshot};

mod bom;
mod pdf_netlist;
mod print_preview;
#[cfg(test)]
pub(crate) mod tests;

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

/// Everything that went wrong while deriving the export's netlist.
#[derive(Default)]
pub(crate) struct ExportIssues {
    /// What the stitcher reported in-band.
    pub(crate) stitch: Vec<signex_net::StitchIssue>,
    /// Declared pages that exist and were read, but that the stitched netlist
    /// does not cover — the stitcher only walks down from the root, so a page
    /// nothing references is simply not in the netlist. Empty for a loose
    /// document, which has no declared pages.
    pub(crate) uncovered_pages: Vec<PathBuf>,
    /// Declared pages with no file at their path at all. Kept apart from
    /// [`Self::uncovered_pages`]: those are a child-sheet-graph problem and
    /// these are not, and a page that does not exist is also dropped from the
    /// exported page set, so the PDF comes out short a page.
    pub(crate) missing_pages: Vec<PathBuf>,
    /// Sheets that exist but could not be read: `(path, why)`.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
}

impl ExportIssues {
    /// Whether the derived netlist is missing connectivity rather than merely
    /// oddly named.
    ///
    /// A `MissingChild` subtree does not just leave its own nets out: nets that
    /// should merge through that sheet's ports stay split, so the surviving
    /// nets can carry the *wrong* names. `SheetCycle` truncates the walk with
    /// the same effect, and a page outside the hierarchy is a whole page of
    /// components and nets that never made it in. `AmbiguousChildFilename` is a
    /// hole for the same reason: the losing parent's subtree was stitched from
    /// the *wrong* file, so that subtree's real nets are absent and the other
    /// file's are grafted in twice. The remaining `StitchIssue` variants
    /// (duplicate UUIDs, shared references, name collisions) describe a complete
    /// netlist with a naming or annotation problem — loud, but not a hole.
    pub(crate) fn netlist_is_incomplete(&self) -> bool {
        !self.uncovered_pages.is_empty()
            || !self.missing_pages.is_empty()
            || !self.unreadable.is_empty()
            || self.stitch.iter().any(|issue| {
                matches!(
                    issue,
                    signex_net::StitchIssue::MissingChild { .. }
                        | signex_net::StitchIssue::SheetCycle { .. }
                        | signex_net::StitchIssue::AmbiguousChildFilename { .. }
                )
            })
    }

    /// One user-facing line per problem, in a stable order.
    pub(crate) fn messages(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .stitch
            .iter()
            .map(crate::app::project_sheets::stitch_issue_message)
            .collect();
        // Reported separately from MissingChild on purpose: "could not be
        // found" sends the user hunting for a file that is sitting right
        // there, when the real problem is its contents. Listed first so it
        // explains any page below that is uncovered *because* of it.
        for (path, why) in &self.unreadable {
            out.push(format!("Netlist: sheet '{}' {why}", path.display()));
        }
        for path in &self.missing_pages {
            out.push(format!(
                "Netlist: page '{}' is listed in the project but no file exists at that \
                 path — it is dropped from the netlist and from the exported pages",
                path.display()
            ));
        }
        for path in &self.uncovered_pages {
            out.push(format!(
                "Netlist: page '{}' is listed in the project but is not in the netlist — \
                 nothing reaches it from the root sheet by a child-sheet reference",
                path.display()
            ));
        }
        out
    }
}

/// Put the stitch issues in front of the user — called once per *user action*
/// (print-preview open, PDF written, netlist written), never from the shared
/// context builder.
fn log_stitch_issues(
    document_state: &crate::app::state::DocumentState,
    ctx: &ExportContext,
    issues: &ExportIssues,
) {
    for message in issues.messages() {
        crate::diagnostics::log_warning(format!("Export: {message}"));
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
    } else if issues.netlist_is_incomplete() {
        crate::diagnostics::log_warning(
            "Export: the project netlist is incomplete — NET_NAME() annotations may be \
             unresolved and may show the WRONG net name, because nets that merge through \
             the missing sheet's ports stay split. Do not use this export as a wiring \
             reference.",
        );
    }
}

/// One page per project sheet entry, in list order. Sheets open as tabs carry
/// the live engine snapshot (so unsaved edits show in the preview); the rest
/// were read from disk by the assembler.
fn owned_pages(
    project: &crate::app::state::LoadedProject,
    pages: &[PathBuf],
    sheet_set: &crate::app::project_sheets::ProjectSheetSet,
) -> Vec<SheetSnapshot> {
    let loaded = &sheet_set.sheets;
    let total = project.data.sheets.len().max(1);
    project
        .data
        .sheets
        .iter()
        .zip(pages)
        .enumerate()
        .filter_map(|(i, (entry, abs_path))| {
            let Some(schematic) = loaded.get(abs_path) else {
                log::warn!(
                    "Print preview: skipping sheet {} ({}): not loadable",
                    entry.name,
                    abs_path.display()
                );
                return None;
            };
            Some(SheetSnapshot {
                path: abs_path.clone(),
                schematic: schematic.clone(),
                sheet_name: entry.name.clone(),
                sheet_number: i + 1,
                sheet_count: total,
            })
        })
        .collect()
}

/// Pages for a loose .snxsch: the unowned open tabs, active one first. An open
/// tab belonging to some *other* loaded project must not ride along.
fn loose_pages(
    document_state: &crate::app::state::DocumentState,
    active_path: &PathBuf,
) -> Vec<SheetSnapshot> {
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
}

/// Title-block metadata for the export, plus the project's active variant.
fn export_metadata(
    active_engine: &signex_engine::Engine,
    owning_project: Option<&crate::app::state::LoadedProject>,
) -> ProjectMetadata {
    let tb = &active_engine.document().title_block;
    let comment = |n: usize| tb.get(&format!("comment{n}")).cloned().unwrap_or_default();
    let mut custom_fields = std::collections::BTreeMap::new();
    let active_variant = owning_project
        .and_then(|p| p.data.active_variant.clone())
        .unwrap_or_else(|| "Base".to_string());
    if !active_variant.eq_ignore_ascii_case("Base") {
        custom_fields.insert("active_variant".to_string(), active_variant);
    }
    ProjectMetadata {
        title: tb.get("title").cloned().unwrap_or_default(),
        revision: tb.get("rev").cloned().unwrap_or_default(),
        date: tb.get("date").cloned().unwrap_or_default(),
        company: tb.get("company").cloned().unwrap_or_default(),
        comments: [comment(1), comment(2), comment(3), comment(4)],
        custom_fields,
    }
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
) -> Option<(ExportContext, ExportIssues)> {
    let active_path = document_state.active_path.as_ref()?;
    let active_engine = document_state.engines.get(active_path)?;

    // Scope comes from `active_document_project()` — the project that owns
    // the active document — NOT `active_loaded_project()`, whose
    // `active_project` pointer is sticky by design and keeps pointing at the
    // last-loaded project while a loose file is focused (#406).
    let owning_project = document_state.active_document_project();

    // One assembler answers what the project consists of — for the export, the
    // cached canvas netlist, the ERC run, annotate and the duplicate-designator
    // reset alike — including what it is rooted at. Pages alone drop
    // hierarchical subtrees; root reachability alone drops flat pages.
    //
    // A project-wide export roots at the *project's* root sheet. Rooting at
    // `active_path` made one File > Export Netlist yield either the project
    // netlist or a subtree-only netlist depending on which sheet the user
    // happened to be looking at — same intent, two different .net files, no
    // way to tell them apart. Active-path rooting stays on the loose-document
    // path, where the active document *is* the whole scope.
    let (pages, sheet_set) =
        crate::app::project_sheets::assemble_active_project_sheets(document_state);

    let sheets = match owning_project {
        Some(project) => owned_pages(project, &pages, &sheet_set),
        None => loose_pages(document_state, active_path),
    };
    let metadata = export_metadata(active_engine, owning_project);

    // Derive the authoritative project netlist off that same sheet set, so the
    // netlist exporter reads the contract instead of re-deriving connectivity
    // (ADR-0002 D7). The children map is keyed by the exact
    // `ChildSheet.filename` each parent references (ADR-0002 D8).
    let mut issues = ExportIssues::default();
    let set = sheet_set;
    let netlist = set.root.clone().and_then(|root_path| {
        // The stitcher walks *down* from the root, so a declared page nothing
        // references is not in the netlist however complete the sheet set is.
        // Carry that shortfall as an incompleteness of its own: without it a
        // flat project exports a .net holding one page of a multi-page board,
        // with `issues` empty and nothing warned.
        //
        // Split by *why* a page is out. "Nothing reaches it from the root by a
        // child-sheet reference" sends the user to inspect a graph, which is
        // the wrong place when the file simply is not there; and a page that
        // exists but will not parse is already reported verbatim above, so it
        // must not also be reported as a graph problem. All three keep the
        // netlist incomplete.
        let unreadable: std::collections::HashSet<&PathBuf> =
            set.unreadable.iter().map(|(path, _)| path).collect();
        for path in &set.pages_outside_the_hierarchy {
            if unreadable.contains(path) {
                continue;
            }
            if set.sheets.contains_key(path) {
                issues.uncovered_pages.push(path.clone());
            } else {
                issues.missing_pages.push(path.clone());
            }
        }
        issues.unreadable = set.unreadable.clone();
        let root = set.sheets.get(&root_path)?;
        let (children, children_issues) =
            crate::app::project_sheets::project_children_map(&set.sheets);
        let project_dir = owning_project.map(|p| p.dir().to_path_buf());
        let root_filename =
            crate::app::project_sheets::root_reference_name(&root_path, project_dir.as_deref());
        // `build_project_netlist` always produces a netlist and reports what
        // it could not stitch in-band. The issues are returned to the caller
        // rather than acted on here, because severity is a per-deliverable
        // policy: the .net refuses on a hole, the PDF proceeds and warns.
        let result = signex_net::build_project_netlist(root, &children, root_filename.as_deref());
        let mut stitch = result.issues;
        stitch.extend(children_issues);
        issues.stitch = stitch;
        Some(result.netlist)
    });

    Some((
        ExportContext {
            sheets,
            metadata,
            netlist,
        },
        issues,
    ))
}
