//! Generation-guarded application of background preview rasterisations, and
//! the per-document page setup the modal restores when it reopens.
//!
//! Every re-rasterisation request takes a new generation number from the
//! document-wide [`RasterQueue`]; its result is applied only if no newer
//! request was made meanwhile, so a slow render of superseded options can
//! never overwrite a newer preview. The counter outlives any one preview
//! session, so a late result from a closed preview can never match the
//! generation of a preview opened afterwards.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Record `generation` (minted by the [`RasterQueue`]) as the newest
    /// request, invalidating every earlier one.
    pub fn begin_raster(&mut self, generation: u64) {
        self.raster_generation = generation;
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

/// Document-wide bookkeeping of background preview renders. It lives outside
/// any one `PreviewState` so generations stay unique across preview sessions,
/// and it is what lets a newer request cancel or coalesce the older ones.
#[derive(Debug, Default)]
pub struct RasterQueue {
    last_minted: u64,
    /// Newest generation, shared with queued blocking jobs so one that has
    /// not started yet can see it was superseded and skip the render.
    latest: Arc<AtomicU64>,
    in_flight: Option<iced::task::Handle>,
}

impl RasterQueue {
    /// Mint the next generation, aborting the previous job's task. Returns the
    /// generation and the shared "newest generation" cell for the new job.
    pub fn begin(&mut self) -> (u64, Arc<AtomicU64>) {
        self.cancel_in_flight();
        self.last_minted = self.last_minted.wrapping_add(1);
        self.latest.store(self.last_minted, Ordering::SeqCst);
        (self.last_minted, Arc::clone(&self.latest))
    }

    /// Remember the running job's handle so the next request can abort it.
    pub fn track(&mut self, handle: iced::task::Handle) {
        self.in_flight = Some(handle);
    }

    /// Abort the running job and invalidate its result (preview closed).
    pub fn supersede_all(&mut self) {
        self.begin();
    }

    fn cancel_in_flight(&mut self) {
        if let Some(handle) = self.in_flight.take() {
            handle.abort();
        }
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

/// Move a remembered setup (and the open preview's source) from `old` to
/// `new` when the document is renamed or saved under another name.
pub fn rekey_page_setup(
    memory: &mut PageSetupMemory,
    preview: Option<&mut PreviewState>,
    old: &Path,
    new: &Path,
) {
    if let Some(remembered) = memory.remove(old) {
        memory.insert(new.to_path_buf(), remembered);
    }
    if let Some(preview) = preview
        && preview.source_path.as_deref() == Some(old)
    {
        preview.source_path = Some(new.to_path_buf());
    }
}

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
            source_path: None,
        }
    }

    #[test]
    fn a_result_from_a_superseded_request_is_discarded() {
        let mut state = blank();
        let (old, new) = (1, 2);
        state.begin_raster(old);
        state.begin_raster(new);

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
        let (old, new) = (1, 2);
        state.begin_raster(old);
        state.begin_raster(new);
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
        let first = 1;
        state.begin_raster(first);
        state.apply_raster(first, vec![page(1)]);
        let second = 2;
        state.begin_raster(second);
        assert_eq!(state.apply_raster(second, Vec::new()), RasterOutcome::Empty);
        assert_eq!(state.pages.len(), 1);
    }

    #[test]
    fn selection_is_clamped_to_the_new_page_count() {
        let mut state = blank();
        let first = 1;
        state.begin_raster(first);
        state.apply_raster(first, vec![page(1), page(2), page(3)]);
        state.selected = 2;
        let second = 2;
        state.begin_raster(second);
        state.apply_raster(second, vec![page(1)]);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn generations_never_repeat_and_the_latest_is_published() {
        let mut queue = RasterQueue::default();
        let (first, latest) = queue.begin();
        let (second, _) = queue.begin();
        assert_ne!(first, second);
        assert_eq!(latest.load(Ordering::SeqCst), second);
        queue.supersede_all();
        assert!(latest.load(Ordering::SeqCst) > second);
    }

    #[test]
    fn rekeying_moves_the_memory_and_the_open_preview_source() {
        let (old, new) = (PathBuf::from("a.snxsch"), PathBuf::from("b.snxsch"));
        let mut memory = PageSetupMemory::new();
        memory.insert(
            old.clone(),
            RememberedPageSetup::capture(&PdfOptions::default()),
        );
        let mut preview = blank();
        preview.source_path = Some(old.clone());
        rekey_page_setup(&mut memory, Some(&mut preview), &old, &new);
        assert!(memory.contains_key(&new) && !memory.contains_key(&old));
        assert_eq!(preview.source_path, Some(new));
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
