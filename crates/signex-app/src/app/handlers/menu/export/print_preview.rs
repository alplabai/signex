//! Print-preview modal handlers. Split from `menu/export.rs`.

use std::path::PathBuf;

use iced::Task;
use signex_output::{PageRange, PageSize, PdfOptions, PreviewOptions, PreviewRasterizer};

use super::super::super::super::*;

impl Signex {
    pub(crate) fn handle_print_preview_requested(&mut self) -> iced::Task<Message> {
        if !self.document_state.has_active_engine() {
            log::warn!("Print preview: no active schematic");
            return iced::Task::none();
        }

        let ctx = match super::build_export_context(&self.document_state) {
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
            let page_size = PageSize::from_standard_str(paper_str);
            let orientation = PageSize::default_orientation_for_standard(paper_str);
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
        let initial_quality = crate::app::state::PdfQuality::Medium300;
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
            .export_scope_project()
            .map(|p| p.data.variant_definitions.clone())
            .unwrap_or_default();
        // Seed `pdf_options.variant` from the project's active variant
        // so the picker reflects the project state on open. The user
        // can override locally without mutating the project.
        let mut pdf_opts = pdf_opts;
        pdf_opts.variant = self
            .document_state
            .export_scope_project()
            .and_then(|p| p.data.active_variant.clone());
        // Seed the file picker off the export context's own sheet set —
        // open-modal default is "export everything", and user toggles
        // take the box from checked to unchecked. Reading `ctx` rather
        // than re-deriving from the project keeps the picker and the
        // exported pages in lockstep for loose documents too.
        let sheet_files: Vec<(PathBuf, String)> = ctx
            .sheets
            .iter()
            .map(|s| (s.path.clone(), s.sheet_name.clone()))
            .collect();
        let selected_files: std::collections::HashSet<PathBuf> =
            sheet_files.iter().map(|(p, _)| p.clone()).collect();
        self.document_state.preview = Some(crate::app::state::PreviewState {
            pages,
            page_handles,
            selected: 0,
            pdf_options: pdf_opts,
            specific_page_input: "1".to_string(),
            zoom: 1.0,
            active_tab: crate::app::state::PdfPreviewTab::Preview,
            pan: (0.0, 0.0),
            panning: None,
            sheet_files,
            selected_files,
            variants,
            quality: initial_quality,
        });
        // Altium parity: open Print Preview / Export PDF as its own OS
        // window so the user can drag it off the app's client area —
        // matches the Annotate / ERC modals.
        self.handle_detach_modal(crate::app::state::ModalId::PrintPreview)
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
                    Message::Export(ExportMsg::PdfFinished(Ok(path)))
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

        let mut ctx = match super::build_export_context(&self.document_state) {
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
                crate::app::state::PreviewState::ZOOM_STEP
            } else if delta_y < 0.0 {
                1.0 / crate::app::state::PreviewState::ZOOM_STEP
            } else {
                return;
            };
            preview.zoom = (preview.zoom * factor).clamp(
                crate::app::state::PreviewState::ZOOM_MIN,
                crate::app::state::PreviewState::ZOOM_MAX,
            );
            if preview.zoom <= 1.0 {
                preview.pan = (0.0, 0.0);
            }
        }
    }

    pub(crate) fn handle_print_preview_set_tab(&mut self, tab: crate::app::state::PdfPreviewTab) {
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
        if let Some(preview) = self.document_state.preview.as_mut() {
            preview.selected_files = preview.sheet_files.iter().map(|(p, _)| p.clone()).collect();
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
