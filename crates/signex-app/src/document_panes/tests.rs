//! The pane rules and their `prefs.json` form (mockup row 207, #1307).

use std::path::{Path, PathBuf};

use serde_json::json;

use super::PaneTab;
use super::model::{DropEdge, DropTarget, MAX_PANES, PaneId, PaneSplit, SplitRefusal, drop_target};
use super::store::{split_from_json, split_to_json};
use crate::app::TileLayout;

const SHEETS: [&str; 3] = ["power", "mcu", "board"];

/// The tabs each shown pane holds, by key.
fn panes_of(split: &PaneSplit<&'static str>, docked: &[&'static str]) -> Vec<Vec<&'static str>> {
    split
        .resolve(docked)
        .iter()
        .map(|pane| pane.tabs.iter().map(|&at| docked[at]).collect())
        .collect()
}

#[test]
fn a_drop_near_a_side_splits_there_and_the_middle_keeps_the_tab() {
    let area = (100.0, 50.0, 800.0, 400.0);
    assert_eq!(
        drop_target(area, (850.0, 250.0)),
        Some(DropTarget::Edge(DropEdge::Right))
    );
    assert_eq!(
        drop_target(area, (120.0, 250.0)),
        Some(DropTarget::Edge(DropEdge::Left))
    );
    assert_eq!(
        drop_target(area, (500.0, 60.0)),
        Some(DropTarget::Edge(DropEdge::Top))
    );
    assert_eq!(
        drop_target(area, (500.0, 440.0)),
        Some(DropTarget::Edge(DropEdge::Bottom))
    );
    assert_eq!(drop_target(area, (500.0, 250.0)), Some(DropTarget::Centre));
    assert_eq!(drop_target(area, (50.0, 250.0)), None, "off the area");
    assert_eq!(drop_target(area, (f32::NAN, 250.0)), None);
    assert_eq!(drop_target((0.0, 0.0, 0.0, 10.0), (0.0, 5.0)), None);
}

/// Mockup row 207, decided as drawn: a drop on an edge moves the document
/// into the new pane, which takes the focus; the others stay behind.
#[test]
fn a_drop_on_an_edge_moves_the_tab_into_a_new_pane() {
    let split =
        PaneSplit::first(&SHEETS, &"board", &"board", DropEdge::Right, false).expect("split");
    assert_eq!(split.direction, TileLayout::SideBySide);
    assert_eq!(
        panes_of(&split, &SHEETS),
        [vec!["power", "mcu"], vec!["board"]]
    );
    let shown = split.resolve(&SHEETS);
    assert_eq!(split.focused_of(&shown, 2), Some(1), "the new pane");
    // The pane left behind shows its first tab: its active one moved out.
    assert_eq!(split.drawn_tabs(&SHEETS, &shown, 1, 2), [0, 2]);

    let split = PaneSplit::first(&SHEETS, &"power", &"mcu", DropEdge::Top, false).expect("split");
    assert_eq!(split.direction, TileLayout::Stacked);
    assert_eq!(
        panes_of(&split, &SHEETS),
        [vec!["power"], vec!["mcu", "board"]]
    );
    let shown = split.resolve(&SHEETS);
    assert_eq!(
        split.drawn_tabs(&SHEETS, &shown, 0, 0),
        [0, 1],
        "the pane left behind keeps showing the tab that was active"
    );
}

/// With Ctrl held the drop opens a second view: the document is in both
/// panes, and either can have the focus.
#[test]
fn a_ctrl_drop_opens_a_second_view_and_keeps_the_tab() {
    let docked = ["power"];
    assert_eq!(
        PaneSplit::first(&docked, &"power", &"power", DropEdge::Right, false),
        Err(SplitRefusal::LastTab),
        "the only tab cannot leave its pane"
    );
    let mut split = PaneSplit::first(&docked, &"power", &"power", DropEdge::Right, true)
        .expect("a second view of the only tab");
    assert_eq!(panes_of(&split, &docked), [vec!["power"], vec!["power"]]);
    assert_eq!(split.focused, 1);
    assert_eq!(
        split.focus(&docked, 0, 0),
        Some(0),
        "the same tab stays active"
    );
    assert_eq!(split.focused, 0);

    let mut split =
        PaneSplit::first(&SHEETS, &"mcu", &"mcu", DropEdge::Left, false).expect("split");
    split
        .add(&SHEETS, &"board", DropEdge::Right, true)
        .expect("a second view of a tab of the rest pane");
    assert_eq!(
        panes_of(&split, &SHEETS),
        [vec!["mcu"], vec!["power", "board"], vec!["board"]]
    );
}

/// A further drop adds a pane on that side; across the panes, or past four,
/// it is refused. A pane emptied by a move goes.
#[test]
fn further_drops_add_panes_up_to_four_and_never_across() {
    let docked = ["a", "b", "c", "d", "e"];
    let mut split = PaneSplit::first(&docked, &"e", &"e", DropEdge::Right, false).expect("split");
    assert_eq!(
        split.add(&docked, &"a", DropEdge::Top, false),
        Err(SplitRefusal::Crosswise)
    );
    split
        .add(&docked, &"a", DropEdge::Left, false)
        .expect("third");
    split
        .add(&docked, &"d", DropEdge::Right, false)
        .expect("fourth");
    assert_eq!(
        panes_of(&split, &docked),
        [vec!["a"], vec!["b", "c"], vec!["e"], vec!["d"]]
    );
    assert_eq!(split.panes.len(), MAX_PANES);
    assert_eq!(
        split.add(&docked, &"b", DropEdge::Right, false),
        Err(SplitRefusal::TooManyPanes)
    );
    assert_eq!(
        split.add(&docked, &"z", DropEdge::Right, false),
        Err(SplitRefusal::NotDocked)
    );

    // Moving "e", alone in its pane, to the far left leaves no empty pane.
    let mut split = PaneSplit::first(&docked, &"e", &"e", DropEdge::Right, false).expect("split");
    split
        .add(&docked, &"e", DropEdge::Left, false)
        .expect("moved");
    assert_eq!(
        panes_of(&split, &docked),
        [vec!["e"], vec!["a", "b", "c", "d"]]
    );
    assert_eq!(split.panes.len(), 2);
    assert_eq!((split.focused, split.rest), (0, 1));
}

/// A tab strip click into the other pane moves the focus there; the pane it
/// left keeps drawing the tab it showed. A tab opened later joins the rest
/// pane.
#[test]
fn the_focus_follows_the_active_tab_and_a_pane_remembers_its_tab() {
    let mut split =
        PaneSplit::first(&SHEETS, &"board", &"board", DropEdge::Right, false).expect("split");
    split.follow_active(&SHEETS, &"mcu");
    assert_eq!(split.focused, 0);
    split.follow_active(&SHEETS, &"board");
    assert_eq!(split.focused, 1);
    let shown = split.resolve(&SHEETS);
    assert_eq!(
        split.drawn_tabs(&SHEETS, &shown, 1, 2),
        [1, 2],
        "the left pane still shows mcu"
    );

    let opened = ["power", "mcu", "board", "io"];
    assert_eq!(
        panes_of(&split, &opened),
        [vec!["power", "mcu", "io"], vec!["board"]]
    );
}

/// Closing a pane's last tab ends that pane; down to one pane the split is
/// over.
#[test]
fn closing_a_panes_last_tab_drops_the_pane_and_one_pane_ends_the_split() {
    let mut split =
        PaneSplit::first(&SHEETS, &"board", &"board", DropEdge::Right, false).expect("split");
    let left = ["power", "mcu"];
    assert!(!split.prune(&left), "one pane is no split");
    assert_eq!(split.panes.len(), 1);
}

fn tab(folder: &Path, name: &str, page: &str) -> PaneTab {
    PaneTab {
        path: folder.join(name),
        page: page.to_string(),
    }
}

/// The kept form names documents from the project folder, reads back the
/// same split, keeps keys it does not know, and refuses what it cannot read.
#[test]
fn a_kept_split_survives_a_write_and_a_read() {
    let folder = PathBuf::from("/w/buck-3v3");
    let docked = [
        tab(&folder, "power.snxsch", "schematic"),
        tab(&folder, "buck-3v3.snxpcb", "pcb"),
    ];
    let split =
        PaneSplit::first(&docked, &docked[1], &docked[1], DropEdge::Right, true).expect("split");
    let old = json!({ "zoom_sync": true, "direction": "stacked" });
    let written = split_to_json(&split, &folder, Some(&old));
    assert_eq!(written["direction"], "side_by_side");
    assert_eq!(written["zoom_sync"], true, "an unknown key is kept");
    assert_eq!(written["panes"][1]["tabs"][0]["path"], "buck-3v3.snxpcb");
    assert_eq!(split_from_json(&written, &folder), Some(split));

    for broken in [
        json!({ "direction": "grid", "panes": [], "rest": 0, "focused": 0 }),
        json!({ "direction": "stacked", "panes": [{ "tabs": [] }], "rest": 0, "focused": 0 }),
        json!({ "direction": "stacked", "panes": [{ "tabs": [] }, { "tabs": [] }], "rest": 2, "focused": 0 }),
        json!({ "direction": "stacked", "panes": [{ "tabs": [{ "path": "a" }] }, { "tabs": [] }], "rest": 0, "focused": 0 }),
        json!("side_by_side"),
    ] {
        assert_eq!(split_from_json(&broken, &folder), None, "{broken}");
    }
}

fn ids_of(split: &PaneSplit<&'static str>) -> Vec<PaneId> {
    split.panes.iter().map(|pane| pane.id).collect()
}

/// A pane keeps its id when an earlier pane closes, so the focus stays with
/// its pane; a new pane never takes the id of one that closed.
#[test]
fn a_pane_keeps_its_id_when_an_earlier_pane_closes() {
    let docked = ["a", "b", "c"];
    let mut split = PaneSplit::first(&docked, &"a", &"c", DropEdge::Left, false).expect("split");
    split
        .add(&docked, &"c", DropEdge::Right, true)
        .expect("second view");
    assert_eq!(ids_of(&split), [PaneId(1), PaneId(0), PaneId(2)]);
    assert_eq!(split.id_of(split.focused), Some(PaneId(2)));

    let left = ["b", "c"];
    assert!(split.prune(&left), "two panes left");
    assert_eq!(ids_of(&split), [PaneId(0), PaneId(2)]);
    assert_eq!(split.focused, 1, "its place moved");
    assert_eq!(
        split.id_of(split.focused),
        Some(PaneId(2)),
        "its id did not"
    );

    split
        .add(&left, &"b", DropEdge::Right, false)
        .expect("moved");
    assert_eq!(split.id_of(split.focused), Some(PaneId(3)), "a fresh id");
}

/// The ids are kept with the split; an entry written before panes had ids
/// reads them as the panes' places, and two panes with one id are refused.
#[test]
fn a_kept_split_keeps_its_pane_ids() {
    let folder = PathBuf::from("/w/buck-3v3");
    let docked = [
        tab(&folder, "power.snxsch", "schematic"),
        tab(&folder, "buck-3v3.snxpcb", "pcb"),
    ];
    let split =
        PaneSplit::first(&docked, &docked[1], &docked[0], DropEdge::Left, false).expect("split");
    let written = split_to_json(&split, &folder, None);
    assert_eq!(written["panes"][0]["id"], 1);
    assert_eq!(written["next_id"], 2);
    assert_eq!(split_from_json(&written, &folder), Some(split));

    let before_ids = json!({
        "direction": "stacked",
        "panes": [
            { "tabs": [{ "path": "power.snxsch", "page": "schematic" }] },
            { "tabs": [] },
        ],
        "rest": 1,
        "focused": 0,
    });
    let read = split_from_json(&before_ids, &folder).expect("read");
    let ids: Vec<PaneId> = read.panes.iter().map(|pane| pane.id).collect();
    assert_eq!(ids, [PaneId(0), PaneId(1)]);
    assert_eq!(read.next_id, 2);

    let twice = json!({
        "direction": "stacked",
        "panes": [{ "id": 4, "tabs": [] }, { "id": 4, "tabs": [] }],
        "rest": 0,
        "focused": 0,
    });
    assert_eq!(split_from_json(&twice, &folder), None);
}
