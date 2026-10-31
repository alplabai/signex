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

use signex_output::{
    ExportContext, Margins, Orientation, PageSize, PdfOptions, PdfScale, PreviewOptions,
    PreviewPage, PreviewPageInfo,
};

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

/// One page of the preview: what it is is known up front, its pixels only
/// once it has been rasterised (pages are rasterised lazily, when shown).
#[derive(Debug, Clone)]
pub struct PreviewSlot {
    /// The sheet number the page prints (N in "Sheet N of M").
    pub page_number: usize,
    /// The sheet count the page prints (M in "Sheet N of M").
    pub sheet_count: usize,
    /// Width in pixels at the preview DPI.
    pub width_px: u32,
    /// Height in pixels at the preview DPI.
    pub height_px: u32,
    pub image: SlotImage,
}

/// Whether a [`PreviewSlot`] has pixels yet.
#[derive(Debug, Clone)]
pub enum SlotImage {
    /// Not rasterised and not asked for.
    Pending,
    /// A background render of this page is on its way.
    Requested,
    Ready(iced::widget::image::Handle),
}

impl PreviewSlot {
    fn from_info(info: &PreviewPageInfo) -> Self {
        Self {
            page_number: info.page_number,
            sheet_count: info.sheet_count,
            width_px: info.width_px,
            height_px: info.height_px,
            image: SlotImage::Pending,
        }
    }

    /// The rasterised page, `None` while it is still pending.
    pub fn handle(&self) -> Option<&iced::widget::image::Handle> {
        match &self.image {
            SlotImage::Ready(handle) => Some(handle),
            SlotImage::Pending | SlotImage::Requested => None,
        }
    }
}

/// What the newest rasterisation request renders from, kept so a page the
/// user navigates to later can be rasterised from the same snapshot. The
/// cache of rasterised pages is the preview's slots; it is keyed by
/// `generation`, so any options change (a new generation) replaces it.
#[derive(Debug, Clone)]
pub struct RasterSource {
    pub generation: u64,
    pub ctx: Arc<ExportContext>,
    pub options: PreviewOptions,
    /// The slots of this generation have been laid out.
    applied: bool,
}

impl RasterSource {
    pub fn new(generation: u64, ctx: Arc<ExportContext>, options: PreviewOptions) -> Self {
        Self {
            generation,
            ctx,
            options,
            applied: false,
        }
    }
}

/// A page to rasterise on demand, with everything the render needs.
pub struct PageRequest {
    pub generation: u64,
    pub ctx: Arc<ExportContext>,
    pub options: PreviewOptions,
    pub position: usize,
}

impl PreviewState {
    /// Record `generation` (minted by the [`RasterQueue`]) as the newest
    /// request, invalidating every earlier one.
    pub fn begin_raster(&mut self, generation: u64) {
        self.raster_generation = generation;
    }

    /// Whether `generation` is still the newest request.
    pub fn is_current_raster(&self, generation: u64) -> bool {
        generation == self.raster_generation
    }

    /// Apply the layout and first pages of the rasterisation tagged
    /// `generation`, unless a newer one has begun since. The slots replace
    /// the whole page cache, so pages of older options never survive.
    pub fn apply_raster(
        &mut self,
        generation: u64,
        outline: &[PreviewPageInfo],
        pages: Vec<(usize, PreviewPage)>,
    ) -> RasterOutcome {
        if generation != self.raster_generation {
            return RasterOutcome::Stale;
        }
        if outline.is_empty() || pages.is_empty() {
            return RasterOutcome::Empty;
        }
        self.pages = outline.iter().map(PreviewSlot::from_info).collect();
        self.fill_pages(pages);
        self.selected = self.selected.min(self.pages.len() - 1);
        self.pan = (0.0, 0.0);
        self.panning = None;
        if let Some(source) = self.raster_source.as_mut()
            && source.generation == generation
        {
            source.applied = true;
        }
        RasterOutcome::Applied
    }

    /// Fill in lazily rasterised pages. Dropped (`false`) unless `generation`
    /// is still the newest request and its layout is the one on screen.
    pub fn apply_pages(&mut self, generation: u64, pages: Vec<(usize, PreviewPage)>) -> bool {
        if !self.is_current_raster(generation) || !self.layout_is_current() {
            return false;
        }
        self.fill_pages(pages);
        true
    }

    fn fill_pages(&mut self, pages: Vec<(usize, PreviewPage)>) {
        for (position, page) in pages {
            if let Some(slot) = self.pages.get_mut(position) {
                slot.image = SlotImage::Ready(iced::widget::image::Handle::from_rgba(
                    page.width_px,
                    page.height_px,
                    page.rgba,
                ));
            }
        }
    }

    /// Whether the slots on screen were laid out by the newest request.
    fn layout_is_current(&self) -> bool {
        self.raster_source
            .as_ref()
            .is_some_and(|source| source.applied && source.generation == self.raster_generation)
    }

    /// The selected page, if it still needs rasterising: marks it requested
    /// so one navigation asks for it once.
    pub fn take_selected_page_request(&mut self) -> Option<PageRequest> {
        if !self.layout_is_current() {
            return None;
        }
        let position = self.selected;
        let slot = self.pages.get_mut(position)?;
        if !matches!(slot.image, SlotImage::Pending) {
            return None;
        }
        slot.image = SlotImage::Requested;
        let source = self.raster_source.as_ref()?;
        Some(PageRequest {
            generation: source.generation,
            ctx: Arc::clone(&source.ctx),
            options: source.options.clone(),
            position,
        })
    }

    /// A page render failed: let the user ask for those pages again.
    pub fn forget_requested_pages(&mut self) {
        for slot in &mut self.pages {
            if matches!(slot.image, SlotImage::Requested) {
                slot.image = SlotImage::Pending;
            }
        }
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
    in_flight: Vec<iced::task::Handle>,
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

    /// The newest generation and its shared cell, for another job of the
    /// same request (a page rasterised on demand) that must not cancel the
    /// jobs already running for it.
    pub fn current(&self) -> (u64, Arc<AtomicU64>) {
        (self.last_minted, Arc::clone(&self.latest))
    }

    /// Remember a running job's handle so the next request can abort it.
    pub fn track(&mut self, handle: iced::task::Handle) {
        self.in_flight.push(handle);
    }

    /// Abort the running job and invalidate its result (preview closed).
    pub fn supersede_all(&mut self) {
        self.begin();
    }

    fn cancel_in_flight(&mut self) {
        for handle in self.in_flight.drain(..) {
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

/// Drop a closed document's remembered setup, and detach the open preview
/// from it so dismissing that preview cannot file the setup back.
pub fn forget_page_setup(
    memory: &mut PageSetupMemory,
    preview: Option<&mut PreviewState>,
    path: &Path,
) {
    memory.remove(path);
    if let Some(preview) = preview
        && preview.source_path.as_deref() == Some(path)
    {
        preview.source_path = None;
    }
}

#[cfg(test)]
impl PreviewState {
    /// A preview with no pages and no raster requested yet.
    pub(crate) fn blank_for_tests() -> Self {
        Self {
            source: PathBuf::new(),
            pages: Vec::new(),
            raster_source: None,
            selected: 0,
            pdf_options: PdfOptions::default(),
            page_setup: crate::app::state::PageSetupInputs::from_options(&PdfOptions::default()),
            specific_page_input: "1".to_string(),
            range_note: None,
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
            export_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(n: usize) -> PreviewPage {
        PreviewPage {
            page_number: n,
            sheet_count: n,
            width_px: 1,
            height_px: 1,
            rgba: vec![0, 0, 0, 255],
        }
    }

    fn info(n: usize) -> PreviewPageInfo {
        PreviewPageInfo {
            page_number: n,
            sheet_count: n,
            width_px: 1,
            height_px: 1,
        }
    }

    fn outline(count: usize) -> Vec<PreviewPageInfo> {
        (1..=count).map(info).collect()
    }

    fn blank() -> PreviewState {
        PreviewState::blank_for_tests()
    }

    /// A preview whose `generation` laid out `count` pages and drew position 0.
    fn drawn_preview(generation: u64, count: usize) -> PreviewState {
        let mut state = blank();
        state.begin_raster(generation);
        state.raster_source = Some(RasterSource::new(
            generation,
            Arc::new(sample_ctx()),
            PreviewOptions::default(),
        ));
        state.apply_raster(generation, &outline(count), vec![(0, page(1))]);
        state
    }

    fn sample_ctx() -> ExportContext {
        ExportContext {
            sheets: Vec::new(),
            metadata: Default::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
            netlist_placements: Vec::new(),
            current_sheet: None,
        }
    }

    fn ready(state: &PreviewState) -> Vec<usize> {
        (0..state.pages.len())
            .filter(|&i| state.pages[i].handle().is_some())
            .collect()
    }

    #[test]
    fn a_result_from_a_superseded_request_is_discarded() {
        let mut state = blank();
        let (old, new) = (1, 2);
        state.begin_raster(old);
        state.begin_raster(new);

        assert_eq!(
            state.apply_raster(old, &outline(1), vec![(0, page(1))]),
            RasterOutcome::Stale
        );
        assert!(state.pages.is_empty(), "stale pages must not be shown");

        assert_eq!(
            state.apply_raster(new, &outline(2), vec![(0, page(1))]),
            RasterOutcome::Applied
        );
        assert_eq!(state.pages.len(), 2);
    }

    #[test]
    fn a_late_old_result_cannot_replace_a_newer_preview() {
        let mut state = blank();
        let (old, new) = (1, 2);
        state.begin_raster(old);
        state.begin_raster(new);
        assert_eq!(
            state.apply_raster(new, &[info(7)], vec![(0, page(7))]),
            RasterOutcome::Applied
        );
        assert_eq!(
            state.apply_raster(old, &[info(1)], vec![(0, page(1))]),
            RasterOutcome::Stale
        );
        assert_eq!(state.pages[0].page_number, 7);
    }

    #[test]
    fn only_the_newest_request_is_current() {
        let mut state = blank();
        state.begin_raster(1);
        state.begin_raster(2);
        assert!(!state.is_current_raster(1));
        assert!(state.is_current_raster(2));
    }

    #[test]
    fn an_empty_result_keeps_the_previous_pages() {
        let mut state = blank();
        let first = 1;
        state.begin_raster(first);
        state.apply_raster(first, &outline(1), vec![(0, page(1))]);
        let second = 2;
        state.begin_raster(second);
        assert_eq!(
            state.apply_raster(second, &[], Vec::new()),
            RasterOutcome::Empty
        );
        assert_eq!(state.pages.len(), 1);
    }

    #[test]
    fn selection_is_clamped_to_the_new_page_count() {
        let mut state = blank();
        let first = 1;
        state.begin_raster(first);
        state.apply_raster(first, &outline(3), vec![(0, page(1))]);
        state.selected = 2;
        let second = 2;
        state.begin_raster(second);
        state.apply_raster(second, &outline(1), vec![(0, page(1))]);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn a_five_page_layout_draws_only_the_page_it_was_given() {
        let state = drawn_preview(1, 5);
        assert_eq!(state.pages.len(), 5);
        assert_eq!(ready(&state), [0]);
    }

    #[test]
    fn navigating_asks_for_the_new_page_once() {
        let mut state = drawn_preview(1, 5);
        assert!(
            state.take_selected_page_request().is_none(),
            "page 0 is drawn"
        );

        state.selected = 1;
        let request = state
            .take_selected_page_request()
            .expect("page 1 is missing");
        assert_eq!((request.generation, request.position), (1, 1));
        assert!(
            state.take_selected_page_request().is_none(),
            "an in-flight page must not be asked for twice"
        );

        assert!(state.apply_pages(1, vec![(1, page(2))]));
        assert_eq!(ready(&state), [0, 1]);
        assert!(
            state.take_selected_page_request().is_none(),
            "page 1 is cached"
        );

        // Back to a drawn page: free.
        state.selected = 0;
        assert!(state.take_selected_page_request().is_none());
    }

    #[test]
    fn a_new_request_drops_every_drawn_page() {
        let mut state = drawn_preview(1, 5);
        state.selected = 1;
        state.take_selected_page_request();
        state.apply_pages(1, vec![(1, page(2))]);
        assert_eq!(ready(&state), [0, 1]);

        // Options changed: generation 2 begins, then lays out and draws one page.
        state.begin_raster(2);
        state.raster_source = Some(RasterSource::new(
            2,
            Arc::new(sample_ctx()),
            PreviewOptions::default(),
        ));
        assert!(
            state.take_selected_page_request().is_none(),
            "no page requests against a layout that is being replaced"
        );
        state.apply_raster(2, &outline(5), vec![(1, page(2))]);
        assert_eq!(ready(&state), [1], "only the page the new render drew");
    }

    #[test]
    fn a_page_result_for_a_superseded_layout_is_dropped() {
        let mut state = drawn_preview(1, 5);
        state.selected = 3;
        state.take_selected_page_request();
        state.begin_raster(2);
        assert!(!state.apply_pages(1, vec![(3, page(4))]));
        assert_eq!(ready(&state), [0]);
    }

    #[test]
    fn a_failed_page_can_be_asked_for_again() {
        let mut state = drawn_preview(1, 5);
        state.selected = 2;
        assert!(state.take_selected_page_request().is_some());
        state.forget_requested_pages();
        assert!(state.take_selected_page_request().is_some());
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
    fn forgetting_drops_only_that_document_and_detaches_its_preview() {
        let gone = PathBuf::from("gone.snxsch");
        let kept = PathBuf::from("kept.snxsch");
        let mut memory = PageSetupMemory::new();
        for path in [&gone, &kept] {
            memory.insert(
                path.clone(),
                RememberedPageSetup::capture(&PdfOptions::default()),
            );
        }
        let mut preview = blank();
        preview.source_path = Some(gone.clone());
        forget_page_setup(&mut memory, Some(&mut preview), &gone);
        assert!(!memory.contains_key(&gone) && memory.contains_key(&kept));
        assert_eq!(preview.source_path, None);

        // A preview open for another document keeps its source.
        let mut other = blank();
        other.source_path = Some(kept.clone());
        forget_page_setup(&mut memory, Some(&mut other), &gone);
        assert_eq!(other.source_path, Some(kept));
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
