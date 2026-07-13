//! Export handlers — PDF export dialog + netlist export. Split from `menu/export.rs`.

use std::path::PathBuf;

use iced::Task;
use signex_output::{Exporter, NetlistExporter, NetlistOptions, PdfExporter};

use super::super::super::super::*;

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

        let mut ctx = match super::build_export_context(&self.document_state) {
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
                self.document_state.export_error =
                    Some("Cannot export PDF: no files selected in the Settings tab.".to_string());
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
                    .add_filter("Standard Netlist", &["net"])
                    .set_file_name("schematic.net")
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            |path| {
                if let Some(path) = path {
                    Message::Export(ExportMsg::NetlistFinished(Ok(path)))
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

        let ctx = match super::build_export_context(&self.document_state) {
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
}
