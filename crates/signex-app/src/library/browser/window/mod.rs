//! Row windowing for the browser grid (#1355).
//!
//! A library table can hold tens of thousands of rows, but only a screenful
//! is ever visible. Every grid row has the same fixed height, so the rows in
//! view follow from the scroll offset and viewport height alone; the grid
//! builds widgets for those rows (plus a little overscan) and stands in for
//! the rest with two spacers of the exact skipped height, leaving the
//! scrollbar and the total content height unchanged.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::Path;

/// Height of every data row in the grid. Rows are pinned to it so the
/// window maths does not depend on what a cell happens to contain. One text
/// line plus the 4 px a text cell pads above and below, the tallest cell.
pub(super) const BROWSER_ROW_HEIGHT: f32 =
    super::BROWSER_TEXT_SIZE * crate::styles::LINE_HEIGHT + 2.0 * 4.0;
/// Height of every data row in thumbnail rows (mockup rows 118 and 119):
/// the row's tile with the text cell's padding above and below.
pub(super) const BROWSER_THUMB_ROW_HEIGHT: f32 = crate::library::panel::THUMB_TILE_H + 2.0 * 4.0;

/// Height of every data row under the shared row style.
pub fn row_height(style: crate::library::state::LibraryRowStyle) -> f32 {
    match style {
        crate::library::state::LibraryRowStyle::Compact => BROWSER_ROW_HEIGHT,
        crate::library::state::LibraryRowStyle::Thumbnail => BROWSER_THUMB_ROW_HEIGHT,
    }
}

/// Height of the header row, which scrolls with the rows and so offsets
/// them from the top of the scrollable content.
pub(super) const BROWSER_HEADER_HEIGHT: f32 = 28.0;
/// Rows built beyond each edge of the viewport, so a fast wheel flick shows
/// content before the next scroll message arrives.
pub(crate) const OVERSCAN_ROWS: usize = 12;
/// Id of a library's grid scrollable, the target of a scroll-to-row task.
/// Per library, so a scroll aimed at one tab never moves another's.
pub fn grid_scroll_id(library_path: &Path) -> iced::widget::Id {
    iced::widget::Id::from(format!("library-browser-grid:{}", library_path.display()))
}

/// Identity of a library's grid within the widget tree.
pub(super) fn library_key(library_path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    library_path.hash(&mut hasher);
    hasher.finish()
}

/// Scroll position of the grid, as last reported by its scrollable.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GridScroll {
    /// Distance the content is scrolled from its top, in pixels.
    pub offset_y: f32,
    /// Height the grid is laid out at, from layout rather than the
    /// scrollable; `0.0` until the first layout.
    pub viewport_h: f32,
}

/// The rows to build, as indices into the `count` visible rows.
///
/// `scroll_y` is the offset into the row area (header already removed). It
/// is clamped to what `count` rows can actually scroll, so an offset left
/// over from before the list shrank still yields a populated window.
pub(crate) fn visible_range(
    scroll_y: f32,
    viewport_h: f32,
    row_h: f32,
    count: usize,
    overscan: usize,
) -> Range<usize> {
    if count == 0 || row_h.is_nan() || row_h <= 0.0 {
        return 0..0;
    }
    let viewport_h = viewport_h.max(0.0);
    let max_scroll = (count as f32 * row_h - viewport_h).max(0.0);
    let scroll_y = if scroll_y.is_finite() {
        scroll_y.clamp(0.0, max_scroll)
    } else {
        0.0
    };
    let first = (scroll_y / row_h).floor() as usize;
    let last = ((scroll_y + viewport_h) / row_h).ceil() as usize;
    let start = first.saturating_sub(overscan).min(count);
    let end = last.saturating_add(overscan).min(count);
    start..end.max(start)
}

/// Scroll offset (from the top of the scrollable content) that brings row
/// `index` fully into view, or `None` when it already is. `viewport_h` of
/// `0.0` (unmeasured) never scrolls.
fn offset_to_reveal(index: usize, scroll: GridScroll, row_h: f32, leading_h: f32) -> Option<f32> {
    if scroll.viewport_h <= 0.0 {
        return None;
    }
    let top = leading_h + index as f32 * row_h;
    let bottom = top + row_h;
    if top < scroll.offset_y {
        Some(top)
    } else if bottom > scroll.offset_y + scroll.viewport_h {
        Some(bottom - scroll.viewport_h)
    } else {
        None
    }
}

/// Scroll offset that brings the grid's `index`-th row into view, or `None`
/// when it already is.
pub fn reveal_row_offset(index: usize, scroll: GridScroll) -> Option<f32> {
    reveal_row_offset_at(index, scroll, BROWSER_ROW_HEIGHT)
}

/// [`reveal_row_offset`] for rows `row_h` tall (see [`row_height`]).
pub fn reveal_row_offset_at(index: usize, scroll: GridScroll, row_h: f32) -> Option<f32> {
    offset_to_reveal(index, scroll, row_h, BROWSER_HEADER_HEIGHT)
}

impl crate::library::state::LibraryState {
    /// Record which library's browser tab is showing. A browser that comes
    /// back to the front starts at the top: its scrollable is rebuilt
    /// there, and a stale offset would draw one blank frame first.
    pub(crate) fn note_shown_browser(&mut self, shown: Option<&Path>) {
        if self.shown_browser.as_deref() == shown {
            return;
        }
        if let Some(browser) = shown.and_then(|p| self.library_browsers.get_mut(p)) {
            browser.grid_scroll.offset_y = 0.0;
        }
        self.shown_browser = shown.map(Path::to_path_buf);
    }

    /// The symbols of the rows the shown browser builds in thumbnail rows,
    /// for the shared thumbnail read; empty in compact rows.
    pub(crate) fn browser_wanted_thumbnails(&self) -> Vec<signex_library::PrimitiveRef> {
        if self.library_panel.row_style != crate::library::state::LibraryRowStyle::Thumbnail {
            return Vec::new();
        }
        let Some(path) = self.shown_browser.as_deref() else {
            return Vec::new();
        };
        let (Some(lib), Some(browser)) = (self.library_at(path), self.library_browsers.get(path))
        else {
            return Vec::new();
        };
        let Some(cache) = browser.view_cache.as_ref() else {
            return Vec::new();
        };
        let Some(rows) = lib
            .tables
            .get(super::view_cache::active_table(lib, browser))
        else {
            return Vec::new();
        };
        let window = visible_rows_at(
            browser.grid_scroll,
            cache.visible.len(),
            BROWSER_THUMB_ROW_HEIGHT,
        );
        cache.visible[window]
            .iter()
            .filter_map(|&i| rows.get(i))
            .map(|row| row.symbol_ref)
            .collect()
    }
}

/// The rows the grid builds for `count` rows at `scroll`.
pub fn visible_rows(scroll: GridScroll, count: usize) -> Range<usize> {
    visible_rows_at(scroll, count, BROWSER_ROW_HEIGHT)
}

/// [`visible_rows`] for rows `row_h` tall (see [`row_height`]).
pub fn visible_rows_at(scroll: GridScroll, count: usize, row_h: f32) -> Range<usize> {
    visible_range(
        scroll.offset_y - BROWSER_HEADER_HEIGHT,
        scroll.viewport_h,
        row_h,
        count,
        OVERSCAN_ROWS,
    )
}

/// Rows built by the grid, so a test can tell "windowed" from "all".
#[cfg(test)]
pub(crate) mod counters {
    use std::cell::Cell;

    thread_local! {
        pub static ROWS_BUILT: Cell<usize> = const { Cell::new(0) };
    }
}

#[cfg(test)]
mod tests;
