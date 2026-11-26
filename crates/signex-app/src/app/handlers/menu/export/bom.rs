//! BOM preview modal handlers. Split from `menu/export.rs`.

use std::path::PathBuf;

use iced::Task;
use signex_output::{BomColumn, BomExporter, BomFormat, BomGrouping, BomOptions, Exporter};

use super::super::super::super::*;

#[cfg(test)]
#[path = "bom_computed_tests.rs"]
mod computed_tests;
#[path = "bom_lines.rs"]
mod lines;
#[path = "bom_rows.rs"]
mod rows;
#[cfg(test)]
#[path = "bom_tests.rs"]
mod tests;

impl Signex {
    /// Open the BOM preview modal, the export dialog of mockup row 112.
    /// Builds the rolled-up table from the active project's
    /// schematic snapshot and seeds the modal with the default
    /// options. The user adjusts grouping / include flags / format
    /// in the modal and clicks Export to drive the file dialog.
    pub(crate) fn handle_bom_preview_open(&mut self) -> Task<Message> {
        let anchor = self.document_state.active_path.clone();
        if !self.document_state.has_active_engine() {
            self.commit_drafts_in_scope_of(anchor.as_deref());
            if let Some(anchor) = anchor.as_deref() {
                self.report_part_field_issues_in_scope_of(anchor);
            }
            log::warn!("BOM preview: no active schematic");
            return Task::none();
        }
        // Pin the preview to the document it is built for; Export writes
        // this one even after a tab switch (#852).
        let Some(source) = anchor else {
            crate::diagnostics::log_warning("BOM preview: no active document to export");
            return Task::none();
        };
        let active_variant = self
            .document_state
            .document_project(&source)
            .and_then(|p| p.data.active_variant.clone());
        self.open_bom_preview_in(source, active_variant)
    }

    /// The BOM preview of the sheets `source` (an open sheet) belongs to,
    /// with `variant` picked in its variant select (`None` or Base's name is
    /// Base): the dialog Open in BOM Studio opens from the Variant Manager
    /// (#2949) and File ▸ Reports ▸ BOM opens on the active variant.
    pub(crate) fn open_bom_preview_in(
        &mut self,
        source: PathBuf,
        variant: Option<String>,
    ) -> Task<Message> {
        self.commit_drafts_in_scope_of(Some(&source));
        self.report_part_field_issues_in_scope_of(&source);
        let variants = self
            .document_state
            .document_project(&source)
            .map(|p| p.data.variant_definitions.clone())
            .unwrap_or_default();
        let opts = BomOptions {
            active_variant: variant.filter(|name| !signex_types::project::is_base_variant(name)),
            ..BomOptions::default()
        };
        let Some((ctx, issues)) =
            super::build_bom_export_scope_for_source(&self.document_state, &source)
        else {
            self.refuse_export(signex_i18n::ids::NOTICE_BOM_NO_SHEET);
            return Task::none();
        };
        if self.refused_for_channel_naming(&issues) {
            return Task::none();
        }
        let (table, validation_report) = self.bom_table_of(ctx, &opts);
        // Default sort: ascending by the Designator column when it exists
        // in the column set, so lines open in designator order.
        let default_sort = opts
            .columns
            .iter()
            .position(|c| matches!(c, signex_output::BomColumn::Designator))
            .map(|idx| (idx, true));
        let mut preview = crate::app::state::BomPreviewState {
            row_order: Vec::new(),
            source,
            options: opts,
            table,
            validation_report,
            variants,
            sort: default_sort,
            column_drag: None,
            column_drag_press_x: None,
            column_hover: None,
            column_widths: std::collections::HashMap::new(),
            column_resize: None,
            sidebar_tab: crate::app::state::BomSidebarTab::General,
            filter: String::new(),
            visible_lines: Vec::new(),
            row_flags: Vec::new(),
            issue_lines: Vec::new(),
            selected_row: None,
            scroll_x: 0.0,
            export_pending: false,
        };
        preview.refresh_rows();
        self.document_state.bom_preview = Some(preview);
        self.handle_detach_modal(crate::app::state::ModalId::BomPreview)
    }

    pub(crate) fn handle_bom_preview_set_grouping(&mut self, grouping: BomGrouping) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.grouping = grouping;
        }
        self.rerollup_bom_preview();
    }

    pub(crate) fn handle_bom_preview_set_format(&mut self, format: BomFormat) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.format = format;
        }
    }

    /// The one not-fitted option over the two domain flags, which it always
    /// sets together (docs/DECISIONS.md, 2026-11-19).
    pub(crate) fn handle_bom_preview_include_not_fitted(&mut self, include: bool) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.include_dnp = include;
            preview.options.include_not_fitted = include;
        }
        self.rerollup_bom_preview();
    }

    /// Select the line an issue points at and scroll it into view. A filter
    /// that hides the line is cleared, so the selection is never off screen.
    pub(crate) fn handle_bom_preview_issue_selected(&mut self, issue: usize) -> Task<Message> {
        let Some(preview) = self.document_state.bom_preview.as_mut() else {
            return Task::none();
        };
        let Some(Some(at)) = preview.issue_lines.get(issue).copied() else {
            return Task::none();
        };
        preview.selected_row = Some(at.row);
        let line = at.line.saturating_sub(1);
        if !preview.visible_lines.contains(&line) {
            preview.filter.clear();
            preview.refresh_rows();
        }
        let Some(position) = preview.visible_lines.iter().position(|&l| l == line) else {
            return Task::none();
        };
        let y = position as f32 * crate::styles::ChromeMetrics::DEFAULT.field;
        iced::widget::operation::scrollable::scroll_to(
            crate::app::state::BomPreviewState::BODY_SCROLL,
            iced::widget::scrollable::AbsoluteOffset {
                x: None,
                y: Some(y),
            },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// The table body scrolled: move the sticky header to the same
    /// horizontal offset.
    pub(crate) fn handle_bom_preview_body_scrolled(&mut self, x: f32) -> Task<Message> {
        let Some(preview) = self.document_state.bom_preview.as_mut() else {
            return Task::none();
        };
        if preview.scroll_x == x {
            return Task::none();
        }
        preview.scroll_x = x;
        iced::widget::operation::scrollable::scroll_to(
            crate::app::state::BomPreviewState::HEADER_SCROLL,
            iced::widget::scrollable::AbsoluteOffset {
                x: Some(x),
                y: None,
            },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// Turn one validation rule on or off and re-validate. Rule flags do not
    /// change the rolled-up table, so no re-rollup happens.
    pub(crate) fn handle_bom_preview_set_rule(
        &mut self,
        rule: signex_output::BomRule,
        enabled: bool,
    ) {
        let Some(preview) = self.document_state.bom_preview.as_mut() else {
            return;
        };
        preview.options.rule_options.set_enabled(rule, enabled);
        let mut opts = preview.options.clone();
        opts.board_left_out = self.active_board_left_out_of_bom();
        let Some(ctx) = super::build_bom_export_context(&self.document_state) else {
            return;
        };
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.validation_report = signex_output::bom::validate(&ctx, &opts, &preview.table);
        }
    }

    /// Flip a column's presence in `BomOptions.columns`. Removing a
    /// column drops it from the preview + export; adding pushes it
    /// to the end of the display order. Re-adding a column the user
    /// previously removed lands it at the end (predictable;
    /// preserving the original slot would require a separate
    /// "available columns" Vec).
    pub(crate) fn handle_bom_preview_toggle_column(&mut self, col: BomColumn) {
        let is_computed = matches!(col, BomColumn::Computed(_));
        let mut added = false;
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            if let Some(idx) = preview.options.columns.iter().position(|c| c == &col) {
                preview.options.columns.remove(idx);
            } else {
                preview.options.columns.push(col);
                added = true;
            }
        }
        // A column toggle changes only the shown and exported columns, except
        // that a computed column's cells are evaluated in the rollup (#1988).
        if added && is_computed {
            self.rerollup_bom_preview();
            self.report_computed_bom_issues();
        }
    }

    /// One Messages line per computed cell of the preview that failed
    /// (#1988), naming its line and the error.
    pub(crate) fn report_computed_bom_issues(&self) {
        let Some(preview) = self.document_state.bom_preview.as_ref() else {
            return;
        };
        for issue in signex_output::bom::computed_issues(&preview.table, &preview.options.columns) {
            crate::diagnostics::log_warning(issue.to_string());
        }
    }

    /// Switch the active variant in the BOM preview. `None` is the
    /// "Base" view (no variant override). Triggers a rerollup since
    /// variant_fitted resolution can flip qty totals.
    pub(crate) fn handle_bom_preview_set_variant(&mut self, variant: Option<String>) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.active_variant = variant;
        }
        self.rerollup_bom_preview();
    }

    /// Click on a header cell — cycle through ascending → descending
    /// → no-sort. The previewed table rows get sorted live in
    /// `view_bom_preview`; we don't re-export-order the underlying
    /// table because sorting is a render-only convenience and the
    /// final exported file should mirror the user's chosen sort, not
    /// the rollup default.
    pub(crate) fn handle_bom_preview_sort_column(&mut self, idx: usize) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            // Click-no-move: arrived here via the on_release
            // sort branch. Clear the drag state so the highlight
            // doesn't linger.
            preview.column_drag = None;
            preview.column_drag_press_x = None;
            preview.sort = match preview.sort {
                Some((cur, true)) if cur == idx => Some((idx, false)),
                Some((cur, false)) if cur == idx => None,
                _ => Some((idx, true)),
            };
        }
    }

    pub(crate) fn handle_bom_preview_column_drag_start(&mut self, idx: usize) {
        let press_x = self.interaction_state.last_mouse_pos.0;
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.column_drag = Some(idx);
            preview.column_drag_press_x = Some(press_x);
        }
    }

    pub(crate) fn handle_bom_preview_column_drag_drop(&mut self, dest: usize) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.column_drag_press_x = None;
            if let Some(src) = preview.column_drag.take()
                && src != dest
                && src < preview.options.columns.len()
                && dest < preview.options.columns.len()
            {
                let col = preview.options.columns.remove(src);
                // The dropped column takes the slot of the header it was
                // released on, in both directions (#592): `dest` is that
                // header's index in the pre-removal list, and inserting
                // the column back at `dest` puts it exactly there.
                //   - dragging right (src < dest): the columns between
                //     shift one left, so the target header ends up just
                //     before the dropped column.
                //   - dragging left (src > dest): they shift one right, so
                //     the target header ends up just after it.
                // (`dest - 1` would instead insert *before* the target when
                // dragging right, the convention for a drop line drawn
                // between headers, which this header-drop UI has no use for.)
                let insert_at = dest.min(preview.options.columns.len());
                preview.options.columns.insert(insert_at, col);
                // Sort spec follows the moved column. If the
                // sort was on a different column, indices may
                // have shifted under it.
                if let Some((sort_idx, asc)) = preview.sort {
                    let new_idx = if sort_idx == src {
                        insert_at
                    } else if src < sort_idx && insert_at >= sort_idx {
                        sort_idx - 1
                    } else if src > sort_idx && insert_at <= sort_idx {
                        sort_idx + 1
                    } else {
                        sort_idx
                    };
                    preview.sort = Some((new_idx, asc));
                }
            }
        }
    }

    /// User clicked Export inside the BOM preview modal — capture the live
    /// options and the preview's source document, kick off the file dialog,
    /// and finish in `handle_export_bom_finished`. Mirrors the
    /// PrintPreview → Export PDF pattern.
    pub(crate) fn handle_bom_preview_export(&mut self) -> Option<Task<Message>> {
        let Some(preview) = self.document_state.bom_preview.as_mut() else {
            return Some(Task::none());
        };
        preview.export_pending = true;
        // A column drag cannot drop while the settings wait for the save.
        preview.column_drag = None;
        preview.column_drag_press_x = None;
        let options = preview.options.clone();
        let source = preview.source.clone();
        let (ext, format_filter) = bom_format_filter(options.format);
        let (filter_label, filter_exts) = format_filter;
        let target = self.save_target(Some(&source));
        let picker = self.ui_state.io.picker.clone();
        let default_name_owned = target.file_name("bom", "-bom", ext);
        let filter_label_owned = self.ui_state.i18n.message_plain(filter_label, &[]);
        let base = picker.dialog().title_in(
            &self.ui_state.i18n,
            signex_i18n::ids::FILE_DIALOG_EXPORT_BOM,
        );
        let filter_exts_owned: Vec<String> = filter_exts.iter().map(|s| s.to_string()).collect();

        // The options and source ride inside `BomFinished`, so a second
        // Export click gets its own copy (#852). The dialog stays open: a
        // cancelled save keeps its settings, and `handle_bom_written` closes
        // it once the file is written.
        Some(Task::perform(
            async move {
                let exts_refs: Vec<&str> = filter_exts_owned.iter().map(String::as_str).collect();
                target
                    .dialog(base)
                    .await
                    .add_filter(filter_label_owned.as_str(), &exts_refs)
                    .set_file_name(default_name_owned.as_str())
                    .save_file()
                    .await
            },
            move |path| {
                Message::Export(ExportMsg::BomFinished {
                    source,
                    options: Box::new(options),
                    result: path.ok_or_else(|| "cancelled".to_string()),
                })
            },
        ))
    }

    /// Whether `msg` would change what Export writes while an Export is in
    /// its save dialog or write. Those edits are not taken (#2792); viewing
    /// the table, and Cancel, still work.
    pub(crate) fn bom_edit_waits_for_save(&self, msg: &BomPreviewMsg) -> bool {
        let pending = self
            .document_state
            .bom_preview
            .as_ref()
            .is_some_and(|preview| preview.export_pending);
        pending
            && matches!(
                msg,
                BomPreviewMsg::SetGrouping(..)
                    | BomPreviewMsg::SetFormat(..)
                    | BomPreviewMsg::NotFittedIncludedToggled(..)
                    | BomPreviewMsg::SetRule(..)
                    | BomPreviewMsg::ToggleColumn(..)
                    | BomPreviewMsg::SetVariant(..)
                    | BomPreviewMsg::ColumnDragStart(..)
                    | BomPreviewMsg::ColumnDragDrop(..)
                    | BomPreviewMsg::Export
            )
    }

    /// The Export of `source` ended without closing its dialog: it takes
    /// edits again.
    fn end_bom_export(&mut self, source: &std::path::Path) {
        if let Some(preview) = self.document_state.bom_preview.as_mut()
            && preview.source == source
        {
            preview.export_pending = false;
        }
    }

    /// A BOM's write landed: report it, and close the dialog open for its
    /// document only when the file was written.
    pub(crate) fn handle_bom_written(
        &mut self,
        source: PathBuf,
        outcome: crate::app::contracts::ExportWriteOutcome,
    ) -> Task<Message> {
        self.end_bom_export(&source);
        let is_written = outcome.result.is_ok();
        let reported = self.handle_export_written(outcome);
        let is_open_for_source = self
            .document_state
            .bom_preview
            .as_ref()
            .is_some_and(|preview| preview.source == source);
        if is_written && is_open_for_source {
            Task::batch([reported, self.handle_bom_preview_close()])
        } else {
            reported
        }
    }

    pub(crate) fn handle_bom_preview_close(&mut self) -> Task<Message> {
        self.document_state.bom_preview = None;
        self.ui_state
            .modal_offsets
            .remove(&crate::app::state::ModalId::BomPreview);
        self.ui_state.modal_dragging = None;
        self.close_detached_modal(crate::app::state::ModalId::BomPreview)
    }

    /// Build (or rebuild) the BomTable from the current document
    /// state and the supplied options. Returns `None` when there's
    /// no active schematic to roll up.
    fn rebuild_bom_table(
        &self,
        opts: &BomOptions,
    ) -> Option<(signex_output::BomTable, signex_output::BomValidationReport)> {
        let ctx = super::build_bom_export_context(&self.document_state)?;
        Some(self.bom_table_of(ctx, opts))
    }

    /// The rolled-up table and its validation of an assembled scope.
    pub(crate) fn bom_table_of(
        &self,
        mut ctx: signex_output::ExportContext,
        opts: &BomOptions,
    ) -> (signex_output::BomTable, signex_output::BomValidationReport) {
        crate::library::drift::apply_library_names(&mut ctx, &self.library.drift.names);
        let opts = BomOptions {
            price_book: crate::library::bom_prices::price_book(&self.library),
            board_left_out: self.active_board_left_out_of_bom(),
            alternates: self
                .document_state
                .active_path
                .as_deref()
                .map(|source| self.bom_alternates_of(source))
                .unwrap_or_default(),
            ..opts.clone()
        };
        signex_output::bom::rollup_with_validation(&ctx, &opts)
    }

    fn rerollup_bom_preview(&mut self) {
        let Some(opts) = self
            .document_state
            .bom_preview
            .as_ref()
            .map(|p| p.options.clone())
        else {
            return;
        };
        if let Some((table, report)) = self.rebuild_bom_table(&opts)
            && let Some(preview) = self.document_state.bom_preview.as_mut()
        {
            preview.table = table;
            preview.validation_report = report;
            // Row indexes name lines of the old table.
            preview.selected_row = None;
        }
    }

    pub(crate) fn handle_export_bom_finished(
        &mut self,
        source: PathBuf,
        mut opts: BomOptions,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("BOM export not written: {e}");
                self.end_bom_export(&source);
                return Task::none();
            }
        };

        let (mut ctx, issues) =
            match super::build_bom_export_scope_for_source(&self.document_state, &source) {
                Some(c) => c,
                None => {
                    self.end_bom_export(&source);
                    self.refuse_export(
                        signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_BOM_SOURCE_CLOSED)
                            .arg("path", source.display().to_string()),
                    );
                    return Task::none();
                }
            };
        if self.refused_for_channel_naming(&issues) {
            self.end_bom_export(&source);
            return Task::none();
        }
        // A BOM is a machine-and-procurement-consumed deliverable, not the
        // print-preview intermediate `rebuild_bom_table` renders — so unlike
        // that path it must not stay silent. A cross-dir child-filename
        // collision (or a missing/cyclic child) can roll the BOM up from the
        // *wrong* subtree — wrong parts, wrong quantities. Surface it loudly,
        // once, from this user action, exactly like the PDF/netlist
        // deliverables (ADR-0002 D8: degradation is reported, never silent).
        super::log_stitch_issues(&source, &ctx, &issues);
        crate::library::drift::apply_library_names(&mut ctx, &self.library.drift.names);
        // A project whose hierarchy could not be derived falls back to the
        // printed pages only, so child-sheet parts would be missing. Say so in
        // BOM terms; the netlist warning does not tell a BOM user that.
        if self.document_state.document_project(&source).is_some() && ctx.netlist_sheets.is_empty()
        {
            crate::diagnostics::log_warning(
                "BOM: project hierarchy could not be derived - parts on child sheets are not included",
            );
        }

        // Picker filter sets a default extension; the user may rename the
        // file in the dialog. Trust the saved-as extension over the
        // picker's intent.
        opts.format = BomFormat::from_output_path(&save_path);
        opts.price_book = crate::library::bom_prices::price_book(&self.library);
        opts.board_left_out = self.board_left_out_of_bom(&source);
        opts.alternates = self.bom_alternates_of(&source);

        match BomExporter.export(&ctx, &opts) {
            Ok(output) => {
                let success_log = format!(
                    "Wrote {} ({} bytes)",
                    save_path.display(),
                    output.bytes.len(),
                );
                // Advisory, not a gate: the file is written; tell the
                // user what validation found instead of dropping it.
                let advisory = bom_validation_notice(&output.validation_report, &save_path);
                // A computed cell that failed is written empty (#1988).
                for issue in &output.computed_issues {
                    crate::diagnostics::log_warning(issue.to_string());
                }
                let write = self.write_export(
                    save_path,
                    output.bytes,
                    "BOM",
                    success_log,
                    advisory,
                    Vec::new(),
                );
                return write.map(move |message| match message {
                    Message::Export(ExportMsg::FileWritten(outcome)) => {
                        Message::Export(ExportMsg::BomWritten {
                            source: source.clone(),
                            outcome,
                        })
                    }
                    other => other,
                });
            }
            Err(e) => {
                self.end_bom_export(&source);
                self.report_export_failure(
                    signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_BOM_EXPORT_FAILED)
                        .arg("error", e.to_string()),
                );
            }
        }

        Task::none()
    }

    /// [`Self::board_left_out_of_bom`] for the document in view.
    /// The alternates per line of `source`'s project (#2938), for the
    /// Alternates column; none for a sheet outside a project.
    fn bom_alternates_of(
        &self,
        source: &std::path::Path,
    ) -> Vec<signex_types::project::BomLineAlternates> {
        self.document_state
            .document_project(source)
            .map(|project| project.data.bom_alternates.clone())
            .unwrap_or_default()
    }

    fn active_board_left_out_of_bom(&self) -> Vec<signex_types::pcb::SymbolLink> {
        self.document_state
            .active_path
            .as_deref()
            .map(|source| self.board_left_out_of_bom(source))
            .unwrap_or_default()
    }

    /// The parts whose footprint on the board of `source`'s project is of a
    /// type the BOM leaves out (#2365). Read from the board while it is
    /// open; a board that is not open leaves every part in.
    fn board_left_out_of_bom(
        &self,
        source: &std::path::Path,
    ) -> Vec<signex_types::pcb::SymbolLink> {
        let docs = &self.document_state;
        let Some(project) = docs.document_project(source) else {
            return Vec::new();
        };
        let Some(file) = project
            .data
            .pcb_file
            .as_deref()
            .map(str::trim)
            .filter(|f| !f.is_empty())
        else {
            return Vec::new();
        };
        docs.pcb_board(&project.dir().join(file))
            .map(|board| {
                board
                    .footprints
                    .iter()
                    .filter(|fp| !fp.component.component_type.in_bom())
                    .filter_map(|fp| fp.schematic_link.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The file extension a BOM format is saved under, and its save-dialog
/// filter: the catalog name and the extensions it lists.
pub(super) fn bom_format_filter(
    format: BomFormat,
) -> (
    &'static str,
    (signex_i18n::MessageId, &'static [&'static str]),
) {
    use signex_i18n::ids;
    match format {
        BomFormat::Csv => ("csv", (ids::FILE_FILTER_CSV_EXT, &["csv"])),
        BomFormat::Xlsx => ("xlsx", (ids::FILE_FILTER_XLSX_EXT, &["xlsx"])),
        BomFormat::Html => ("html", (ids::FILE_FILTER_HTML_EXT, &["html", "htm"])),
    }
}

/// Summary card shown after a BOM export whose validation found issues;
/// `None` when the report is empty.
fn bom_validation_notice(
    report: &signex_output::BomValidationReport,
    saved: &std::path::Path,
) -> Option<crate::app::state::ErrorNotice> {
    use signex_i18n::{Translatable, ids};
    const SHOWN: usize = 8;
    let summary = report.summary()?;
    // The issue lines are the validator's own English (#2061).
    let issues: String = report
        .issues
        .iter()
        .take(SHOWN)
        .map(|issue| format!("\n- {}", issue.message))
        .collect();
    let more = report.issues.len().saturating_sub(SHOWN);
    let id = if more > 0 {
        ids::NOTICE_BOM_ISSUES_MORE
    } else {
        ids::NOTICE_BOM_ISSUES
    };
    Some(crate::app::state::ErrorNotice::export_advisory(
        Translatable::new(id)
            .arg("path", saved.display().to_string())
            .arg("summary", summary)
            .arg("issues", issues)
            .arg("more", more),
    ))
}
