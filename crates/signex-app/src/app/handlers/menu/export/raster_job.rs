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
    pub(super) ctx: ExportContext,
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
    /// A worker that panicked comes back as `RasterFailed`; one that was
    /// cancelled (superseded) as an empty, stale-checked render.
    pub(super) fn into_task(self) -> Task<Message> {
        let generation = self.generation;
        Task::perform(
            async move {
                let worker = tokio::task::spawn_blocking(move || self.run());
                // Aborting this task must also drop a render still queued.
                let _abort_on_drop = AbortOnDrop(worker.abort_handle());
                match worker.await {
                    Ok(msg) => msg,
                    Err(error) => worker_failure(generation, error),
                }
            },
            Message::PrintPreview,
        )
    }
}

fn worker_failure(generation: u64, error: tokio::task::JoinError) -> PrintPreviewMsg {
    if error.is_cancelled() {
        return PrintPreviewMsg::Rasterized {
            generation,
            pages: Arc::new(Vec::new()),
        };
    }
    PrintPreviewMsg::RasterFailed {
        generation,
        reason: panic_reason(error),
    }
}

/// The panic message, when the payload is the usual `&str` / `String`.
fn panic_reason(error: tokio::task::JoinError) -> String {
    if !error.is_panic() {
        return error.to_string();
    }
    let payload = error.into_panic();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the render panicked".to_string())
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
            // Only a real empty range reaches here; a crashed render is
            // `RasterFailed` (#1092).
            log::warn!("Print preview: no pages rendered");
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                "Preview has no pages for the selected range. Check page range input.".to_string(),
            ));
        }
    }
}

impl Signex {
    /// A background rasterisation crashed. The newest one is reported in
    /// the Messages panel and the error card; the previous pages stay up.
    pub(crate) fn handle_print_preview_raster_failed(&mut self, generation: u64, reason: &str) {
        let Some(preview) = self.document_state.preview.as_ref() else {
            return;
        };
        if !preview.is_current_raster(generation) {
            log::debug!("Print preview: dropped a superseded render failure: {reason}");
            return;
        }
        crate::diagnostics::log_error(
            "Print preview render failed",
            &anyhow::anyhow!(reason.to_string()),
        );
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(format!(
            "The preview could not be rendered: {reason}"
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join_error_of(f: impl FnOnce() + Send + 'static) -> tokio::task::JoinError {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        rt.block_on(async { tokio::task::spawn_blocking(f).await.unwrap_err() })
    }

    #[test]
    fn a_panicking_render_is_a_failure_not_an_empty_range() {
        let error = join_error_of(|| panic!("glyph cache poisoned"));
        match worker_failure(7, error) {
            PrintPreviewMsg::RasterFailed { generation, reason } => {
                assert_eq!(generation, 7);
                assert_eq!(reason, "glyph cache poisoned");
            }
            other => panic!("expected RasterFailed, got {other:?}"),
        }
    }

    #[test]
    fn a_formatted_panic_message_is_kept() {
        let page = 3;
        let error = join_error_of(move || panic!("page {page} has no sheet"));
        let PrintPreviewMsg::RasterFailed { reason, .. } = worker_failure(1, error) else {
            panic!("expected RasterFailed");
        };
        assert_eq!(reason, "page 3 has no sheet");
    }

    fn app_with_preview_at(generation: u64) -> crate::app::Signex {
        let (mut app, _boot) = crate::app::Signex::new();
        let mut preview = crate::app::state::PreviewState::blank_for_tests();
        preview.begin_raster(generation);
        app.document_state.preview = Some(preview);
        app
    }

    #[test]
    fn the_newest_render_failure_shows_the_card_and_reaches_messages() {
        let reason = "raster-1092-newest-unique-reason";
        let before = crate::diagnostics::logged_count(reason);
        let mut app = app_with_preview_at(5);

        app.handle_print_preview_raster_failed(5, reason);

        let notice = app
            .document_state
            .error_notice
            .as_ref()
            .expect("error card");
        assert!(
            notice.detail.contains(reason),
            "card names the reason: {}",
            notice.detail
        );
        assert_eq!(
            crate::diagnostics::logged_count(reason),
            before + 1,
            "one Messages entry"
        );
    }

    #[test]
    fn a_superseded_render_failure_is_dropped_silently() {
        let reason = "raster-1092-superseded-unique-reason";
        let before = crate::diagnostics::logged_count(reason);
        let mut app = app_with_preview_at(6);

        app.handle_print_preview_raster_failed(5, reason);

        assert!(
            app.document_state.error_notice.is_none(),
            "no card for a stale render"
        );
        assert_eq!(
            crate::diagnostics::logged_count(reason),
            before,
            "nothing reaches Messages"
        );
    }
}
