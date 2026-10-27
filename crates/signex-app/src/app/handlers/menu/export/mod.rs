//! Export / print-preview menu handlers (PDF, netlist, BOM, print).

use std::path::PathBuf;

use signex_output::{ExportContext, ProjectMetadata, SheetSnapshot};
use signex_types::schematic::InstancePath;

use crate::app::Signex;

mod bom;
mod class_styles;
#[cfg(test)]
mod current_page_tests;
mod erc_report;
mod loose_bom;
mod page_setup;
mod pdf_netlist;
mod placements;
mod print_preview;
mod raster_job;
mod sheet_numbers;
mod write;
#[cfg(test)]
pub(crate) use write::settle;
#[cfg(test)]
mod source_tests;
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
#[cfg(test)]
fn build_export_context(
    document_state: &crate::app::state::DocumentState,
) -> Option<ExportContext> {
    build_page_scope(document_state).map(|(ctx, _issues)| ctx)
}

/// The BOM's context: like [`build_export_context`] but carrying the project
/// hierarchy (`netlist_sheets` + placement counts), so a BOM counts every part
/// on every child sheet once per placement (#730). A loose document keeps the
/// exported page scope (every unowned open tab), like the PDF: the hierarchy
/// is dropped so the BOM does not narrow to the active sheet's subtree.
fn build_bom_export_context(
    document_state: &crate::app::state::DocumentState,
) -> Option<ExportContext> {
    build_bom_export_scope(document_state).map(|(ctx, _issues)| ctx)
}

/// The BOM's scope, shared by the preview and the written file so the user
/// sees what gets exported: [`build_export_scope`] plus the loose-document
/// rule (drop the hierarchy, keep every unowned open tab).
fn build_bom_export_scope(
    document_state: &crate::app::state::DocumentState,
) -> Option<(ExportContext, ExportIssues)> {
    build_bom_export_scope_for_source(document_state, document_state.active_path.as_deref()?)
}

/// [`build_bom_export_scope`] for an explicit source document (#852): the BOM
/// completion path, where the dialog may have outlived the originating tab.
fn build_bom_export_scope_for_source(
    document_state: &crate::app::state::DocumentState,
    source: &std::path::Path,
) -> Option<(ExportContext, ExportIssues)> {
    if document_state.document_project(source).is_none() {
        // Walk from the loose hierarchy's tops, not the focused tab, so the
        // count does not depend on focus (#1139).
        let (mut ctx, issues) = loose_bom::loose_bom_scope(document_state, source)?;
        ctx.netlist_sheets.clear();
        ctx.netlist_placements.clear();
        return Some((ctx, issues));
    }
    build_export_scope_for_source(document_state, source)
}

/// Everything that went wrong while deriving the export's netlist.
#[derive(Default)]
pub(crate) struct ExportIssues {
    /// What the stitcher reported in-band.
    pub(crate) stitch: Vec<signex_net::StitchIssue>,
    /// Declared pages with no file at their path at all. A page that does not
    /// exist is also dropped from the exported page set, so the PDF comes out
    /// short a page.
    ///
    /// A page that exists and was read but that nothing hierarchically
    /// references is *not* a shortfall of its own (#430): `project_roots`
    /// names it as an entry point in its own right, so the stitcher's
    /// multi-root traversal walks it as an independent top-level page and a
    /// flat multi-page project's second, third, … page contributes instead of
    /// vanishing.
    pub(crate) missing_pages: Vec<PathBuf>,
    /// Sheets that exist but could not be read: `(path, why)`.
    pub(crate) unreadable: Vec<(PathBuf, String)>,
    /// The project's sheet numbering (#1081), not an issue list: its own
    /// problems never stop an export and are not in [`Self::messages`]; the
    /// PDF and print-preview paths log them. `None` for a loose document.
    pub(crate) sheet_numbering: Option<sheet_numbers::ScopeNumbering>,
    /// The placements the stitcher walked (#1133), to vet a remembered one.
    pub(crate) live: placements::LiveOccurrences,
}

impl ExportIssues {
    /// Whether the derived netlist is missing connectivity rather than merely
    /// oddly named.
    ///
    /// A `MissingChild` subtree does not just leave its own nets out: nets that
    /// should merge through that sheet's ports stay split, so the surviving
    /// nets can carry the *wrong* names. `SheetCycle` truncates the walk with
    /// the same effect, and a declared page with no file behind it is a whole
    /// page of components and nets that never made it in. `SheetKeyCollision`
    /// is a hole too, though a different one: the dropped sheet never entered
    /// the graph at all, so its components and nets are simply absent —
    /// nothing is stitched from the wrong file and nothing is grafted in
    /// twice, which is what the pre-#466 filename-keyed model did and what
    /// this variant's predecessor described. The remaining `StitchIssue`
    /// variants (duplicate UUIDs, shared references, name collisions) describe
    /// a complete netlist with a naming or annotation problem — loud, but not
    /// a hole.
    pub(crate) fn netlist_is_incomplete(&self) -> bool {
        !self.missing_pages.is_empty()
            || !self.unreadable.is_empty()
            || self.stitch.iter().any(|issue| {
                matches!(
                    issue,
                    signex_net::StitchIssue::MissingChild { .. }
                        | signex_net::StitchIssue::SheetCycle { .. }
                        | signex_net::StitchIssue::SheetKeyCollision { .. }
                )
            })
    }

    /// The refusal text when one child file is placed more than once and its
    /// parts keep one shared designator. The Protel file keys every terminal
    /// on the designator alone, so the two physical parts would collapse into
    /// a single `[ ... ]` block with that part's pin listed in two nets.
    ///
    /// Power references (`#PWR*`, `is_power`) are neither refused nor listed:
    /// they get no component block and stay out of the net blocks, so two
    /// placements sharing one cannot merge anything. `ctx` is the scope the
    /// issues came from; its `netlist_sheets` say which references are power.
    pub(crate) fn shared_reference_refusal(&self, ctx: &ExportContext) -> Option<String> {
        let placements = ctx.sheet_placements();
        let is_power = |reference: &str| {
            reference.starts_with("#PWR")
                || placements.iter().any(|(sheet, path)| {
                    sheet
                        .symbols
                        .iter()
                        .any(|sym| sym.is_power && sym.designator_in(path) == reference)
                })
        };
        let shared: std::collections::BTreeSet<String> = self
            .stitch
            .iter()
            .filter_map(|issue| match issue {
                signex_net::StitchIssue::SharedReferenceAcrossInstances {
                    filename,
                    reference,
                } if !is_power(reference) => Some(format!("{reference} (in {filename})")),
                _ => None,
            })
            .collect();
        if shared.is_empty() {
            return None;
        }
        Some(format!(
            concat!(
                "Netlist export refused: a sheet file is placed more than once and its ",
                "designator(s) {} are shared by every placement, so two parts would merge ",
                "into one in the Protel file. Give each placement its own designators ",
                "(or use a separate sheet file per instance) and export again.",
            ),
            shared.into_iter().collect::<Vec<_>>().join(", ")
        ))
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
        out
    }
}

/// Tell the user which characters none of the bundled PDF fonts cover; the
/// page shows an empty box for each.
fn log_uncovered_chars(chars: &[char]) {
    if chars.is_empty() {
        return;
    }
    let listed: String = chars.iter().take(20).collect();
    let more = if chars.len() > 20 { "..." } else { "" };
    crate::diagnostics::log_warning(format!(
        "PDF export: {} character(s) have no glyph in the bundled fonts and were drawn as empty boxes: {listed}{more}",
        chars.len()
    ));
}

/// Put the stitch issues in front of the user — called once per *user action*
/// (print-preview open, PDF written, netlist written), never from the shared
/// context builder.
fn log_stitch_issues(source: &std::path::Path, ctx: &ExportContext, issues: &ExportIssues) {
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
            source.display()
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
                placement: Default::default(),
            })
        })
        .collect()
}

/// Pages for a loose .snxsch: the unowned open tabs, active one first. An open
/// tab belonging to some *other* loaded project must not ride along.
fn loose_pages(
    document_state: &crate::app::state::DocumentState,
    active_path: &std::path::Path,
) -> Vec<SheetSnapshot> {
    let mut paths: Vec<PathBuf> = document_state.unowned_engine_paths();
    paths.sort_by_key(|p| p.as_path() != active_path);
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
                placement: Default::default(),
            })
        })
        .collect()
}

/// Title-block metadata for the export, plus the project's active variant.
/// The canvas title block resolves with the same values (#1079).
pub(crate) fn export_metadata(
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
    // Title-block fields beyond the four with their own metadata slot
    // (#1080); `${DOCUMENTNUMBER}`, `${DRAWNBY}`, … read them from here.
    for key in ["document_number", "drawn_by", "checked_by", "approved_by"] {
        if let Some(value) = tb.get(key) {
            custom_fields.insert(key.to_string(), value.clone());
        }
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
#[cfg(test)]
fn build_export_scope(
    document_state: &crate::app::state::DocumentState,
) -> Option<(ExportContext, ExportIssues)> {
    build_scope(document_state, document_state.active_path.as_deref()?, true)
}

/// [`build_export_scope`] without `ExportContext::netlist_sheets`, for the
/// paths that never read it (PDF, print preview). Skips the per-sheet
/// copy that only the Protel netlist needs.
fn build_page_scope(
    document_state: &crate::app::state::DocumentState,
) -> Option<(ExportContext, ExportIssues)> {
    build_scope(
        document_state,
        document_state.active_path.as_deref()?,
        false,
    )
}

/// [`build_page_scope`] for an explicit source document rather than the active
/// tab — the BOM / PDF completion path, where the OS dialog may have outlived
/// the tab the export was started from (#833). `None` when `source` is no
/// longer open.
fn build_page_scope_for_source(
    document_state: &crate::app::state::DocumentState,
    source: &std::path::Path,
) -> Option<(ExportContext, ExportIssues)> {
    build_scope(document_state, source, false)
}

/// [`build_export_scope`] for an explicit source document (#852): the netlist
/// completion path.
fn build_export_scope_for_source(
    document_state: &crate::app::state::DocumentState,
    source: &std::path::Path,
) -> Option<(ExportContext, ExportIssues)> {
    build_scope(document_state, source, true)
}

fn build_scope(
    document_state: &crate::app::state::DocumentState,
    active_path: &std::path::Path,
    with_netlist_sheets: bool,
) -> Option<(ExportContext, ExportIssues)> {
    let active_engine = document_state.engines.get(active_path)?;

    // Scope comes from `active_document_project()` — the project that owns
    // the active document — NOT `active_loaded_project()`, whose
    // `active_project` pointer is sticky by design and keeps pointing at the
    // last-loaded project while a loose file is focused (#406).
    let owning_project = document_state.document_project(active_path);

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
    if let Some(warning) =
        crate::app::project_sheets::netlist_root_fallback_warning_for(document_state, active_path)
    {
        crate::diagnostics::log_warning(warning);
    }
    let (pages, sheet_set) =
        crate::app::project_sheets::assemble_project_sheets_for(document_state, active_path);

    let sheets = match owning_project {
        Some(project) => owned_pages(project, &pages, &sheet_set),
        None => loose_pages(document_state, active_path),
    };
    let metadata = export_metadata(active_engine, owning_project);
    // The active sheet's page, matched under the project's case rules (#694):
    // `A.snxsch` and `a.snxsch` are one file on a case-insensitive volume, but
    // `signex-output` compares paths exactly. Hand it the page's own spelling.
    let fold = crate::app::state::project_case::fold_of(owning_project);
    let active_key = crate::app::state::scope::path_key_with(active_path, fold);
    let current_sheet = sheets
        .iter()
        .find(|s| crate::app::state::scope::path_key_with(&s.path, fold) == active_key)
        .map_or_else(|| active_path.to_path_buf(), |s| s.path.clone());

    // Derive the authoritative project netlist off that same sheet set, so the
    // netlist exporter reads the contract instead of re-deriving connectivity
    // (ADR-0002 D7). The graph is re-keyed by resolved path, per parent
    // (ADR-0002 D8, #466), not by the bare `ChildSheet.filename` string.
    let mut issues = ExportIssues::default();
    let set = sheet_set;
    let mut netlist_sheets: Vec<signex_types::schematic::SchematicSheet> = Vec::new();
    let mut netlist_placements: Vec<signex_types::net::SheetPlacement> = Vec::new();
    // The occurrence walk and how page paths are keyed against it, kept for
    // the per-placement page copies below.
    let mut page_occurrences: Vec<signex_net::SheetOccurrence> = Vec::new();
    let mut page_keying: Option<(Option<PathBuf>, crate::app::state::scope::CaseFold)> = None;
    let netlist = set.root.clone().and_then(|root_path| {
        // A declared page that exists and was read but that nothing
        // hierarchically references is stitched in below as its own
        // independent top-level page (#430) — see `project_roots` — so it is
        // only a genuine shortfall here when it could not be loaded at all.
        //
        // "No file exists at that path" sends the user to the right place; a
        // page that exists but will not parse is already reported verbatim
        // above (`unreadable`), so it must not also be reported as missing.
        let unreadable: std::collections::HashSet<&PathBuf> =
            set.unreadable.iter().map(|(path, _)| path).collect();
        for path in &set.pages_outside_the_hierarchy {
            if !unreadable.contains(path) && !set.sheets.contains_key(path) {
                issues.missing_pages.push(path.clone());
            }
        }
        issues.unreadable = set.unreadable.clone();
        let project_dir = owning_project.map(|p| p.dir().to_path_buf());
        let base_dir = project_dir
            .clone()
            .or_else(|| root_path.parent().map(PathBuf::from));
        let fold = set.fold;
        let graph = crate::app::project_sheets::project_graph_with(
            &set.sheets,
            base_dir.as_deref(),
            Some(&root_path),
            fold,
        );
        let root_key =
            crate::app::project_sheets::sheet_key_with(&root_path, base_dir.as_deref(), fold);
        if !graph.sheets.contains_key(&root_key) {
            // The root is exempt from losing a SheetKey collision (#536),
            // so reaching here means it was never loaded at all — absent,
            // or present and unparseable. Either way there is no netlist
            // to derive, but the user still has to be told: bailing with
            // `issues` left empty is what produced no `.net`, a PDF whose
            // `NET_NAME()` resolved against nothing, and not one line in
            // the Messages panel.
            issues.stitch.extend(graph.issues);
            crate::diagnostics::log_warning(format!(
                "Netlist: root sheet '{}' is not part of the assembled project, so no netlist \
                 was derived",
                root_path.display()
            ));
            return None;
        }
        // Root + every declared page its hierarchy never reaches (#430) —
        // a flat multi-page project stitches into one netlist only if each
        // page is named as a root of its own.
        let roots = crate::app::project_sheets::project_roots(
            root_key,
            &set,
            &graph,
            base_dir.as_deref(),
            fold,
        );
        // `build_project_netlist` always produces a netlist and reports what
        // it could not stitch in-band. The issues are returned to the caller
        // rather than acted on here, because severity is a per-deliverable
        // policy: the .net refuses on a hole, the PDF proceeds and warns.
        let project = signex_net::ProjectGraph {
            sheets: &graph.sheets,
            resolved: &graph.resolved,
            roots: &roots,
        };
        let result = signex_net::build_project_netlist(&project);
        let occurrences = signex_net::project_occurrences(&project);
        page_occurrences = occurrences.clone();
        page_keying = Some((base_dir.clone(), fold));
        // One project numbering (#1081), from the same walk; loose documents
        // keep the export-order numbers.
        if let Some(owner) = owning_project {
            let (numbering, numbering_issues) =
                signex_net::sheet_numbering(&project, &owner.data.name);
            issues.sheet_numbering = Some(sheet_numbers::ScopeNumbering::new(
                numbering,
                numbering_issues,
                base_dir.clone(),
                fold,
            ));
        }
        // Hand the exporters the very sheets the netlist was derived from, in
        // key order so the output is deterministic: hierarchical children are
        // not printed pages, yet their parts sit in the netlist's terminals.
        if with_netlist_sheets {
            // Placements come from the same occurrence walk the netlist just
            // used: the BOM multiplies a child by how often it is placed
            // (#730) and the Protel file resolves designators per placement.
            // A sheet the walk never reaches has none and counts zero.
            let mut keyed: Vec<_> = graph.sheets.into_iter().collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0));
            let keys: Vec<_> = keyed.iter().map(|(key, _)| key.clone()).collect();
            netlist_placements = placements::netlist_placements(&keys, occurrences);
            netlist_sheets = keyed.into_iter().map(|(_, sheet)| sheet).collect();
        }
        let mut stitch = result.issues;
        stitch.extend(graph.issues);
        issues.stitch = stitch;
        Some(result.netlist)
    });

    // Printed pages follow the hierarchy the netlist was derived from: a sheet
    // reached once prints as that placement, one reached N times prints N page
    // copies (#829). Without a derived hierarchy the pages print as they are.
    let sheets = match &page_keying {
        Some((base_dir, fold)) => {
            let copies =
                placements::page_copies(sheets, &page_occurrences, base_dir.as_deref(), *fold);
            match &issues.sheet_numbering {
                Some(numbering) => numbering.numbered_pages(copies),
                None => copies,
            }
        }
        None => sheets,
    };

    issues.live = placements::LiveOccurrences::new(page_occurrences, page_keying);
    Some((
        ExportContext {
            sheets,
            metadata,
            netlist,
            netlist_sheets,
            netlist_placements,
            current_sheet: Some(current_sheet),
        },
        issues,
    ))
}

impl Signex {
    /// A tab's remembered placement (#1133) that the stitcher no longer
    /// reaches (sheet symbol deleted, rebound, undone) is reported once and
    /// dropped, so the next render or export falls back to the first copy
    /// silently. Only "Current page" uses the placement, so other ranges leave
    /// it alone.
    pub(crate) fn retire_dead_placement(
        &mut self,
        issues: &ExportIssues,
        source: &std::path::Path,
        range: &signex_output::PageRange,
    ) {
        if !matches!(range, signex_output::PageRange::Current) {
            return;
        }
        let Some(placement) = self.document_state.tab_placements.get(source) else {
            return;
        };
        if issues.live.contains(source, placement) {
            return;
        }
        crate::diagnostics::log_warning(format!(
            "Export: {} was opened through sheet placement {placement}, which no longer \
             exists; printing the first placement instead",
            source.display()
        ));
        self.document_state.tab_placements.forget(source);
    }
}

/// A declared page placed several times prints one copy per placement, and
/// "Current page" is the first of them. When the user descended into this tab
/// through one of the placements (#1133), keep only that copy of the page.
fn keep_descended_copy(ctx: &mut ExportContext, placement: Option<&InstancePath>) {
    let Some(placement) = placement else {
        return;
    };
    let current = ctx.current_sheet.clone();
    let has_copy = ctx
        .sheets
        .iter()
        .any(|s| Some(&s.path) == current.as_ref() && &s.placement == placement);
    // A copy that no longer exists leaves the first; `retire_dead_placement`
    // has already told the user and dropped the stale entry.
    if has_copy {
        ctx.sheets
            .retain(|s| Some(&s.path) != current.as_ref() || &s.placement == placement);
    }
}

/// "Current page" for a document that is not a declared page — a hierarchical
/// child opened by descending into a sheet symbol. The declared pages do not
/// contain it, so the plain index lookup would silently print sheet 0. When
/// `range` is `Current` and the viewed sheet is not among `ctx.sheets`, replace
/// the page set with that sheet's live snapshot and return `true` (the caller
/// then skips its file-pick filter, which only names declared pages). Inside a
/// project (`issues.sheet_numbering`) the page prints its placement's project number and
/// count; a loose document prints 1 of 1.
///
/// `descended` is the placement the user came through to reach `source`
/// (#1133), read by the caller when the export was requested. It is used only
/// while `issues.live` still lists it; a placement the walk no longer reaches
/// is ignored and the first-copy guess stands in (the caller reports it once
/// through [`Signex::retire_dead_placement`]).
fn isolate_undeclared_current(
    ctx: &mut ExportContext,
    issues: &ExportIssues,
    document_state: &crate::app::state::DocumentState,
    source: &std::path::Path,
    descended: Option<&InstancePath>,
    range: &signex_output::PageRange,
) -> bool {
    if !matches!(range, signex_output::PageRange::Current) {
        return false;
    }
    let descended = descended.filter(|placement| issues.live.contains(source, placement));
    if ctx
        .sheets
        .iter()
        .any(|s| Some(&s.path) == ctx.current_sheet.as_ref())
    {
        keep_descended_copy(ctx, descended);
        return false;
    }
    let Some(engine) = document_state.engines.get(source) else {
        crate::diagnostics::log_warning(format!(
            "Export: current sheet {} is not open; printing the first page instead",
            source.display()
        ));
        return false;
    };
    let sheet_name = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Sheet")
        .to_string();
    let schematic = engine.document().clone();
    // The placement the user descended through, when one was recorded (#1133).
    // A child opened straight from disk has none, so print the copy a
    // placed-twice child would list first: the smallest occurrence path among
    // its symbols' netlist terminals. A child the netlist never reached stays
    // a root-placement page.
    let placement = match descended {
        Some(recorded) => recorded.clone(),
        None => {
            let uuids: std::collections::HashSet<_> =
                schematic.symbols.iter().map(|s| s.uuid).collect();
            crate::app::state::default_placement(
                ctx.netlist
                    .iter()
                    .flat_map(|netlist| &netlist.nets)
                    .flat_map(|net| &net.terminals)
                    .filter(|t| uuids.contains(&t.symbol))
                    .map(|t| t.occurrence.clone()),
            )
            .unwrap_or_default()
        }
    };
    // A child with no symbol on a net terminal guesses the root path, which no
    // placement of it carries: number it as its first placement instead.
    let (sheet_number, sheet_count) = issues.sheet_numbering.as_ref().map_or((1, 1), |numbering| {
        let number = numbering
            .number_of(source, &placement)
            .or_else(|| numbering.first_number_of(source))
            .unwrap_or(1);
        (number, numbering.count())
    });
    ctx.sheets = vec![
        SheetSnapshot {
            path: source.to_path_buf(),
            schematic,
            sheet_name,
            sheet_number,
            sheet_count,
            placement: Default::default(),
        }
        .for_placement(placement, ""),
    ];
    ctx.current_sheet = Some(source.to_path_buf());
    true
}
