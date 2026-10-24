//! Background rasterisation of the PDF export preview.
//!
//! `update()` only snapshots the export context and options into a
//! [`PreviewRasterJob`]; the render runs on a blocking thread inside a `Task`
//! and comes back as `PrintPreviewMsg::Rasterized`, which is applied only if
//! it is still the newest request (see `PreviewState::apply_raster`). A job
//! superseded before it starts skips the render, and the previous task is
//! aborted when a new one launches, so fast typing cannot pile up renders.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use iced::Task;
use signex_output::{ExportContext, PreviewOptions, PreviewRasterizer};

use super::super::super::super::*;

/// Everything a rasterisation needs, detached from the live app state.
pub(super) struct PreviewRasterJob {
    generation: u64,
    latest: Arc<AtomicU64>,
    ctx: ExportContext,
    options: PreviewOptions,
}

impl PreviewRasterJob {
    pub(super) fn new(
        generation: u64,
        latest: Arc<AtomicU64>,
        ctx: ExportContext,
        options: PreviewOptions,
    ) -> Self {
        Self {
            generation,
            latest,
            ctx,
            options,
        }
    }

    /// Render now, on the calling thread.
    pub(super) fn run(self) -> PrintPreviewMsg {
        // Queued behind a newer request: nobody will apply this result.
        let pages = if self.latest.load(Ordering::SeqCst) == self.generation {
            PreviewRasterizer.rasterize(&self.ctx, &self.options)
        } else {
            Vec::new()
        };
        PrintPreviewMsg::Rasterized {
            generation: self.generation,
            pages: Arc::new(pages),
        }
    }

    /// Hand the render to the blocking pool; the result arrives as a message.
    /// `None` result from a failed worker becomes an empty (stale-checked)
    /// render rather than a panic.
    pub(super) fn into_task(self) -> Task<Message> {
        let generation = self.generation;
        Task::perform(
            async move {
                let worker = tokio::task::spawn_blocking(move || self.run());
                // Aborting this task must also drop a render still queued.
                let _abort_on_drop = AbortOnDrop(worker.abort_handle());
                worker.await.unwrap_or(PrintPreviewMsg::Rasterized {
                    generation,
                    pages: Arc::new(Vec::new()),
                })
            },
            Message::PrintPreview,
        )
    }
}

/// Aborts the blocking job (if it has not started) when the awaiting task is
/// dropped, e.g. because a newer render superseded it.
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Signex {
    /// Run `job` as an abortable task and remember its handle so the next
    /// request (or the preview closing) cancels it.
    pub(super) fn launch_raster(&mut self, job: PreviewRasterJob) -> Task<Message> {
        let (task, handle) = job.into_task().abortable();
        self.document_state.raster_queue.track(handle);
        task
    }
}

impl Signex {
    /// Apply a finished background rasterisation. A result superseded by a
    /// newer request, or arriving after the modal closed, is dropped.
    pub(crate) fn handle_print_preview_rasterized(
        &mut self,
        generation: u64,
        pages: Arc<Vec<signex_output::PreviewPage>>,
    ) {
        let Some(preview) = self.document_state.preview.as_mut() else {
            return;
        };
        let pages = Arc::try_unwrap(pages).unwrap_or_else(|shared| (*shared).clone());
        let outcome = preview.apply_raster(generation, pages);
        if outcome == crate::app::state::RasterOutcome::Applied {
            log::info!("Print preview: rendered {} page(s)", preview.pages.len());
        }
        if outcome == crate::app::state::RasterOutcome::Empty {
            log::warn!("Print preview: no pages rendered");
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                "Preview has no pages for the selected range. Check page range input.".to_string(),
            ));
        }
    }
}
