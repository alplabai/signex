//! BOM preview modal handlers. Split from `menu/export.rs`.

use std::path::PathBuf;

use iced::Task;
use signex_output::{BomColumn, BomExporter, BomFormat, BomGrouping, BomOptions, Exporter};

use super::super::super::super::*;

impl Signex {
    /// Open the BOM preview modal — Altium parity with Print Preview.
    /// Builds the rolled-up table from the active project's
    /// schematic snapshot and seeds the modal with the default
    /// options. The user adjusts grouping / include flags / format
    /// in the modal and clicks Export to drive the file dialog.
    pub(crate) fn handle_bom_preview_open(&mut self) -> Task<Message> {
        if !self.document_state.has_active_engine() {
            log::warn!("BOM preview: no active schematic");
            return Task::none();
        }
        let active_variant = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.active_variant.clone())
            .unwrap_or_else(|| "Base".to_string());
        let variants = self
            .document_state
            .active_document_project()
            .map(|p| p.data.variant_definitions.clone())
            .unwrap_or_default();
        let opts = BomOptions {
            active_variant: if active_variant.eq_ignore_ascii_case("Base") {
                None
            } else {
                Some(active_variant)
            },
            ..BomOptions::default()
        };
        let Some(table) = self.rebuild_bom_table(&opts) else {
            self.document_state.export_error =
                Some("Cannot build BOM: no active schematic.".to_string());
            return Task::none();
        };
        // Default sort: ascending by the Designator column when it
        // exists in the column set. Matches Altium's "rows are
        // ordered by reference designator on open" convention.
        let default_sort = opts
            .columns
            .iter()
            .position(|c| matches!(c, signex_output::BomColumn::Designator))
            .map(|idx| (idx, true));
        self.document_state.bom_preview = Some(crate::app::state::BomPreviewState {
            options: opts,
            table,
            variants,
            sort: default_sort,
            column_drag: None,
            column_drag_press_x: None,
            column_hover: None,
            column_widths: std::collections::HashMap::new(),
            column_resize: None,
            sidebar_tab: crate::app::state::BomSidebarTab::General,
        });
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

    pub(crate) fn handle_bom_preview_set_include_dnp(&mut self, include: bool) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.include_dnp = include;
        }
        self.rerollup_bom_preview();
    }

    pub(crate) fn handle_bom_preview_set_include_not_fitted(&mut self, include: bool) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            preview.options.include_not_fitted = include;
        }
        self.rerollup_bom_preview();
    }

    /// Flip a column's presence in `BomOptions.columns`. Removing a
    /// column drops it from the preview + export; adding pushes it
    /// to the end of the display order. Re-adding a column the user
    /// previously removed lands it at the end (predictable;
    /// preserving the original slot would require a separate
    /// "available columns" Vec).
    pub(crate) fn handle_bom_preview_toggle_column(&mut self, col: BomColumn) {
        if let Some(preview) = self.document_state.bom_preview.as_mut() {
            if let Some(idx) = preview.options.columns.iter().position(|c| c == &col) {
                preview.options.columns.remove(idx);
            } else {
                preview.options.columns.push(col);
            }
        }
        // Column toggles don't change the rolled-up table, only the
        // displayed columns + the export columns. No rerollup needed.
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
            if let Some(src) = preview.column_drag.take() {
                if src != dest
                    && src < preview.options.columns.len()
                    && dest < preview.options.columns.len()
                {
                    let col = preview.options.columns.remove(src);
                    let insert_at = if src < dest { dest } else { dest };
                    let insert_at = insert_at.min(preview.options.columns.len());
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
    }

    /// User clicked Export inside the BOM preview modal — stash the
    /// live options on the document, kick off the file dialog, and
    /// finish in `handle_export_bom_finished`. Mirrors the
    /// PrintPreview → Export PDF pattern; without `pending_bom_options`
    /// the finish handler would fall back to defaults and the user's
    /// column / grouping / variant picks would silently disappear
    /// between modal-close and file-write.
    pub(crate) fn handle_bom_preview_export(&mut self) -> Option<Task<Message>> {
        let Some(preview) = self.document_state.bom_preview.as_ref() else {
            return Some(Task::none());
        };
        let options = preview.options.clone();
        let (default_name, format_filter) = match options.format {
            BomFormat::Csv => ("bom.csv", ("CSV (.csv)", &["csv"][..])),
            BomFormat::Xlsx => ("bom.xlsx", ("Excel (.xlsx)", &["xlsx"][..])),
            BomFormat::Html => ("bom.html", ("HTML (.html)", &["html", "htm"][..])),
        };
        let (filter_label, filter_exts) = format_filter;
        let default_name_owned = default_name.to_string();
        let filter_label_owned = filter_label.to_string();
        let filter_exts_owned: Vec<String> = filter_exts.iter().map(|s| s.to_string()).collect();

        // Stash the live options + dismiss the preview state. The
        // detached preview window also needs to close so the OS
        // window doesn't linger after Export — same shape as
        // `handle_print_preview_export`.
        self.document_state.pending_bom_options = Some(options);
        self.document_state.bom_preview = None;
        let close_window = self.close_detached_modal(crate::app::state::ModalId::BomPreview);

        let dialog = Task::perform(
            async move {
                let exts_refs: Vec<&str> = filter_exts_owned.iter().map(String::as_str).collect();
                rfd::AsyncFileDialog::new()
                    .set_title("Export Bill of Materials")
                    .add_filter(filter_label_owned.as_str(), &exts_refs)
                    .set_file_name(default_name_owned.as_str())
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| {
                if let Some(path) = path {
                    Message::Export(ExportMsg::BomFinished(Ok(path)))
                } else {
                    Message::Noop
                }
            },
        );
        Some(Task::batch([close_window, dialog]))
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
    fn rebuild_bom_table(&self, opts: &BomOptions) -> Option<signex_output::BomTable> {
        let ctx = super::build_export_context(&self.document_state)?;
        Some(signex_output::bom::rollup(&ctx, opts))
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
        if let Some(table) = self.rebuild_bom_table(&opts)
            && let Some(preview) = self.document_state.bom_preview.as_mut()
        {
            preview.table = table;
        }
    }

    pub(crate) fn handle_export_bom_finished(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                // Cancelled — drop any pending options so the next
                // open of the modal starts fresh.
                self.document_state.pending_bom_options = None;
                log::info!("BOM export cancelled: {e}");
                return Task::none();
            }
        };

        let ctx = match super::build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                self.document_state.pending_bom_options = None;
                self.document_state.export_error =
                    Some("Cannot export BOM: no active schematic.".to_string());
                return Task::none();
            }
        };

        // Honour the user's picks from the BOM preview modal when
        // present. The pending slot is populated by
        // `handle_bom_preview_export`; falling back here covers any
        // hypothetical future caller (none today) that drives the
        // export message directly without going through the preview.
        let format_from_path = BomFormat::from_output_path(&save_path);
        let opts = match self.document_state.pending_bom_options.take() {
            Some(mut user_opts) => {
                // Picker filter sets a default extension; the user
                // may rename the file in the dialog. Trust the
                // saved-as extension over the picker's intent.
                user_opts.format = format_from_path;
                user_opts
            }
            None => {
                let active_variant = self
                    .document_state
                    .active_document_project()
                    .and_then(|p| p.data.active_variant.clone())
                    .unwrap_or_else(|| "Base".to_string());
                BomOptions {
                    columns: vec![
                        BomColumn::Name,
                        BomColumn::Description,
                        BomColumn::Designator,
                        BomColumn::Footprint,
                        BomColumn::LibRef,
                        BomColumn::Qty,
                    ],
                    grouping: BomGrouping::Grouped,
                    format: format_from_path,
                    include_dnp: false,
                    include_not_fitted: false,
                    active_variant: if active_variant.eq_ignore_ascii_case("Base") {
                        None
                    } else {
                        Some(active_variant)
                    },
                    rule_options: Default::default(),
                }
            }
        };

        match BomExporter.export(&ctx, &opts) {
            Ok(output) => match std::fs::write(&save_path, &output.bytes) {
                Ok(()) => log::info!(
                    "Wrote {} ({} bytes)",
                    save_path.display(),
                    output.bytes.len(),
                ),
                Err(e) => {
                    self.document_state.export_error = Some(format!(
                        "Could not write BOM to {}:\n{e}",
                        save_path.display(),
                    ));
                }
            },
            Err(e) => {
                self.document_state.export_error = Some(format!("BOM export failed: {e}"));
            }
        }

        Task::none()
    }
}
