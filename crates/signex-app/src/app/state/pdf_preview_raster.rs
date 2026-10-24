//! Generation-guarded application of background preview rasterisations, and
//! the per-document page setup the modal restores when it reopens.
//!
//! Every re-rasterisation request takes a new generation number; its result
//! is applied only if no newer request was made meanwhile, so a slow render of
//! superseded options can never overwrite a newer preview.

use std::path::PathBuf;

use signex_output::{Margins, Orientation, PageSize, PdfOptions, PdfScale, PreviewPage};

use super::PreviewState;

/// What became of a finished background rasterisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterOutcome {
    /// Options changed (or the modal closed) since the render began.
    Stale,
    /// The render produced no pages; the previous pages were kept.
    Empty,
    /// The pages replaced the displayed preview.
    Applied,
}

impl PreviewState {
    /// Start a new rasterisation: invalidates every earlier one and returns
    /// the generation the new result must carry back.
    pub fn begin_raster(&mut self) -> u64 {
        self.raster_generation = self.raster_generation.wrapping_add(1);
        self.raster_generation
    }

    /// Apply the pages of the rasterisation tagged `generation`, unless a
    /// newer one has begun since.
    pub fn apply_raster(&mut self, generation: u64, pages: Vec<PreviewPage>) -> RasterOutcome {
        if generation != self.raster_generation {
            return RasterOutcome::Stale;
        }
        if pages.is_empty() {
            return RasterOutcome::Empty;
        }
        self.page_handles = pages
            .iter()
            .map(|page| {
                iced::widget::image::Handle::from_rgba(
                    page.width_px,
                    page.height_px,
                    page.rgba.clone(),
                )
            })
            .collect();
        self.selected = self.selected.min(pages.len() - 1);
        self.pages = pages;
        self.pan = (0.0, 0.0);
        self.panning = None;
        RasterOutcome::Applied
    }
}

/// The page-setup half of [`PdfOptions`] a document keeps for the session.
#[derive(Debug, Clone)]
pub struct RememberedPageSetup {
    page_size: PageSize,
    orientation: Orientation,
    margins: Margins,
    scale: PdfScale,
}

impl RememberedPageSetup {
    pub fn capture(options: &PdfOptions) -> Self {
        Self {
            page_size: options.page_size,
            orientation: options.orientation,
            margins: options.margins,
            scale: options.scale,
        }
    }

    pub fn restore_into(&self, options: &mut PdfOptions) {
        options.page_size = self.page_size;
        options.orientation = self.orientation;
        options.margins = self.margins;
        options.scale = self.scale;
    }
}

/// Session memory of each document's page setup, keyed by the document path.
pub type PageSetupMemory = std::collections::HashMap<PathBuf, RememberedPageSetup>;

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: usize) -> PreviewPage {
        PreviewPage {
            page_number: n,
            width_px: 1,
            height_px: 1,
            rgba: vec![0, 0, 0, 255],
        }
    }

    fn blank() -> PreviewState {
        PreviewState {
            pages: Vec::new(),
            page_handles: Vec::new(),
            selected: 0,
            pdf_options: PdfOptions::default(),
            page_setup: crate::app::state::PageSetupInputs::from_options(&PdfOptions::default()),
            specific_page_input: "1".to_string(),
            zoom: 1.0,
            active_tab: crate::app::state::PdfPreviewTab::Preview,
            pan: (0.0, 0.0),
            panning: None,
            sheet_files: Vec::new(),
            selected_files: std::collections::HashSet::new(),
            variants: Vec::new(),
            quality: crate::app::state::PdfQuality::Draft72,
            raster_generation: 0,
        }
    }

    #[test]
    fn a_result_from_a_superseded_request_is_discarded() {
        let mut state = blank();
        let old = state.begin_raster();
        let new = state.begin_raster();
        assert_ne!(old, new);

        assert_eq!(state.apply_raster(old, vec![page(1)]), RasterOutcome::Stale);
        assert!(state.pages.is_empty(), "stale pages must not be shown");

        assert_eq!(
            state.apply_raster(new, vec![page(1), page(2)]),
            RasterOutcome::Applied
        );
        assert_eq!(state.pages.len(), 2);
        assert_eq!(state.page_handles.len(), 2);
    }

    #[test]
    fn a_late_old_result_cannot_replace_a_newer_preview() {
        let mut state = blank();
        let old = state.begin_raster();
        let new = state.begin_raster();
        assert_eq!(
            state.apply_raster(new, vec![page(7)]),
            RasterOutcome::Applied
        );
        assert_eq!(state.apply_raster(old, vec![page(1)]), RasterOutcome::Stale);
        assert_eq!(state.pages[0].page_number, 7);
    }

    #[test]
    fn an_empty_result_keeps_the_previous_pages() {
        let mut state = blank();
        let first = state.begin_raster();
        state.apply_raster(first, vec![page(1)]);
        let second = state.begin_raster();
        assert_eq!(state.apply_raster(second, Vec::new()), RasterOutcome::Empty);
        assert_eq!(state.pages.len(), 1);
    }

    #[test]
    fn selection_is_clamped_to_the_new_page_count() {
        let mut state = blank();
        let first = state.begin_raster();
        state.apply_raster(first, vec![page(1), page(2), page(3)]);
        state.selected = 2;
        let second = state.begin_raster();
        state.apply_raster(second, vec![page(1)]);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn remembered_setup_round_trips_the_page_setup_fields() {
        let custom = PdfOptions {
            page_size: PageSize::IsoA3,
            orientation: Orientation::Portrait,
            scale: PdfScale::Percent(50.0),
            ..PdfOptions::default()
        };
        let remembered = RememberedPageSetup::capture(&custom);

        let mut fresh = PdfOptions::default();
        remembered.restore_into(&mut fresh);
        assert_eq!(fresh.page_size, PageSize::IsoA3);
        assert_eq!(fresh.orientation, Orientation::Portrait);
        assert_eq!(fresh.scale, PdfScale::Percent(50.0));
    }
}
