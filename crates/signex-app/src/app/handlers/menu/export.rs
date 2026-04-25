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
                log::info!("PDF export cancelled: {e}");
                return Task::none();
            }
        };

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
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
                    .add_filter("KiCad Netlist", &["net"])
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
        self.document_state.bom_preview = Some(crate::app::state::BomPreviewState {
            options: opts,
            table,
            variants,
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

    /// User clicked Export inside the BOM preview modal — stash the
    /// live options on the document, kick off the file dialog, and
    /// finish in `handle_export_bom_finished` (which already exists).
    pub(crate) fn handle_bom_preview_export(&mut self) -> Option<Task<Message>> {
        let Some(_preview) = self.document_state.bom_preview.as_ref() else {
            return Some(Task::none());
        };
        // The file picker's filter list mirrors the format the user
        // picked so the default suffix matches their choice. The
        // existing finish handler resolves the format from the
        // returned file path's extension.
        let (default_name, format_filter) = match self
            .document_state
            .bom_preview
            .as_ref()
            .map(|p| p.options.format)
            .unwrap_or(BomFormat::Csv)
        {
            BomFormat::Csv => ("bom.csv", ("CSV (.csv)", &["csv"][..])),
            BomFormat::Xlsx => ("bom.xlsx", ("Excel (.xlsx)", &["xlsx"][..])),
            BomFormat::Html => ("bom.html", ("HTML (.html)", &["html", "htm"][..])),
        };
        let (filter_label, filter_exts) = format_filter;
        let default_name_owned = default_name.to_string();
        let filter_label_owned = filter_label.to_string();
        let filter_exts_owned: Vec<String> =
            filter_exts.iter().map(|s| s.to_string()).collect();
        Some(Task::perform(
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
        ))
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

    /// Backwards-compat shim: keep the old "no preview" code path
    /// available in case anything still calls it directly. The menu
    /// handler now dispatches through `handle_bom_preview_open`
    /// instead, so this isn't reachable from the UI.
    #[allow(dead_code)]
    pub(crate) fn handle_export_bom_requested(&mut self) -> Option<Task<Message>> {
        Some(self.handle_bom_preview_open())
    }

    pub(crate) fn handle_export_bom_finished(
        &mut self,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("BOM export cancelled: {e}");
                return Task::none();
            }
        };

        let active_variant = self
            .document_state
            .active_loaded_project()
            .and_then(|p| p.data.active_variant.clone())
            .unwrap_or_else(|| "Base".to_string());
        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                self.document_state.export_error =
                    Some("Cannot export BOM: no active schematic.".to_string());
                return Task::none();
            }
        };

        let format = BomFormat::from_output_path(&save_path);

        let opts = BomOptions {
            columns: vec![
                BomColumn::Name,
                BomColumn::Description,
                BomColumn::Designator,
                BomColumn::Footprint,
                BomColumn::LibRef,
                BomColumn::Qty,
            ],
            grouping: BomGrouping::Grouped,
            format,
            include_dnp: false,
            include_not_fitted: false,
            active_variant: if active_variant.eq_ignore_ascii_case("Base") {
                None
            } else {
                Some(active_variant)
            },
            rule_options: Default::default(),
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
        // always defaulting to A4 landscape.
        let pdf_opts = {
            let paper_str = ctx
                .sheets
                .first()
                .map(|s| s.schematic.paper_size.as_str())
                .unwrap_or("A4");
            let page_size = PageSize::from_kicad_str(paper_str);
            let orientation = PageSize::default_orientation_for_kicad(paper_str);
            PdfOptions {
                page_size,
                orientation,
                ..PdfOptions::default()
            }
        };

        let pages = PreviewRasterizer.rasterize(
            &ctx,
            &PreviewOptions {
                pdf: pdf_opts.clone(),
                dpi: 96.0,
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
        self.document_state.preview = Some(crate::app::state::PreviewState {
            pages,
            page_handles,
            selected: 0,
            pdf_options: pdf_opts,
            specific_page_input: "1".to_string(),
        });
        // Altium parity: open Print Preview / Export PDF as its own OS
        // window so the user can drag it off the app's client area —
        // matches the Annotate / ERC modals.
        self.handle_detach_modal(crate::app::state::ModalId::PrintPreview)
    }

    pub(crate) fn handle_print_preview_select_page(&mut self, idx: usize) {
        if let Some(preview) = self.document_state.preview.as_mut() {
            if idx < preview.pages.len() {
                preview.selected = idx;
            }
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
        let pdf_options = match self.document_state.preview.as_ref() {
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
                options
            }
            None => return Some(Task::none()),
        };

        self.document_state.preview = None;
        self.document_state.pending_pdf_options = Some(pdf_options);

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
            .remove(&crate::app::state::ModalId::PrintPreview);
        self.ui_state.modal_dragging = None;
        // Close the detached OS window if the modal was popped out.
        self.close_detached_modal(crate::app::state::ModalId::PrintPreview)
    }

    fn rerasterize_print_preview(&mut self) {
        let (pdf_opts, selected, specific_page_input) = match self.document_state.preview.as_ref() {
            Some(preview) => (
                preview.pdf_options.clone(),
                preview.selected,
                preview.specific_page_input.clone(),
            ),
            None => return,
        };

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => return,
        };

        let pages = PreviewRasterizer.rasterize(
            &ctx,
            &PreviewOptions {
                pdf: pdf_opts.clone(),
                dpi: 96.0,
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

        self.document_state.preview = Some(crate::app::state::PreviewState {
            selected: selected.min(pages.len().saturating_sub(1)),
            pages,
            page_handles,
            pdf_options: pdf_opts,
            specific_page_input,
        });
    }
}

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
    // the active document isn't tied to a project (loose .kicad_sch),
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
                    None => match kicad_parser::parse_schematic_file(&abs_path) {
                        Ok(s) => s,
                        Err(e) => {
                            log::warn!(
                                "Print preview: skipping sheet {} ({}): {e}",
                                entry.name,
                                abs_path.display()
                            );
                            continue;
                        }
                    },
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
