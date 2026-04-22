use std::path::PathBuf;

use iced::Task;
use signex_output::{
    ExportContext, Exporter, NetlistExporter, NetlistOptions, PdfExporter, PdfOptions,
    ProjectMetadata, SheetSnapshot, PreviewRasterizer, PreviewOptions,
};

use super::super::super::*;

impl Signex {
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
                log::warn!("PDF export: no active schematic");
                return Task::none();
            }
        };

        match PdfExporter.export(&ctx, &PdfOptions::default()) {
            Ok(output) => match std::fs::write(&save_path, &output.bytes) {
                Ok(()) => log::info!(
                    "Wrote {} ({} page(s), {} bytes)",
                    save_path.display(),
                    output.page_count,
                    output.bytes.len(),
                ),
                Err(e) => log::error!("PDF write failed: {e}"),
            },
            Err(e) => log::error!("PDF export failed: {e}"),
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
                Err(e) => log::error!("Netlist write failed: {e}"),
            },
            Err(e) => log::error!("Netlist export failed: {e}"),
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

        let rasterizer = PreviewRasterizer;
        let opts = PreviewOptions {
            pdf: PdfOptions::default(),
            dpi: 96.0,
        };

        let pages = rasterizer.rasterize(&ctx, &opts);

        if pages.is_empty() {
            log::warn!("Print preview: no pages rendered");
            return;
        }

        // Store preview state in document_state.
        // TODO: Add preview field to DocumentState
        log::info!("Print preview: rendered {} page(s)", pages.len());
    }

    pub(crate) fn handle_print_preview_select_page(&mut self, _idx: usize) {
        // TODO: Update selected page index in preview state
        log::debug!("Print preview: select page {}", _idx);
    }

    pub(crate) fn handle_print_preview_export(&mut self) {
        // Reuse the PDF export flow
        self.handle_export_pdf_requested();
    }

    pub(crate) fn handle_print_preview_close(&mut self) {
        // TODO: Clear preview state from document_state
        log::debug!("Print preview: closed");
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
