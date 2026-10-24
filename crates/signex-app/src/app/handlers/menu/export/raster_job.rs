//! Background rasterisation of the PDF export preview.
//!
//! `update()` only snapshots the export context and options into a
//! [`PreviewRasterJob`]; the render runs inside a `Task` and comes back as
//! `PrintPreviewMsg::Rasterized`, which is applied only if it is still the
//! newest request (see `PreviewState::apply_raster`).

use std::sync::Arc;

use iced::Task;
use signex_output::{ExportContext, PreviewOptions, PreviewRasterizer};

use super::super::super::super::*;

/// Everything a rasterisation needs, detached from the live app state.
pub(super) struct PreviewRasterJob {
    generation: u64,
    ctx: ExportContext,
    options: PreviewOptions,
}

impl PreviewRasterJob {
    pub(super) fn new(generation: u64, ctx: ExportContext, options: PreviewOptions) -> Self {
        Self {
            generation,
            ctx,
            options,
        }
    }

    /// Render now, on the calling thread.
    pub(super) fn run(self) -> PrintPreviewMsg {
        let pages = PreviewRasterizer.rasterize(&self.ctx, &self.options);
        PrintPreviewMsg::Rasterized {
            generation: self.generation,
            pages: Arc::new(pages),
        }
    }

    /// Hand the render to the runtime; the result arrives as a message.
    pub(super) fn into_task(self) -> Task<Message> {
        Task::perform(async move { self.run() }, Message::PrintPreview)
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
        if preview.apply_raster(generation, pages) == crate::app::state::RasterOutcome::Empty {
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::export(
                "Preview has no pages for the selected range. Check page range input.".to_string(),
            ));
        }
    }
}
