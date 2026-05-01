use std::path::PathBuf;

use iced::Task;
use signex_output::{
    BomColumn, BomExporter, BomFormat, BomGrouping, BomOptions, ExportContext, Exporter,
    NetlistExporter, NetlistOptions, PageRange, PageSize, PdfExporter, PdfOptions, PreviewOptions,
    PreviewRasterizer, ProjectMetadata, SheetSnapshot,
};

use super::super::super::*;

impl Signex {
    pub(crate) fn handle_export_pdf_open_dialog(&mut self) -> iced::Task<Message> {
        if !self.document_state.has_active_engine() {
            log::warn!("PDF export: no active schematic");
            return iced::Task::none();
        }
        // PDF Export now opens the unified Print Preview modal — the
        // user gets the rasterized preview AND every PDF setting in a
        // single overlay instead of a settings-only dialog. The
        // Print-Preview "Export PDF" button drives the file picker
        // directly (see `handle_print_preview_export`).
        self.handle_print_preview_requested()
    }

    pub(crate) fn handle_export_pdf_finished(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                // Cancelled — drop pending state so the next open
                // starts fresh.
                self.document_state.pending_pdf_options = None;
                self.document_state.pending_pdf_files = None;
                log::info!("PDF export cancelled: {e}");
                return Task::none();
            }
        };

        let mut ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                self.document_state.pending_pdf_options = None;
                self.document_state.pending_pdf_files = None;
                self.document_state.export_error =
                    Some("Cannot export PDF: no active schematic.".to_string());
                return Task::none();
            }
        };

        // Use pending options if they were set by the dialog, otherwise fall back to defaults.
        let options = self
            .document_state
            .pending_pdf_options
            .take()
            .unwrap_or_default();
        // Filter sheets to the user's file picks. The preview path
        // populates `pending_pdf_files`; legacy direct-export
        // callers (none today) leave it None and get the full
        // sheet list.
        if let Some(files) = self.document_state.pending_pdf_files.take() {
            ctx.sheets.retain(|s| files.contains(&s.path));
            if ctx.sheets.is_empty() {
                self.document_state.export_error = Some(
                    "Cannot export PDF: no files selected in the Settings tab."
                        .to_string(),
                );
                return Task::none();
            }
        }
        // Apply the variant override the user picked in the modal.
        // The PDF renderer reads `active_variant` out of
        // `metadata.custom_fields` for title-block substitution; this
        // keeps the PDF in sync with the modal's variant picker even
        // when the project's active variant is something else.
        if let Some(variant) = &options.variant {
            if variant.eq_ignore_ascii_case("Base") {
                ctx.metadata.custom_fields.remove("active_variant");
            } else {
                ctx.metadata
                    .custom_fields
                    .insert("active_variant".to_string(), variant.clone());
            }
        }

        match PdfExporter.export(&ctx, &options) {
            Ok(output) => match std::fs::write(&save_path, &output.bytes) {
                Ok(()) => log::info!(
                    "Wrote {} ({} page(s), {} bytes)",
                    save_path.display(),
                    output.page_count,
                    output.bytes.len(),
                ),
                Err(e) => {
                    self.document_state.export_error = Some(format!(
                        "Could not write PDF to {}:\n{e}",
                        save_path.display(),
                    ));
                }
            },
            Err(e) => {
                self.document_state.export_error = Some(format!("PDF export failed: {e}"));
            }
        }

        Task::none()
    }

    pub(crate) fn handle_export_netlist_requested(&mut self) -> Option<Task<Message>> {
        if !self.document_state.has_active_engine() {
            log::warn!("Netlist export: no active schematic");
            return Some(Task::none());
        }

        Some(Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Export Netlist")
                    .add_filter("Netlist", &["net"])
                    .set_file_name("schematic.net")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| {
                if let Some(path) = path {
                    Message::ExportNetlistFinished(Ok(path))
                } else {
                    Message::Noop
                }
            },
        ))
    }

    pub(crate) fn handle_export_netlist_finished(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("Netlist export cancelled: {e}");
                return Task::none();
            }
        };

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                log::warn!("Netlist export: no active schematic");
                return Task::none();
            }
        };

        match NetlistExporter.export(&ctx, &NetlistOptions::default()) {
            Ok(output) => match std::fs::write(&save_path, &output.bytes) {
                Ok(()) => log::info!(
                    "Wrote {} ({} bytes)",
                    save_path.display(),
                    output.bytes.len(),
                ),
                Err(e) => {
                    self.document_state.export_error = Some(format!(
                        "Could not write netlist to {}:\n{e}",
                        save_path.display(),
                    ));
                }
            },
            Err(e) => {
                self.document_state.export_error = Some(format!("Netlist export failed: {e}"));
            }
        }

        Task::none()
    }

    pub(crate) fn handle_dismiss_export_error(&mut self) {
        self.document_state.export_error = None;
    }

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
            .active_loaded_project()
            .and_then(|p| p.data.active_variant.clone())
            .unwrap_or_else(|| "Base".to_string());
        let variants = self
            .document_state
            .active_loaded_project()
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
        self.document_state.bom_preview = Some(crate::app::states::BomPreviewState {
            options: opts,
            table,
            variants,
            sort: default_sort,
            column_drag: None,
            column_drag_press_x: None,
            column_hover: None,
            column_widths: std::collections::HashMap::new(),
            column_resize: None,
            sidebar_tab: crate::app::states::BomSidebarTab::General,
        });
        self.handle_detach_modal(crate::app::states::ModalId::BomPreview)
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
                if src != dest && src < preview.options.columns.len() && dest < preview.options.columns.len() {
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
        let filter_exts_owned: Vec<String> =
            filter_exts.iter().map(|s| s.to_string()).collect();

        // Stash the live options + dismiss the preview state. The
        // detached preview window also needs to close so the OS
        // window doesn't linger after Export — same shape as
        // `handle_print_preview_export`.
        self.document_state.pending_bom_options = Some(options);
        self.document_state.bom_preview = None;
        let close_window =
            self.close_detached_modal(crate::app::states::ModalId::BomPreview);

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
                    Message::ExportBomFinished(Ok(path))
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
            .remove(&crate::app::states::ModalId::BomPreview);
        self.ui_state.modal_dragging = None;
        self.close_detached_modal(crate::app::states::ModalId::BomPreview)
    }

    /// Build (or rebuild) the BomTable from the current document
    /// state and the supplied options. Returns `None` when there's
    /// no active schematic to roll up.
    fn rebuild_bom_table(&self, opts: &BomOptions) -> Option<signex_output::BomTable> {
        let ctx = build_export_context(&self.document_state)?;
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

        let ctx = match build_export_context(&self.document_state) {
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
                    .active_loaded_project()
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

    pub(crate) fn handle_print_preview_requested(&mut self) -> iced::Task<Message> {
        if !self.document_state.has_active_engine() {
            log::warn!("Print preview: no active schematic");
            return iced::Task::none();
        }

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                log::warn!("Print preview: no active schematic");
                return iced::Task::none();
            }
        };

        // Derive page size and orientation from the active schematic document
        // so the preview matches the actual sheet dimensions rather than
        // always defaulting to A4 landscape. Pull the palette from the
        // active theme so PDF wires / symbols / labels match the canvas.
        let pdf_opts = {
            let paper_str = ctx
                .sheets
                .first()
                .map(|s| s.schematic.paper_size.as_str())
                .unwrap_or("A4");
            let page_size = PageSize::from_paper_name(paper_str);
            let orientation = PageSize::default_orientation_for_paper_name(paper_str);
            let palette = signex_output::SchematicPalette::from(
                &signex_types::theme::canvas_colors(self.ui_state.theme_id),
            );
            PdfOptions {
                page_size,
                orientation,
                palette,
                ..PdfOptions::default()
            }
        };

        // Open-modal default quality is Medium — same value seeded
        // into `PreviewState.quality` below, so the very first
        // rasterisation matches what the picker shows.
        let initial_quality = crate::app::states::PdfQuality::Medium300;
        let pages = PreviewRasterizer.rasterize(
            &ctx,
            &PreviewOptions {
                pdf: pdf_opts.clone(),
                dpi: initial_quality.preview_dpi(),
            },
        );

        if pages.is_empty() {
            log::warn!("Print preview: no pages rendered");
            return iced::Task::none();
        }

        let page_handles: Vec<iced::widget::image::Handle> = pages
            .iter()
            .map(|page| {
                iced::widget::image::Handle::from_rgba(
                    page.width_px,
                    page.height_px,
                    page.rgba.clone(),
                )
            })
            .collect();

        log::info!("Print preview: rendered {} page(s)", pages.len());
        let variants = self
            .document_state
            .active_loaded_project()
            .map(|p| p.data.variant_definitions.clone())
            .unwrap_or_default();
        // Seed `pdf_options.variant` from the project's active variant
        // so the picker reflects the project state on open. The user
        // can override locally without mutating the project.
        let mut pdf_opts = pdf_opts;
        pdf_opts.variant = self
            .document_state
            .active_loaded_project()
            .and_then(|p| p.data.active_variant.clone());
        // Seed the file picker with every sheet from the active
        // project — open-modal default is "export everything", and
        // user toggles take the box from checked to unchecked.
        let selected_files: std::collections::HashSet<PathBuf> = self
            .document_state
            .active_loaded_project()
            .map(|p| {
                let dir = std::path::PathBuf::from(&p.data.dir);
                p.data
                    .sheets
                    .iter()
                    .map(|s| dir.join(&s.filename))
                    .collect()
            })
            .unwrap_or_default();
        self.document_state.preview = Some(crate::app::states::PreviewState {
            pages,
            page_handles,
            selected: 0,
            pdf_options: pdf_opts,
            specific_page_input: "1".to_string(),
            zoom: 1.0,
            active_tab: crate::app::states::PdfPreviewTab::Preview,
            pan: (0.0, 0.0),
            panning: None,
            selected_files,
            variants,
            quality: initial_quality,
        });
        // Altium parity: open Print Preview / Export PDF as its own OS
        // window so the user can drag it off the app's client area —
        // matches the Annotate / ERC modals.
        self.handle_detach_modal(crate::app::states::ModalId::PrintPreview)
    }

    pub(crate) fn handle_print_preview_select_page(&mut self, idx: usize) {
        if let Some(preview) = self.document_state.preview.as_mut()
            && idx < preview.pages.len()
        {
            preview.selected = idx;
        }
    }

    pub(crate) fn handle_print_preview_set_colour_mode(&mut self, mode: signex_output::ColourMode) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.colour_mode = mode;
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_page_range_all(&mut self) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.page_range = PageRange::All;
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_page_range_current(&mut self) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.page_range = PageRange::Current;
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_page_range_specific(&mut self) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.page_range = PageRange::Specific(vec![1]);
            if preview.specific_page_input.trim().is_empty() {
                preview.specific_page_input = "1".to_string();
            }
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_specific_page_input(&mut self, value: String) {
        let parsed_page = value.trim().parse::<usize>().ok();
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.specific_page_input = value;
            if let Some(page) = parsed_page.filter(|p| *p > 0) {
                preview.pdf_options.page_range = PageRange::Specific(vec![page]);
            }
        }
        if parsed_page.map(|p| p > 0).unwrap_or(false) {
            self.rerasterize_print_preview();
        }
    }

    pub(crate) fn handle_print_preview_set_fit_to_page(&mut self, fit: bool) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.scale = if fit {
                signex_output::PdfScale::FitToPage
            } else {
                signex_output::PdfScale::OneToOne
            };
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_include_title_block(&mut self, include: bool) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.include_title_block = include;
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_export(&mut self) -> Option<Task<Message>> {
        // Pull the (possibly edited) options out of the live preview
        // and drive the OS save-file dialog directly. The intermediate
        // settings-only dialog was removed when PDF Export started
        // opening Print Preview as the unified modal.
        let (pdf_options, files) = match self.document_state.preview.as_ref() {
            Some(preview) => {
                let mut options = preview.pdf_options.clone();
                if matches!(options.page_range, PageRange::Specific(_)) {
                    let parsed = preview.specific_page_input.trim().parse::<usize>().ok();
                    let page = match parsed {
                        Some(p) if p > 0 => p,
                        _ => {
                            self.document_state.export_error = Some(
                                "Specific page must be a positive page number (1, 2, 3, ...)."
                                    .to_string(),
                            );
                            return Some(Task::none());
                        }
                    };
                    options.page_range = PageRange::Specific(vec![page]);
                }
                if preview.selected_files.is_empty() {
                    self.document_state.export_error = Some(
                        "Select at least one file in the Settings tab before exporting."
                            .to_string(),
                    );
                    return Some(Task::none());
                }
                // Quality is the only Settings field not stored
                // directly on `pdf_options` — the picker shows a
                // PdfQuality enum, the exporter wants a DPI float.
                // Mapping happens here so the rest of the options
                // struct can stay authoritative.
                options.dpi = preview.quality.export_dpi();
                (options, preview.selected_files.clone())
            }
            None => return Some(Task::none()),
        };

        // Modal stays open across Export — the user might want to
        // tweak settings + re-export to a different filename without
        // re-opening Print Preview from scratch. We stash the options
        // and selected files into pending slots for
        // `handle_export_pdf_finished` to consume; on cancel the
        // pending state is dropped and the modal is unaffected.
        self.document_state.pending_pdf_options = Some(pdf_options);
        self.document_state.pending_pdf_files = Some(files);

        Some(Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Export PDF")
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("schematic.pdf")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| {
                if let Some(path) = path {
                    Message::ExportPdfFinished(Ok(path))
                } else {
                    Message::Noop
                }
            },
        ))
    }

    pub(crate) fn handle_print_preview_close(&mut self) -> iced::Task<Message> {
        self.document_state.preview = None;
        // Reset the modal's drag offset so the next open re-centres
        // — matches Annotate / ERC close behaviour.
        self.ui_state
            .modal_offsets
            .remove(&crate::app::states::ModalId::PrintPreview);
        self.ui_state.modal_dragging = None;
        // Close the detached OS window if the modal was popped out.
        self.close_detached_modal(crate::app::states::ModalId::PrintPreview)
    }

    /// Public alias so the dispatcher can poke a re-rasterise after
    /// mutating `pdf_options` directly. The private function below
    /// was the original entry point; this just lifts visibility.
    pub(crate) fn handle_print_preview_rerender(&mut self) {
        self.rerasterize_print_preview();
    }

    fn rerasterize_print_preview(&mut self) {
        let (pdf_opts, file_filter, preview_dpi) = match self.document_state.preview.as_ref() {
            Some(preview) => (
                preview.pdf_options.clone(),
                preview.selected_files.clone(),
                preview.quality.preview_dpi(),
            ),
            None => return,
        };

        let mut ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => return,
        };
        // Drop sheets the user unchecked in the file picker. An
        // empty filter set means "no files selected" — matches
        // Altium's "you must pick at least one file" UX. We render
        // a still-valid empty preview so the modal stays usable.
        ctx.sheets.retain(|sheet| file_filter.contains(&sheet.path));
        if ctx.sheets.is_empty() {
            // Don't blow up with a "no pages" error in the empty-set
            // case — the user is mid-toggle. Drop the preview pages
            // so the modal shows a placeholder instead.
            if let Some(preview) = self.document_state.preview.as_mut() {
                preview.pages.clear();
                preview.page_handles.clear();
                preview.selected = 0;
                preview.pan = (0.0, 0.0);
                preview.panning = None;
            }
            return;
        }

        let pages = PreviewRasterizer.rasterize(
            &ctx,
            &PreviewOptions {
                pdf: pdf_opts.clone(),
                dpi: preview_dpi,
            },
        );

        if pages.is_empty() {
            self.document_state.export_error = Some(
                "Preview has no pages for the selected range. Check page range input.".to_string(),
            );
            return;
        }

        let page_handles: Vec<iced::widget::image::Handle> = pages
            .iter()
            .map(|page| {
                iced::widget::image::Handle::from_rgba(
                    page.width_px,
                    page.height_px,
                    page.rgba.clone(),
                )
            })
            .collect();

        // Re-rasterise preserves the user's zoom + every settings-tab
        // pick so toggling colour mode / page range doesn't reset
        // their state. Mutate in place rather than reconstruct so the
        // ~30 settings fields keep their values without a stutter
        // through every constructor call site.
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.selected = preview.selected.min(pages.len().saturating_sub(1));
            preview.pages = pages;
            preview.page_handles = page_handles;
            preview.pdf_options = pdf_opts;
            preview.pan = (0.0, 0.0);
            preview.panning = None;
        }
    }

    /// Scroll-wheel zoom on the preview image. `delta_y` follows the
    /// usual sign convention (positive = scroll up = zoom in). The
    /// step is `ZOOM_STEP` per wheel notch, clamped to
    /// `[ZOOM_MIN, ZOOM_MAX]`. Snapping back below 1× resets the pan
    /// since there's nothing to pan over once the image fits the
    /// viewport.
    pub(crate) fn handle_print_preview_zoom(&mut self, delta_y: f32) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            let factor = if delta_y > 0.0 {
                crate::app::states::PreviewState::ZOOM_STEP
            } else if delta_y < 0.0 {
                1.0 / crate::app::states::PreviewState::ZOOM_STEP
            } else {
                return;
            };
            preview.zoom = (preview.zoom * factor).clamp(
                crate::app::states::PreviewState::ZOOM_MIN,
                crate::app::states::PreviewState::ZOOM_MAX,
            );
            if preview.zoom <= 1.0 {
                preview.pan = (0.0, 0.0);
            }
        }
    }

    pub(crate) fn handle_print_preview_set_tab(&mut self, tab: crate::app::states::PdfPreviewTab) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.active_tab = tab;
        }
    }

    /// Press on the preview viewport — arms pan-drag. Subsequent
    /// `last_mouse_pos` updates are converted into pan offsets in
    /// `handle_layout_drag_moved`. Snaps to the current pan as the
    /// origin so the page doesn't jump on press. Reads the cursor
    /// from `interaction_state` rather than from the message so the
    /// press location matches what `last_mouse_pos` says now (iced
    /// builds messages eagerly at view-render time, so coords on
    /// the message would be up to one frame stale).
    pub(crate) fn handle_print_preview_pan_start(&mut self) {
        let (x, y) = self.interaction_state.last_mouse_pos;
        if let Some(preview) = self.document_state.preview.as_mut() {
            // Pan is meaningless at zoom ≤ 1 — the image fits the
            // viewport. Skip arming so the press still falls through
            // as a no-op (the user can still scroll-wheel to zoom).
            if preview.zoom > 1.0 {
                preview.panning = Some((preview.pan, x, y));
            }
        }
    }

    pub(crate) fn handle_print_preview_pan_finished(&mut self) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.panning = None;
        }
    }

    pub(crate) fn handle_print_preview_toggle_file(&mut self, path: std::path::PathBuf) {
        if let Some(preview) = self.document_state.preview.as_mut()
            && !preview.selected_files.remove(&path)
        {
            preview.selected_files.insert(path);
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_select_all_files(&mut self) {
        let all: std::collections::HashSet<PathBuf> = self
            .document_state
            .active_loaded_project()
            .map(|p| {
                let dir = std::path::PathBuf::from(&p.data.dir);
                p.data
                    .sheets
                    .iter()
                    .map(|s| dir.join(&s.filename))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.selected_files = all;
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_clear_all_files(&mut self) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.selected_files.clear();
        }
        self.rerasterize_print_preview();
    }

    pub(crate) fn handle_print_preview_set_variant(&mut self, variant: Option<String>) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.pdf_options.variant = variant;
        }
        // No rerasterize: variant doesn't currently feed into the
        // SVG render pipeline. When it does, move this handler into
        // the visual-toggle group in the dispatcher (call
        // `handle_print_preview_rerender` after mutating).
    }
}

/// Snapshot every open engine as a `SheetSnapshot`, active engine first.
/// Returns `None` if there is no active engine.
fn build_export_context(
    document_state: &crate::app::states::DocumentState,
) -> Option<ExportContext> {
    let active_path = document_state.active_path.as_ref()?;
    let active_engine = document_state.engines.get(active_path)?;

    // Project-wide PDF: walk the active project's full sheet list rather
    // than just the open tabs. Sheets currently opened as tabs use the
    // live engine snapshot (so unsaved edits show in the preview);
    // unopened sheets are read straight from disk via the parser. If
    // the active document isn't tied to a project (loose .snxsch),
    // we fall back to the engines map so a single-sheet preview still
    // works.
    let sheets: Vec<SheetSnapshot> =
        if let Some(project) = document_state.active_loaded_project() {
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
        custom_fields.insert(
            "active_variant".to_string(),
            active_variant,
        );
    }
    let metadata = ProjectMetadata {
        title: tb.get("title").cloned().unwrap_or_default(),
        revision: tb.get("rev").cloned().unwrap_or_default(),
        date: tb.get("date").cloned().unwrap_or_default(),
        company: tb.get("company").cloned().unwrap_or_default(),
        comments: [comment(1), comment(2), comment(3), comment(4)],
        custom_fields,
    };

    Some(ExportContext { sheets, metadata })
}
