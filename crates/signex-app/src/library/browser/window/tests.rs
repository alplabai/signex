//! The grid builds only the rows near the viewport (#1355).

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PlmReserved, PrimitiveRef, RowId,
};
use signex_types::theme::{ThemeId, theme_tokens};
use uuid::Uuid;

use super::counters::ROWS_BUILT;
use super::*;
use crate::library::state::OpenLibrary;
use crate::library::state::{LibraryBrowserState, LibraryDisplaySettings, LibraryState};

const H: f32 = BROWSER_ROW_HEIGHT;

fn scrolled(offset_y: f32, viewport_h: f32) -> GridScroll {
    GridScroll {
        offset_y,
        viewport_h,
    }
}

// ── visible_range ────────────────────────────────────────────────

#[test]
fn at_the_top_the_window_starts_at_row_zero() {
    let r = visible_range(0.0, 10.0 * H, H, 10_000, 3);
    assert_eq!(r, 0..13);
}

#[test]
fn mid_list_the_window_is_the_viewport_plus_overscan_each_side() {
    // Row 100 sits exactly at the top edge; the viewport shows 100..110.
    let r = visible_range(100.0 * H, 10.0 * H, H, 10_000, 3);
    assert_eq!(r, 97..113);
}

#[test]
fn a_partly_visible_row_at_each_edge_is_included() {
    // Top edge cuts row 100, bottom edge cuts row 110.
    let r = visible_range(100.5 * H, 10.0 * H, H, 10_000, 0);
    assert_eq!(r, 100..111);
}

#[test]
fn at_the_end_the_window_stops_at_the_last_row() {
    let count = 10_000;
    let end = count as f32 * H - 10.0 * H;
    let r = visible_range(end, 10.0 * H, H, count, 3);
    assert_eq!(r, 9_987..10_000);
}

#[test]
fn an_offset_past_the_end_is_clamped_to_the_last_page() {
    let r = visible_range(1.0e9, 10.0 * H, H, 10_000, 0);
    assert_eq!(r, 9_990..10_000);
}

#[test]
fn after_the_list_shrinks_a_stale_offset_still_shows_rows() {
    // Scrolled deep into a long list, then a filter leaves 5 rows.
    let r = visible_range(200_000.0, 10.0 * H, H, 5, 3);
    assert_eq!(r, 0..5);
}

#[test]
fn a_negative_or_non_finite_offset_means_the_top() {
    assert_eq!(visible_range(-50.0, 5.0 * H, H, 100, 0), 0..5);
    assert_eq!(visible_range(f32::NAN, 5.0 * H, H, 100, 0), 0..5);
    assert_eq!(visible_range(f32::INFINITY, 5.0 * H, H, 100, 0), 0..5);
}

#[test]
fn a_zero_viewport_builds_only_the_overscan() {
    assert_eq!(visible_range(0.0, 0.0, H, 1_000, 4), 0..4);
    let r = visible_range(50.0 * H, 0.0, H, 1_000, 4);
    assert_eq!(r, 46..54);
    assert_eq!(visible_range(0.0, -5.0, H, 1_000, 0), 0..0);
}

#[test]
fn an_empty_list_or_a_degenerate_row_height_builds_nothing() {
    assert_eq!(visible_range(0.0, 500.0, H, 0, 5), 0..0);
    assert_eq!(visible_range(0.0, 500.0, 0.0, 100, 5), 0..0);
    assert_eq!(visible_range(0.0, 500.0, f32::NAN, 100, 5), 0..0);
}

#[test]
fn a_short_list_is_built_whole() {
    assert_eq!(visible_range(0.0, 1000.0, H, 3, 12), 0..3);
}

#[test]
fn spacers_plus_built_rows_always_add_up_to_the_full_height() {
    let count = 10_000usize;
    for step in 0..400 {
        let scroll = step as f32 * 613.7;
        let r = visible_range(scroll, 700.0, H, count, OVERSCAN_ROWS);
        let total = r.start as f32 * H + r.len() as f32 * H + (count - r.end) as f32 * H;
        assert_eq!(total, count as f32 * H, "scroll {scroll}");
    }
}

#[test]
fn the_window_is_bounded_and_covers_every_row_in_view() {
    let count = 50_000usize;
    let viewport = 700.0;
    let bound = (viewport / H).ceil() as usize + 1 + 2 * OVERSCAN_ROWS;
    for step in 0..500 {
        let scroll = step as f32 * 2_391.3;
        let r = visible_range(scroll, viewport, H, count, OVERSCAN_ROWS);
        assert!(r.len() <= bound, "scroll {scroll}: {} rows", r.len());
        let max_scroll = count as f32 * H - viewport;
        let y = scroll.min(max_scroll);
        let first_in_view = (y / H) as usize;
        let last_in_view = (((y + viewport) / H).ceil() as usize).min(count) - 1;
        assert!(
            r.start <= first_in_view && last_in_view < r.end,
            "scroll {scroll}"
        );
    }
}

// ── offset_to_reveal ─────────────────────────────────────────────

#[test]
fn a_row_already_in_view_needs_no_scroll() {
    let s = scrolled(0.0, 500.0);
    assert_eq!(reveal_row_offset(2, s), None);
}

#[test]
fn a_row_below_the_viewport_scrolls_it_to_the_bottom_edge() {
    let s = scrolled(0.0, 500.0);
    let off = reveal_row_offset(9_000, s).expect("scrolls");
    let after = scrolled(off, 500.0);
    assert_eq!(reveal_row_offset(9_000, after), None);
    let bottom = BROWSER_HEADER_HEIGHT + 9_001.0 * H;
    assert_eq!(off + 500.0, bottom);
}

#[test]
fn a_row_above_the_viewport_scrolls_it_to_the_top_edge() {
    let s = scrolled(100_000.0, 500.0);
    let off = reveal_row_offset(10, s).expect("scrolls");
    assert_eq!(off, BROWSER_HEADER_HEIGHT + 10.0 * H);
}

#[test]
fn an_unmeasured_viewport_never_scrolls() {
    assert_eq!(reveal_row_offset(9_000, GridScroll::default()), None);
}

// ── the built grid ───────────────────────────────────────────────

const TABLE: &str = "Resistors";

fn row(pn: &str) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "MPN"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

fn library_of(n: usize) -> (LibraryState, PathBuf, Vec<RowId>) {
    library_at(n, "/tmp/grid-window/Window.snxlib")
}

fn library_at(n: usize, path: &str) -> (LibraryState, PathBuf, Vec<RowId>) {
    let root = PathBuf::from(path);
    let rows: Vec<ComponentRow> = (0..n).map(|i| row(&format!("R{i:06}"))).collect();
    let ids = rows.iter().map(|r| RowId::from_uuid(r.row_id)).collect();
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: root.clone(),
        display_name: "Window".into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::from([(TABLE.to_string(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    state.touch_data();
    let mut browser = LibraryBrowserState::new(root.clone());
    browser.active_table = Some(TABLE.to_string());
    state.library_browsers.insert(root.clone(), browser);
    state.sync_view_caches();
    (state, root, ids)
}

/// Draw the grid once and return how many row widgets it built.
fn rows_built_by_view(state: &LibraryState, root: &PathBuf) -> usize {
    let tokens = theme_tokens(ThemeId::Signex);
    ROWS_BUILT.with(|c| c.set(0));
    drop(crate::library::browser::view(
        root,
        state,
        &state.library_browsers[root],
        &tokens,
    ));
    ROWS_BUILT.with(|c| c.get())
}

fn bound(viewport: f32) -> usize {
    (viewport / H).ceil() as usize + 1 + 2 * OVERSCAN_ROWS
}

#[test]
fn ten_thousand_rows_build_only_the_window() {
    let (mut state, root, _) = library_of(10_000);
    state.library_browsers.get_mut(&root).unwrap().grid_scroll = scrolled(0.0, 700.0);

    let built = rows_built_by_view(&state, &root);

    assert!(built > 0);
    assert!(built <= bound(700.0), "built {built} of 10000");
}

#[test]
fn before_the_first_layout_the_grid_builds_only_the_overscan() {
    let (state, root, _) = library_of(10_000);

    let built = rows_built_by_view(&state, &root);

    assert!(built > 0);
    assert!(built <= OVERSCAN_ROWS, "built {built}");
}

#[test]
fn a_table_that_fits_after_the_grid_grows_is_built_whole() {
    // 50 rows fill a 700 px grid with a scrollbar. Maximised to 1300 px they
    // fit, so the scrollable publishes nothing; only the new height (from
    // layout) reaches the state.
    let (mut state, root, _) = library_of(50);
    state.library_browsers.get_mut(&root).unwrap().grid_scroll = scrolled(0.0, 700.0);
    assert!(rows_built_by_view(&state, &root) < 50);

    state
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .grid_scroll
        .viewport_h = 1300.0;

    assert_eq!(rows_built_by_view(&state, &root), 50);
}

#[test]
fn a_bottom_offset_left_over_from_before_the_grid_grew_does_not_blank_the_top() {
    let (mut state, root, _) = library_of(50);
    let bottom = BROWSER_HEADER_HEIGHT + 50.0 * H - 700.0;
    state.library_browsers.get_mut(&root).unwrap().grid_scroll = scrolled(bottom, 700.0);
    assert!(
        rows_built_by_view(&state, &root) < 50,
        "bottom window skips the top"
    );

    // Taller grid, same stale offset: iced has reset its own offset to 0.
    state
        .library_browsers
        .get_mut(&root)
        .unwrap()
        .grid_scroll
        .viewport_h = 1300.0;

    assert_eq!(rows_built_by_view(&state, &root), 50);
}

#[test]
fn a_small_table_is_built_whole() {
    let (state, root, _) = library_of(7);
    assert_eq!(rows_built_by_view(&state, &root), 7);
}

#[test]
fn a_stale_offset_after_the_table_shrinks_still_draws_its_rows() {
    let (mut state, root, _) = library_of(30);
    state.library_browsers.get_mut(&root).unwrap().grid_scroll = scrolled(500_000.0, 400.0);

    // The clamped window ends at the last row, so the list's tail is drawn
    // rather than blank space.
    let built = rows_built_by_view(&state, &root);
    assert!(built > 0 && built <= 30, "built {built}");
}

#[test]
fn a_selection_outside_the_window_is_kept_and_scrolling_reveals_it() {
    let (mut state, root, ids) = library_of(10_000);
    let target = 9_000;
    {
        let b = state.library_browsers.get_mut(&root).unwrap();
        b.grid_scroll = scrolled(0.0, 700.0);
        b.selected_row = Some(ids[target]);
    }

    // Off-screen: not built, but the view still draws and keeps the pick.
    let built = rows_built_by_view(&state, &root);
    assert!(built <= bound(700.0));
    assert_eq!(
        state.library_browsers[&root].selected_row,
        Some(ids[target])
    );

    let index = state
        .browser_row_position_of_selection(&root)
        .expect("selection is in the filtered rows");
    assert_eq!(index, target);
    let scroll = state.library_browsers[&root].grid_scroll;
    let offset = reveal_row_offset(index, scroll).expect("needs scrolling");

    let range = visible_range(
        offset - BROWSER_HEADER_HEIGHT,
        scroll.viewport_h,
        H,
        10_000,
        OVERSCAN_ROWS,
    );
    assert!(range.contains(&index), "{range:?} misses {index}");
    // And the grid built after the scroll contains it: bounded again.
    state.library_browsers.get_mut(&root).unwrap().grid_scroll = scrolled(offset, 700.0);
    assert!(rows_built_by_view(&state, &root) <= bound(700.0));
}

// ── widget identity across a window shift ────────────────────────

/// Draw the grid for `scroll` and return its element.
fn grid_element<'a>(
    state: &'a LibraryState,
    root: &'a std::path::Path,
    scroll: GridScroll,
    tokens: &'a signex_types::theme::ThemeTokens,
    browser: &'a mut LibraryBrowserState,
) -> iced::Element<'a, crate::library::messages::LibraryMessage> {
    browser.grid_scroll = scroll;
    let lib = state.library_at(root).expect("mounted");
    let cache = browser.view_cache.as_ref().expect("synced");
    let rows = &lib.tables[TABLE];
    let visible: Vec<&ComponentRow> = cache.visible.iter().map(|&i| &rows[i]).collect();
    let prices = crate::library::browser::price::PriceView::with_main_currency(
        &lib.pricing,
        state.clock,
        &cache.main_currency,
    );
    crate::library::browser::grid::view_grid(
        root,
        TABLE,
        &visible,
        &cache.columns,
        browser,
        crate::library::browser::grid::GridCaches {
            dangling: &lib.dangling,
            prices: &prices,
        },
        tokens,
        false,
    )
}

/// The row id each grid row's widget state is bound to, by table position.
fn bound_row_ids(
    tree: &iced::advanced::widget::Tree,
    scroll: GridScroll,
    count: usize,
) -> HashMap<usize, Uuid> {
    let range = visible_rows(scroll, count);
    // The grid column sits three levels under the container (sensor, key, scrollable).
    let grid = &tree.children[0].children[0].children[0];
    let lead = 1 + usize::from(range.start > 0);
    range
        .clone()
        .map(|i| {
            let slot = &grid.children[lead + (i - range.start)];
            (i, *slot.state.downcast_ref::<Uuid>())
        })
        .collect()
}

#[test]
fn a_rows_widget_state_never_follows_another_row_when_the_window_shifts() {
    // Focus and cursor live in the widget state of the cell editors. iced
    // diffs a column's children by position, so without a key a row would
    // inherit the state of whichever row held its slot before the shift, and
    // an inline edit would be committed to the wrong row.
    let (state, root, ids) = library_of(10_000);
    let tokens = theme_tokens(ThemeId::Signex);
    let mut browser = state.library_browsers[&root].clone();

    let top = scrolled(0.0, 700.0);
    let mut tree = {
        let element = grid_element(&state, &root, top, &tokens, &mut browser);
        iced::advanced::widget::Tree::new(&element)
    };
    for step in [1, 7, 30, 500] {
        let shifted = scrolled(BROWSER_HEADER_HEIGHT + step as f32 * H, 700.0);
        {
            let element = grid_element(&state, &root, shifted, &tokens, &mut browser);
            tree.diff(&element);
        }
        for (i, bound) in bound_row_ids(&tree, shifted, 10_000) {
            assert_eq!(bound, ids[i].as_uuid(), "row {i} after a {step}-row shift");
        }
    }
}

// ── fixed row height ─────────────────────────────────────────────

#[test]
fn the_fixed_row_height_fits_the_tallest_cell() {
    use crate::library::browser::BROWSER_TEXT_SIZE;
    use iced::Pixels;
    use iced::widget::text::LineHeight;
    let line = LineHeight::default()
        .to_absolute(Pixels(BROWSER_TEXT_SIZE))
        .0;
    // Cell content plus its padding, per kind (see `view_grid`): a text cell
    // pads 4 px above and below, an input pads 2 inside a 2-px cell, and the
    // unbound-pick button pads 2 inside a 2-px cell.
    let text_cell = line + 2.0 * 4.0;
    let input_cell = line + 2.0 * 2.0 + 2.0 * 2.0;
    let pick_button_cell = line + 2.0 * 2.0 + 2.0 * 2.0;
    for tallest in [text_cell, input_cell, pick_button_cell] {
        assert!(tallest <= H, "a {tallest}-px cell overflows the {H}-px row");
    }
}

// ── focus does not migrate ───────────────────────────────────────

type InputState =
    iced::widget::text_input::State<<iced::Renderer as iced::advanced::text::Renderer>::Paragraph>;

/// Apply `f` to every text-input state under `tree`.
fn each_input(tree: &mut iced::advanced::widget::Tree, f: &mut dyn FnMut(&mut InputState)) {
    if let iced::advanced::widget::tree::State::Some(state) = &mut tree.state
        && let Some(input) = state.downcast_mut::<InputState>()
    {
        f(input);
    }
    for child in &mut tree.children {
        each_input(child, f);
    }
}

#[test]
fn a_focused_cell_editor_never_appears_on_another_row_after_a_shift() {
    let (state, root, ids) = library_of(10_000);
    let tokens = theme_tokens(ThemeId::Signex);
    let mut browser = state.library_browsers[&root].clone();

    let start = scrolled(BROWSER_HEADER_HEIGHT + 100.0 * H, 700.0);
    let mut tree = {
        let element = grid_element(&state, &root, start, &tokens, &mut browser);
        iced::advanced::widget::Tree::new(&element)
    };
    let focused_row = 110;
    let range = visible_rows(start, 10_000);
    let lead = 1 + usize::from(range.start > 0);
    let mut focused_any = false;
    each_input(
        &mut tree.children[0].children[0].children[0].children[lead + focused_row - range.start],
        &mut |input| {
            input.focus();
            focused_any = true;
        },
    );
    assert!(focused_any, "the row has a cell editor to focus");

    for step in [1, 3, 13, 40] {
        let shifted = scrolled(BROWSER_HEADER_HEIGHT + (100 + step) as f32 * H, 700.0);
        {
            let element = grid_element(&state, &root, shifted, &tokens, &mut browser);
            tree.diff(&element);
        }
        let range = visible_rows(shifted, 10_000);
        let lead = 1 + usize::from(range.start > 0);
        for i in range.clone() {
            let slot =
                &mut tree.children[0].children[0].children[0].children[lead + i - range.start];
            let mut focused = false;
            each_input(slot, &mut |input| focused |= input.is_focused());
            if i != focused_row {
                assert!(
                    !focused,
                    "row {i} ({}) holds the focus after a {step}-row shift",
                    ids[i].as_uuid()
                );
            }
        }
    }
}

// ── the sensor across library tabs ───────────────────────────────

/// Draw `browser`'s grid on `cache`, redraw once, and return the heights
/// the grid asked the state to adopt, plus the cache for the next draw.
fn resize_reports(
    state: &LibraryState,
    root: &std::path::Path,
    browser: &mut LibraryBrowserState,
    cache: iced_runtime::user_interface::Cache,
) -> (Vec<f32>, iced_runtime::user_interface::Cache) {
    use crate::library::messages::LibraryMessage;
    use iced::advanced::clipboard;
    use iced::{Size, mouse, window};

    let tokens = theme_tokens(ThemeId::Signex);
    let mut renderer = crate::layout_probe::headless_renderer();
    let element = grid_element(state, root, browser.grid_scroll, &tokens, browser);
    let mut ui =
        iced_runtime::UserInterface::build(element, Size::new(1200.0, 800.0), cache, &mut renderer);
    let mut published: Vec<LibraryMessage> = Vec::new();
    let redraw = iced::Event::Window(window::Event::RedrawRequested(std::time::Instant::now()));
    let _ = ui.update(
        &[redraw.clone(), redraw],
        mouse::Cursor::Unavailable,
        &mut renderer,
        &mut clipboard::Null,
        &mut published,
    );
    let heights = published
        .into_iter()
        .filter_map(|m| match m {
            LibraryMessage::BrowserGridResized { viewport_h, .. } => Some(viewport_h),
            _ => None,
        })
        .collect();
    (heights, ui.into_cache())
}

#[test]
fn a_second_library_on_the_same_ui_cache_still_reports_its_height() {
    let (state_a, root_a, _) = library_at(200, "/tmp/grid-window/A.snxlib");
    let (state_b, root_b, _) = library_at(200, "/tmp/grid-window/B.snxlib");
    let mut browser_a = state_a.library_browsers[&root_a].clone();
    let mut browser_b = state_b.library_browsers[&root_b].clone();

    let (a, cache) = resize_reports(
        &state_a,
        &root_a,
        &mut browser_a,
        iced_runtime::user_interface::Cache::default(),
    );
    assert_eq!(a.len(), 1, "the first draw reports its height once");
    let laid_out = a[0];
    assert!(laid_out > 0.0);

    // B has never been measured: it must report, A's record notwithstanding.
    let (b, cache) = resize_reports(&state_b, &root_b, &mut browser_b, cache);
    assert_eq!(b, [laid_out]);

    // And a stale, non-zero height in the state is corrected the same way.
    browser_b.grid_scroll.viewport_h = 300.0;
    let (stale, _) = resize_reports(&state_b, &root_b, &mut browser_b, cache);
    assert_eq!(stale, [laid_out]);
}

#[test]
fn a_grid_whose_height_the_state_already_knows_publishes_nothing() {
    let (state, root, _) = library_of(200);
    let mut browser = state.library_browsers[&root].clone();
    let (first, cache) = resize_reports(
        &state,
        &root,
        &mut browser,
        iced_runtime::user_interface::Cache::default(),
    );
    browser.grid_scroll.viewport_h = first[0];

    let (again, _) = resize_reports(&state, &root, &mut browser, cache);

    assert!(again.is_empty(), "{again:?}");
}
