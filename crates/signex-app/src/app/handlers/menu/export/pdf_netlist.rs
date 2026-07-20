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

        let (mut ctx, issues) = match super::build_export_scope(&self.document_state) {
            Some(c) => c,
            None => {
                self.document_state.pending_pdf_options = None;
                self.document_state.pending_pdf_files = None;
                self.document_state.export_error =
                    Some("Cannot export PDF: no active schematic.".to_string());
                return Task::none();
            }
        };
        // Human-consumed deliverable: a partial PDF beats a refusal — the
        // user may well be printing or reviewing mid-refactor with a child
        // file genuinely absent from disk. So this degrades rather than
        // blocks, but it degrades *loudly*, once, from this user action.
        super::log_stitch_issues(&self.document_state, &ctx, &issues);

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

        let (ctx, issues) = match super::build_export_scope(&self.document_state) {
            Some(c) => c,
            None => {
                log::warn!("Netlist export: no active schematic");
                return Task::none();
            }
        };
        super::log_stitch_issues(&self.document_state, &ctx, &issues);

        // Machine-consumed deliverable: this file is imported into a PCB, and
        // a hole in it becomes missing components on the board plus nets that
        // stayed split where they should have merged through the missing
        // sheet's ports. There is no "read the warning and judge" step
        // downstream, so refuse-by-default and write nothing rather than hand
        // the layout tool a plausible-looking wrong netlist. (The PDF, which a
        // human reads, takes the opposite call.)
        //
        // #431: instead of a dead-end error, raise a two-choice prompt — the
        // refusal stays the default (nothing written until the user acts), but
        // "Export anyway (incomplete)" writes the partial `.net` WITH the
        // omission recorded in its header comment. See
        // `handle_netlist_export_anyway`.
        if issues.netlist_is_incomplete() {
            // Snapshot the export scope alongside the omitted-page messages, so
            // "Export anyway" writes from exactly this derivation. Bytes and the
            // INCOMPLETE header then always describe the same project state, even
            // if the document changes while the prompt is up (#431 review).
            self.document_state.netlist_incomplete_prompt =
                Some(crate::app::state::NetlistIncompletePrompt {
                    save_path,
                    messages: issues.messages(),
                    ctx,
                });
            return Task::none();
        }

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

    /// #431 — "Export anyway (incomplete)". The user explicitly chose to ship
    /// the partial netlist. Re-derives the export scope, writes the
    /// best-available (root-reachable) netlist to the pending path WITH an
    /// INCOMPLETE header comment listing the omitted pages, then clears the
    /// prompt. The header is non-negotiable: a partial `.net` NEVER reaches
    /// disk without the incompleteness recorded in the file itself, so a
    /// downstream PCB import can see it is partial.
    pub(crate) fn handle_netlist_export_anyway(&mut self) -> Task<Message> {
        let Some(prompt) = self.document_state.netlist_incomplete_prompt.take() else {
            return Task::none();
        };
        let save_path = prompt.save_path;

        // Write from the scope SNAPSHOTTED when the prompt was raised, never a
        // fresh re-derivation (#431 review, CRITICAL): the header names the pages
        // omitted at prompt time, so the bytes must come from that same
        // derivation. Re-deriving here would let a mid-modal document edit leave
        // the on-disk INCOMPLETE header describing a different project state than
        // the file it is attached to — the very fab hazard #431 exists to
        // prevent, just relocated from "no header" to "wrong header".
        let options = NetlistOptions {
            incomplete_note: Some(prompt.messages),
            ..NetlistOptions::default()
        };
        match NetlistExporter.export(&prompt.ctx, &options) {
            Ok(output) => match std::fs::write(&save_path, &output.bytes) {
                Ok(()) => log::info!(
                    "Wrote INCOMPLETE netlist {} ({} bytes) — user chose Export anyway",
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

    /// #431 — "Cancel" on the netlist-incomplete prompt (or click-outside).
    /// Writes nothing; just clears the pending prompt.
    pub(crate) fn handle_netlist_cancel_incomplete(&mut self) {
        self.document_state.netlist_incomplete_prompt = None;
    }
}
