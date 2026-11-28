//! The pane menu's rules (mockup row 207): Tile Grid's panes, Move Tab to
//! Other Pane, Close Pane, and a grid split and its sizes in the kept form.

use std::path::PathBuf;

use serde_json::json;

use super::PaneTab;
use super::model::{PaneSplit, SplitRefusal};
use super::store::{shares_from_json, split_from_json, split_to_json, with_shares};
use crate::app::TileLayout;

const DOCKED: [&str; 4] = ["power", "mcu", "board", "list"];

/// The tabs each shown pane holds, by key.
fn panes_of(split: &PaneSplit<&'static str>) -> Vec<Vec<&'static str>> {
    split
        .resolve(&DOCKED)
        .iter()
        .map(|pane| pane.tabs.iter().map(|&at| DOCKED[at]).collect())
        .collect()
}

/// One pane per tab, side by side, `active` focused.
fn one_each(active: &'static str) -> PaneSplit<&'static str> {
    let groups: Vec<Vec<usize>> = (0..DOCKED.len()).map(|at| vec![at]).collect();
    PaneSplit::from_groups(&DOCKED, &groups, TileLayout::SideBySide, &active).expect("panes")
}

/// Every tab is listed in its group's pane; the pane of the active tab has
/// the focus, shows it and takes the tabs opened later.
#[test]
fn panes_from_groups_list_every_tab_and_focus_the_active_one() {
    let groups = vec![vec![0, 1], vec![2], vec![3]];
    let split = PaneSplit::from_groups(&DOCKED, &groups, TileLayout::Grid, &"mcu").expect("a grid");
    assert_eq!(split.direction, TileLayout::Grid);
    assert_eq!(
        panes_of(&split),
        [vec!["power", "mcu"], vec!["board"], vec!["list"]]
    );
    assert_eq!((split.focused, split.rest), (0, 0));
    assert_eq!(split.panes[0].shown, Some("mcu"));
    let one = vec![vec![0, 1, 2, 3]];
    assert!(PaneSplit::from_groups(&DOCKED, &one, TileLayout::Grid, &"mcu").is_none());
}

/// Move Tab to Other Pane: the tab goes to the next pane, which takes the
/// focus; the pane it left closes when empty, unless one pane would be left.
#[test]
fn a_moved_tab_joins_the_next_pane_and_takes_the_focus() {
    let mut split = one_each("mcu");
    split.move_tab(&DOCKED, &"mcu").expect("moved");
    assert_eq!(
        panes_of(&split),
        [vec!["power"], vec!["mcu", "board"], vec!["list"]]
    );
    assert_eq!(split.panes[split.focused].shown, Some("mcu"));

    let mut last = one_each("list");
    last.move_tab(&DOCKED, &"list").expect("moved");
    assert_eq!(
        panes_of(&last)[0],
        ["power", "list"],
        "the last pane wraps to the first"
    );

    let groups = [vec![0, 1, 2], vec![3]];
    let mut two =
        PaneSplit::from_groups(&DOCKED, &groups, TileLayout::Stacked, &"list").expect("two panes");
    assert_eq!(two.move_tab(&DOCKED, &"list"), Err(SplitRefusal::LastTab));
}

/// Close Pane: its tabs join the next pane (the one before for the last),
/// which takes the focus; down to one pane the split is over.
#[test]
fn a_closed_pane_hands_its_tabs_on() {
    let mut split = one_each("board");
    assert_eq!(split.close_pane(&DOCKED, 2), Ok(true));
    assert_eq!(
        panes_of(&split),
        [vec!["power"], vec!["mcu"], vec!["board", "list"]]
    );
    assert_eq!(split.focused, 2);
    assert_eq!(split.close_pane(&DOCKED, 2), Ok(true));
    assert_eq!(
        panes_of(&split),
        [vec!["power"], vec!["mcu", "board", "list"]]
    );
    assert_eq!(split.close_pane(&DOCKED, 0), Ok(false), "one pane left");
    assert_eq!(split.close_pane(&DOCKED, 9), Err(SplitRefusal::NotTiled));
}

/// A grid split is kept as `tile_grid` and read back; the sizes of a drag
/// ride beside it in thousandths, and equal sizes write none.
#[test]
fn a_grid_and_its_sizes_survive_the_kept_form() {
    let folder = PathBuf::from("/w/buck-3v3");
    let docked: Vec<PaneTab> = ["power.snxsch", "mcu.snxsch", "buck-3v3.snxpcb"]
        .iter()
        .map(|name| PaneTab {
            path: folder.join(name),
            page: "schematic".to_string(),
        })
        .collect();
    let groups = vec![vec![0], vec![1], vec![2]];
    let split =
        PaneSplit::from_groups(&docked, &groups, TileLayout::Grid, &docked[1]).expect("grid");
    let old = json!({ "panes_note": "kept", "shares": [1, 2] });
    let shares: &[u16] = &[400, 750];
    let written = with_shares(split_to_json(&split, &folder, Some(&old)), Some(shares));
    assert_eq!(written["direction"], "tile_grid");
    assert_eq!(written["panes_note"], "kept", "an unknown key is kept");
    assert_eq!(shares_from_json(&written), Some(vec![400, 750]));
    assert_eq!(split_from_json(&written, &folder), Some(split.clone()));

    let equal = with_shares(written, None);
    assert_eq!(equal.get("shares"), None);
    assert_eq!(shares_from_json(&json!({ "shares": [400, 1001] })), None);
    assert_eq!(shares_from_json(&json!({ "shares": "wide" })), None);
}
