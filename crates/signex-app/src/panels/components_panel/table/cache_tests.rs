//! The table cache is rebuilt in `update` only when its inputs change, and
//! a frame draws only the lines in view (docs/architecture/
//! library-scale-bench.md): 25,000 parts stay smooth.

use signex_library::templates::ColumnKey;

use super::test_fixture::{capacitor, parts, show};
use super::{TableLine, body};
use crate::library::messages::ComponentsTableMsg as M;
use crate::panels::components_panel::tests::library_with;

/// A sync with nothing changed (a hover, a scroll) reuses the arranged
/// rows; a sort change, a search or new data rebuilds them once.
#[test]
fn the_rows_are_rearranged_only_when_an_input_changes() {
    let mut state = parts();
    show(&mut state, "*");
    let passes =
        |s: &crate::library::state::LibraryState| s.components_panel.table.cache.arrange_passes;
    let before = passes(&state);
    state.components_panel.hovered = None;
    state.sync_view_caches();
    state.sync_view_caches();
    assert_eq!(passes(&state), before, "nothing changed");

    let _ = state
        .components_panel
        .table
        .apply(M::HeaderPressed(ColumnKey::Manufacturer));
    state.sync_view_caches();
    assert_eq!(passes(&state), before + 1, "a new sort");

    state.components_panel.filter = "C1".into();
    state.sync_view_caches();
    assert_eq!(passes(&state), before + 2, "a new search");
    state.sync_view_caches();
    assert_eq!(passes(&state), before + 2);

    // Opening a group rebuilds the lines, not the arrangement.
    let _ = state
        .components_panel
        .table
        .apply(M::GroupToggled(Vec::new()));
    state.sync_view_caches();
    assert_eq!(passes(&state), before + 2);
}

/// The search narrows the table too: `100n` (a value word) keeps the two
/// 100 nF capacitors.
#[test]
fn the_search_narrows_the_table() {
    let mut state = parts();
    show(&mut state, "*");
    state.components_panel.filter = "100n".into();
    state.sync_view_caches();
    let mut drawn = super::test_fixture::drawn_parts(&state);
    drawn.sort();
    assert_eq!(drawn, ["C1", "C2"]);
    assert_eq!(state.components_panel.table.cache.base.len(), 9);
}

/// 25,000 capacitors: grouped by the category's view the table draws only
/// its group lines; flat, a frame builds a window of lines, not all of
/// them, and a hover does not sort again.
#[test]
fn twenty_five_thousand_parts_build_once_and_draw_a_window() {
    const N: usize = 25_000;
    let packages = ["0201", "0402", "0603", "0805", "1206"];
    let rows = (0..N)
        .map(|i| {
            capacitor(
                &format!("C{i:05}"),
                f64::from((i % 97) as u32 + 1) * 1e-9,
                f64::from((i % 7) as u32 + 1) * 6.3,
                packages[i % packages.len()],
            )
        })
        .collect();
    let mut state = library_with(rows);
    show(&mut state, "capacitor");
    let table = &state.components_panel.table;
    assert_eq!(table.cache.base.len(), N);
    assert_eq!(
        table.cache.lines.len(),
        packages.len(),
        "only the package groups"
    );
    assert!(
        table
            .cache
            .lines
            .iter()
            .all(|l| matches!(l, TableLine::Group { .. }))
    );

    let passes = table.cache.arrange_passes;
    state.sync_view_caches();
    assert_eq!(state.components_panel.table.cache.arrange_passes, passes);

    for _ in 0..2 {
        let _ = state.components_panel.table.apply(M::GroupRemoved(0));
    }
    state.sync_view_caches();
    let lines = state.components_panel.table.cache.lines.len();
    assert_eq!(lines, N);
    let window = body::window(lines, 12_000.0, 600.0);
    assert!(window.len() < 100, "a frame builds {} lines", window.len());
    assert!(window.start > 0 && window.end < lines);
}
