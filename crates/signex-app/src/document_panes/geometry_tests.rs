//! Mockup row 207: where the panes sit after a splitter drag, the splitters
//! between them, Tile Grid's grouping by document kind, the sizes kept in
//! thousandths and Sync Zoom's centre.

use super::geometry::{MIN_PANE_EXTENT, dragged};
use super::{PaneShares, grid_groups, pane_tiles, splitters, zoom_kept_centre};
use crate::app::TileLayout;

fn sizes(count: usize, layout: TileLayout, shares: &[f32]) -> Vec<(f32, f32, f32, f32)> {
    pane_tiles(count, (1000.0, 600.0), layout, shares)
        .iter()
        .map(|t| (t.position.0, t.position.1, t.size.0, t.size.1))
        .collect()
}

/// Shares that fit the panes size them; the last pane takes the rest.
#[test]
fn shares_size_the_panes_and_the_last_takes_the_rest() {
    assert_eq!(
        sizes(2, TileLayout::SideBySide, &[0.3, 0.7]),
        [(0.0, 0.0, 300.0, 600.0), (300.0, 0.0, 700.0, 600.0)]
    );
    assert_eq!(
        sizes(2, TileLayout::Stacked, &[0.25, 0.75]),
        [(0.0, 0.0, 1000.0, 150.0), (0.0, 150.0, 1000.0, 450.0)]
    );
}

/// Shares made for another count, or that do not add up to a whole, are
/// equal panes: the split Tile uses.
#[test]
fn shares_that_do_not_fit_are_equal_panes() {
    let equal = sizes(2, TileLayout::SideBySide, &[]);
    assert_eq!(sizes(2, TileLayout::SideBySide, &[0.2, 0.3, 0.5]), equal);
    assert_eq!(sizes(2, TileLayout::SideBySide, &[0.2, 0.2]), equal);
    assert_eq!(sizes(2, TileLayout::SideBySide, &[f32::NAN, 1.0]), equal);
    assert_eq!(equal[0].2, 500.0);
}

/// A splitter sits between each two neighbours; a grid of four has the
/// column line down the whole area and the row line across it, a grid of
/// three a column line down the top row only.
#[test]
fn splitters_run_between_the_panes() {
    let lines = splitters(3, (900.0, 600.0), TileLayout::SideBySide, &[]);
    let at: Vec<(usize, bool, f32)> = lines.iter().map(|s| (s.index, s.vertical, s.at)).collect();
    assert_eq!(at, [(0, true, 300.0), (1, true, 600.0)]);
    assert_eq!(lines[0].length, 600.0);

    let grid = splitters(4, (1000.0, 600.0), TileLayout::Grid, &[]);
    assert_eq!(grid.len(), 2);
    assert_eq!(
        (grid[0].vertical, grid[0].at, grid[0].length),
        (true, 500.0, 600.0)
    );
    assert_eq!(
        (grid[1].vertical, grid[1].at, grid[1].length),
        (false, 300.0, 1000.0)
    );

    let three = splitters(3, (1000.0, 600.0), TileLayout::Grid, &[]);
    assert_eq!(
        three[0].length, 300.0,
        "the column line stops at the top row"
    );
    assert!(splitters(1, (1000.0, 600.0), TileLayout::SideBySide, &[]).is_empty());
}

/// A drag moves the line between its two panes only, and stops short of
/// either edge.
#[test]
fn a_drag_moves_one_line_and_keeps_both_panes_open() {
    let shares =
        dragged(3, (900.0, 600.0), TileLayout::SideBySide, &[], 0, 200.0).expect("a splitter");
    let placed = sizes_at(3, (900.0, 600.0), &shares);
    for (got, want) in placed.iter().zip([200.0, 400.0, 300.0]) {
        assert!((got - want).abs() <= 1.0, "{placed:?}");
    }
    assert_eq!(
        placed.iter().sum::<f32>(),
        900.0,
        "the panes cover the area"
    );

    let far = dragged(2, (1000.0, 600.0), TileLayout::SideBySide, &[], 0, 5.0).expect("a splitter");
    assert!((far[0] * 1000.0 - MIN_PANE_EXTENT).abs() < 0.5, "{far:?}");
    assert!(dragged(2, (1000.0, 600.0), TileLayout::SideBySide, &[], 1, 5.0).is_none());

    let grid = dragged(4, (1000.0, 600.0), TileLayout::Grid, &[], 1, 450.0).expect("row line");
    assert_eq!(grid, [0.5, 0.75]);
    assert!(dragged(2, (1000.0, 600.0), TileLayout::Grid, &[], 1, 450.0).is_none());
}

fn sizes_at(count: usize, area: (f32, f32), shares: &[f32]) -> Vec<f32> {
    pane_tiles(count, area, TileLayout::SideBySide, shares)
        .iter()
        .map(|t| t.size.0)
        .collect()
}

/// Tile Grid: one tab per pane up to four; with more, one kind per pane
/// where it can, the largest kind halved until there are four.
#[test]
fn tile_grid_groups_documents_by_kind() {
    assert_eq!(
        grid_groups(&["sch", "pcb", "sch"]),
        [vec![0], vec![1], vec![2]]
    );
    assert_eq!(
        grid_groups(&["sch", "sch", "pcb", "sch", "list"]),
        [vec![0, 1], vec![3], vec![2], vec![4]]
    );
    assert_eq!(
        grid_groups(&["a", "b", "c", "d", "e", "f"]),
        [vec![0], vec![1], vec![2], vec![3, 4, 5]]
    );
    assert!(grid_groups::<&str>(&[]).is_empty());
}

/// Sizes are kept in thousandths and read back only for the panes they were
/// made for.
#[test]
fn shares_are_kept_in_thousandths_for_their_panes() {
    let shares = PaneShares {
        layout: TileLayout::SideBySide,
        count: 2,
        values: vec![0.3, 0.7],
    };
    assert_eq!(shares.per_mille(), [300, 700]);
    let back = PaneShares::from_per_mille(TileLayout::SideBySide, 2, &[300, 700]).expect("read");
    assert_eq!(
        back.for_view(TileLayout::SideBySide, 2).to_vec(),
        vec![0.3, 0.7]
    );
    assert!(back.for_view(TileLayout::SideBySide, 3).is_empty());
    assert!(back.for_view(TileLayout::Stacked, 2).is_empty());
    assert!(PaneShares::from_per_mille(TileLayout::Grid, 4, &[1200, 500]).is_none());
}

/// Sync Zoom keeps the point at the pane's centre where it is.
#[test]
fn sync_zoom_keeps_the_centre_of_the_pane() {
    let camera = (100.0, 50.0, 2.0);
    let synced = zoom_kept_centre(camera, (400.0, 300.0), 4.0);
    assert_eq!(synced.2, 4.0);
    let centre_before = ((200.0 - camera.0) / camera.2, (150.0 - camera.1) / camera.2);
    let centre_after = ((200.0 - synced.0) / synced.2, (150.0 - synced.1) / synced.2);
    assert_eq!(centre_before, centre_after);
    assert_eq!(zoom_kept_centre(camera, (400.0, 300.0), f32::NAN), camera);
}
