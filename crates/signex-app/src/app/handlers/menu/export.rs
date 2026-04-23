use std::path::PathBuf;

use iced::Task;
use signex_output::{
    BomColumn, BomExporter, BomFormat, BomGrouping, BomOptions, ExportContext, Exporter,
    NetlistExporter, NetlistOptions, PageRange, PageSize, PdfExporter, PdfOptions, PreviewOptions,
    PreviewRasterizer, ProjectMetadata, SheetSnapshot,
};

use super::super::super::*;

impl Signex {
    pub(crate) fn handle_export_pdf_open_dialog(&mut self) {
        if !self.document_state.has_active_engine() {
            log::warn!("PDF export: no active schematic");
            return;
        }

        // Prefer a seed copied from Print Preview controls. Otherwise pre-populate
        // from the active document's paper declaration.
        let options = if let Some(seed) = self.document_state.pdf_options_seed.take() {
            seed
        } else if let Some(active_path) = self.document_state.active_path.as_ref() {
            if let Some(engine) = self.document_state.engines.get(active_path) {
                let paper_str = engine.document().paper_size.as_str();
                let page_size = PageSize::from_standard_str(paper_str);
                let orientation = PageSize::default_orientation_for_standard(paper_str);
                PdfOptions {
                    page_size,
                    orientation,
                    ..PdfOptions::default()
                }
            } else {
                PdfOptions::default()
            }
        } else {
            PdfOptions::default()
        };

        let specific_page_input = match &options.page_range {
            PageRange::Specific(pages) if !pages.is_empty() => pages[0].to_string(),
            _ => "1".to_string(),
        };

        self.document_state.pdf_options_dialog = Some(crate::app::state::PdfOptionsDialogState {
            options,
            specific_page_input,
        });
    }

    pub(crate) fn handle_export_pdf_set_page_size(&mut self, page_size: signex_output::PageSize) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.page_size = page_size;
        }
    }

    pub(crate) fn handle_export_pdf_set_orientation(
        &mut self,
        orientation: signex_output::Orientation,
    ) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.orientation = orientation;
        }
    }

    pub(crate) fn handle_export_pdf_set_colour_mode(&mut self, colour_mode: signex_output::ColourMode) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.colour_mode = colour_mode;
        }
    }

    pub(crate) fn handle_export_pdf_set_template(
        &mut self,
        template_id: Option<signex_output::TemplateId>,
    ) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.sheet_template = template_id;
        }
    }

    pub(crate) fn handle_export_pdf_set_fit_to_page(&mut self, fit_to_page: bool) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.scale = if fit_to_page {
                signex_output::PdfScale::FitToPage
            } else {
                signex_output::PdfScale::OneToOne
            };
        }
    }

    pub(crate) fn handle_export_pdf_set_include_title_block(&mut self, include: bool) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.include_title_block = include;
        }
    }

    pub(crate) fn handle_export_pdf_set_page_range_all(&mut self) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.page_range = PageRange::All;
        }
    }

    pub(crate) fn handle_export_pdf_set_page_range_current(&mut self) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.page_range = PageRange::Current;
        }
    }

    pub(crate) fn handle_export_pdf_set_page_range_specific(&mut self) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.options.page_range = PageRange::Specific(vec![1]);
            if dialog.specific_page_input.trim().is_empty() {
                dialog.specific_page_input = "1".to_string();
            }
        }
    }

    pub(crate) fn handle_export_pdf_set_specific_page_input(&mut self, value: String) {
        if let Some(dialog) = self.document_state.pdf_options_dialog.as_mut() {
            dialog.specific_page_input = value;
        }
    }

    pub(crate) fn handle_export_pdf_dialog_cancel(&mut self) {
        self.document_state.pdf_options_dialog = None;
    }

    pub(crate) fn handle_export_pdf_dialog_confirm(&mut self) -> Option<Task<Message>> {
        if !self.document_state.has_active_engine() {
            log::warn!("PDF export: no active schematic");
            return Some(Task::none());
        }

        let dialog = match self.document_state.pdf_options_dialog.as_ref() {
            Some(d) => d,
            None => return Some(Task::none()),
        };

        let mut options = dialog.options.clone();
        if matches!(options.page_range, PageRange::Specific(_)) {
            let page = dialog.specific_page_input.trim().parse::<usize>().ok();
            let page = match page {
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

        self.document_state.pdf_options_dialog = None;

        // Stash options in the document state so handle_export_pdf_finished can access them.
        // We'll use a pending_pdf_options field (add next).
        self.document_state.pending_pdf_options = Some(options);

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

    #[allow(dead_code)]
    pub(crate) fn handle_export_pdf_requested(&mut self) -> Option<Task<Message>> {
        if !self.document_state.has_active_engine() {
            log::warn!("PDF export: no active schematic");
            return Some(Task::none());
        }

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
                self.document_state.export_error =
                    Some(format!("PDF export failed: {e}"));
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
                    .add_filter("Standard Netlist", &["net"])
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
                self.document_state.export_error =
                    Some(format!("Netlist export failed: {e}"));
            }
        }

        Task::none()
    }

    pub(crate) fn handle_dismiss_export_error(&mut self) {
        self.document_state.export_error = None;
    }

    pub(crate) fn handle_export_bom_requested(&mut self) -> Option<Task<Message>> {
        if !self.document_state.has_active_engine() {
            log::warn!("BOM export: no active schematic");
            return Some(Task::none());
        }

        Some(Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Export Bill of Materials")
                    .add_filter("CSV (.csv)", &["csv"])
                    .add_filter("Excel (.xlsx)", &["xlsx"])
                    .add_filter("HTML (.html)", &["html", "htm"])
                    .set_file_name("bom.csv")
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

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                self.document_state.export_error =
                    Some("Cannot export BOM: no active schematic.".to_string());
                return Task::none();
            }
        };

        let format = match save_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("xlsx") => BomFormat::Xlsx,
            Some("html") | Some("htm") => BomFormat::Html,
            _ => BomFormat::Csv,
        };

        let opts = BomOptions {
            columns: vec![
                BomColumn::Reference,
                BomColumn::Qty,
                BomColumn::Value,
                BomColumn::Footprint,
                BomColumn::Description,
            ],
            grouping: BomGrouping::Grouped,
            format,
            include_dnp: false,
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

    pub(crate) fn handle_print_preview_requested(&mut self) {
        if !self.document_state.has_active_engine() {
            log::warn!("Print preview: no active schematic");
            return;
        }

        let ctx = match build_export_context(&self.document_state) {
            Some(c) => c,
            None => {
                log::warn!("Print preview: no active schematic");
                return;
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
            let page_size = PageSize::from_standard_str(paper_str);
            let orientation = PageSize::default_orientation_for_standard(paper_str);
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

        log::info!("Print preview: rendered {} page(s)", pages.len());
        self.document_state.preview = Some(crate::app::state::PreviewState {
            pages,
            page_handles,
            selected: 0,
            pdf_options: pdf_opts,
            specific_page_input: "1".to_string(),
        });
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

    pub(crate) fn handle_print_preview_export(&mut self) -> Option<Task<Message>> {
        if let Some(preview) = self.document_state.preview.as_ref() {
            let mut seed = preview.pdf_options.clone();
            if matches!(seed.page_range, PageRange::Specific(_)) {
                if let Ok(page) = preview.specific_page_input.trim().parse::<usize>() {
                    if page > 0 {
                        seed.page_range = PageRange::Specific(vec![page]);
                    }
                }
            }
            self.document_state.pdf_options_seed = Some(seed);
        }
        // Close the preview overlay and open the PDF options dialog.
        self.document_state.preview = None;
        Some(self.update(Message::ExportPdfOpenDialog))
    }

    pub(crate) fn handle_print_preview_close(&mut self) {
        self.document_state.preview = None;
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
                "Preview has no pages for the selected range. Check page range input."
                    .to_string(),
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

    let mut paths: Vec<PathBuf> = document_state.engines.keys().cloned().collect();
    paths.sort_by_key(|p| p != active_path);

    let sheet_count = paths.len();
    let sheets = paths
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
        .collect::<Vec<_>>();

    let tb = &active_engine.document().title_block;
    let comment = |n: usize| tb.get(&format!("comment{n}")).cloned().unwrap_or_default();
    let metadata = ProjectMetadata {
        title: tb.get("title").cloned().unwrap_or_default(),
        revision: tb.get("rev").cloned().unwrap_or_default(),
        date: tb.get("date").cloned().unwrap_or_default(),
        company: tb.get("company").cloned().unwrap_or_default(),
        comments: [comment(1), comment(2), comment(3), comment(4)],
        custom_fields: Default::default(),
    };

    Some(ExportContext { sheets, metadata })
}
