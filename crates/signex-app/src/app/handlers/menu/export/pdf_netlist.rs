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

    /// `request` carries the source document, options and sheet picks
    /// captured at the Export click; the scope is built from it, never from
    /// whichever tab is active now (#833). A source closed in the meantime is
    /// refused. A dismissed save dialog arrives as `Err("cancelled")`.
    pub(crate) fn handle_export_pdf_finished(
        &mut self,
        request: Box<crate::app::state::PdfExportRequest>,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let crate::app::state::PdfExportRequest {
            source,
            mut options,
            files,
            descended_placement,
        } = *request;
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("PDF export not written: {e}");
                return Task::none();
            }
        };

        let (mut ctx, issues) =
            match super::build_page_scope_for_source(&self.document_state, &source) {
                Some(c) => c,
                None => {
                    let reason = format!(
                        "PDF export refused: the source document {} is no longer open.",
                        source.display()
                    );
                    crate::diagnostics::log_warning(reason.clone());
                    self.document_state.error_notice =
                        Some(crate::app::state::ErrorNotice::export(reason));
                    return Task::none();
                }
            };
        // Class styles are not something the user picked in the modal: they
        // follow the netlist this export just derived, so a sheet that is
        // not open in a tab is styled like one that is.
        options.net_class_styles =
            super::class_styles::export_class_styles(&self.document_state, &source, &ctx);
        // Human-consumed deliverable: a partial PDF beats a refusal — the
        // user may well be printing or reviewing mid-refactor with a child
        // file genuinely absent from disk. So this degrades rather than
        // blocks, but it degrades *loudly*, once, from this user action.
        super::log_stitch_issues(&source, &ctx, &issues);

        self.retire_dead_placement(&issues, &source, &options.page_range);
        // Filter sheets to the user's file picks from the Settings tab.
        if !super::isolate_undeclared_current(
            &mut ctx,
            &issues,
            &self.document_state,
            &source,
            descended_placement.as_ref(),
            &options.page_range,
        ) {
            ctx.sheets
                .retain(|s| files.contains(&(s.path.clone(), s.placement.clone())));
        }
        if ctx.sheets.is_empty() {
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                "Cannot export PDF: no files selected in the Settings tab.".to_string(),
            ));
            return Task::none();
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

        match PdfExporter
            .export(&ctx, &options)
            .inspect(|output| super::log_uncovered_chars(&output.uncovered_chars))
        {
            Ok(output) => {
                let success_log = format!(
                    "Wrote {} ({} page(s), {} bytes)",
                    save_path.display(),
                    output.page_count,
                    output.bytes.len(),
                );
                return super::write::write_export_task(
                    save_path,
                    output.bytes,
                    "PDF",
                    success_log,
                    None,
                    Vec::new(),
                );
            }
            Err(e) => {
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                    format!("PDF export failed: {e}"),
                ));
            }
        }

        Task::none()
    }

    pub(crate) fn handle_export_netlist_requested(&mut self) -> Option<Task<Message>> {
        if !self.document_state.has_active_engine() {
            log::warn!("Netlist export: no active schematic");
            return Some(Task::none());
        }

        // The save dialog outlives any tab switch; pin the export to the
        // document it was started from (#852).
        let source = self.document_state.active_path.clone()?;
        let format = NetlistOptions::default().format;
        Some(Task::perform(
            async move {
                rfd::AsyncFileDialog::new()
                    .set_title("Export Netlist")
                    .add_filter(format.label(), &[format.extension()])
                    .set_file_name(format.default_file_name())
                    .save_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |path| {
                if let Some(path) = path {
                    Message::Export(ExportMsg::NetlistFinished {
                        source,
                        result: Ok(path),
                    })
                } else {
                    Message::Noop
                }
            },
        ))
    }

    pub(crate) fn handle_export_netlist_finished(
        &mut self,
        source: PathBuf,
        result: Result<PathBuf, String>,
    ) -> Task<Message> {
        let save_path = match result {
            Ok(p) => p,
            Err(e) => {
                log::info!("Netlist export cancelled: {e}");
                return Task::none();
            }
        };

        let (ctx, issues) =
            match super::build_export_scope_for_source(&self.document_state, &source) {
                Some(c) => c,
                None => {
                    crate::diagnostics::log_warning(format!(
                        "Netlist export refused: the source document {} is no longer open.",
                        source.display()
                    ));
                    return Task::none();
                }
            };
        super::log_stitch_issues(&source, &ctx, &issues);

        // Not a hole the user can accept: the file would be wrong, not partial.
        if let Some(refusal) = issues.shared_reference_refusal(&ctx) {
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::export(refusal));
            return Task::none();
        }

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
        // "Export anyway (incomplete)" writes the partial `.net` and reports
        // the omission to the Messages panel (Protel has no comment syntax, so
        // the file itself stays clean). See
        // `handle_netlist_export_anyway`.
        if issues.netlist_is_incomplete() {
            // Snapshot the export scope alongside the omitted-page messages, so
            // "Export anyway" writes from exactly this derivation. The bytes and
            // the omitted-page warning then always describe the same project
            // state, even if the document changes while the prompt is up (#431
            // review).
            self.document_state.netlist_incomplete_prompt =
                Some(crate::app::state::NetlistIncompletePrompt {
                    save_path,
                    messages: issues.messages(),
                    ctx,
                });
            return Task::none();
        }

        match NetlistExporter.export(&ctx, &NetlistOptions::default()) {
            Ok(output) => {
                let success_log = format!(
                    "Wrote {} ({} bytes)",
                    save_path.display(),
                    output.bytes.len(),
                );
                return super::write::write_export_task(
                    save_path,
                    output.bytes,
                    "netlist",
                    success_log,
                    None,
                    Vec::new(),
                );
            }
            Err(e) => {
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                    format!("Netlist export failed: {e}"),
                ));
            }
        }

        Task::none()
    }

    /// #431 — "Export anyway (incomplete)". The user explicitly chose to ship
    /// the partial netlist. Re-derives the export scope, writes the
    /// best-available (root-reachable) netlist to the pending path, then clears
    /// the prompt. Protel has no comment syntax, so the file is a clean,
    /// valid netlist; the omission is reported as a warning (the omitted pages,
    /// one per line) in the Messages panel and the log, so a partial `.net`
    /// never reaches disk without the user having been told.
    pub(crate) fn handle_netlist_export_anyway(&mut self) -> Task<Message> {
        let Some(prompt) = self.document_state.netlist_incomplete_prompt.take() else {
            return Task::none();
        };
        let save_path = prompt.save_path;

        // Write from the scope SNAPSHOTTED when the prompt was raised, never a
        // fresh re-derivation (#431 review, CRITICAL): the warning names the
        // pages omitted at prompt time, so the bytes must come from that same
        // derivation. Re-deriving here would let a mid-modal document edit make
        // the warning describe a different project state than the file written.
        match NetlistExporter.export(&prompt.ctx, &NetlistOptions::default()) {
            Ok(output) => {
                let success_log = format!(
                    "Wrote INCOMPLETE netlist {} ({} bytes) — user chose Export anyway",
                    save_path.display(),
                    output.bytes.len(),
                );
                // One short line each: the Messages panel truncates long
                // entries, which would cut the page name off.
                let mut warnings = vec![
                    "Export: wrote an INCOMPLETE netlist (Export anyway) — not a wiring \
                     reference. Omitted:"
                        .to_owned(),
                ];
                warnings.extend(
                    prompt
                        .messages
                        .iter()
                        .map(|message| format!("Export: INCOMPLETE — {message}")),
                );
                return super::write::write_export_task(
                    save_path,
                    output.bytes,
                    "netlist",
                    success_log,
                    None,
                    warnings,
                );
            }
            Err(e) => {
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                    format!("Netlist export failed: {e}"),
                ));
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
