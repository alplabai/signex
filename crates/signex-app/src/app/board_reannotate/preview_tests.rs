//! #2533 — the preview partitions the board: renamed, kept, already in
//! order; the in-order parts show only when asked.

use signex_engine::pcb::designators::{ReannotateOptions, plan_reannotate};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{PreviewRow, in_order, rows};
use crate::app::board_reannotate::BoardReannotateState;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Footprints `(n, reference, x, locked)` on the top side in a row, each
/// linked to a part.
fn board(parts: &[(u128, &str, f64, bool)]) -> PcbBoard {
    let footprints: Vec<_> = parts
        .iter()
        .map(|&(n, reference, x, locked)| {
            serde_json::json!({
                "uuid": id(n), "reference": reference, "value": "v",
                "position": {"x": x, "y": 0.0}, "layer": "TopCopper", "locked": locked,
                "schematic_link": {"sheet": [], "units": [id(n + 0x1000)]},
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({ "uuid": id(999), "footprints": footprints }))
        .expect("board")
}

fn state(board: &PcbBoard, show_in_order: bool) -> BoardReannotateState {
    let plan = plan_reannotate(board, ReannotateOptions::default());
    BoardReannotateState {
        board: "b.snxpcb".into(),
        board_name: "b.snxpcb".into(),
        options: ReannotateOptions::default(),
        in_order: in_order(board, &plan),
        plan,
        revision: 1,
        problem: None,
        show_in_order,
    }
}

#[test]
fn every_footprint_is_renamed_kept_or_already_in_order() {
    // Left to right: R1 is in order, R3 should be R2, R2 should be R3, and
    // the locked R9 keeps its designator.
    let b = board(&[
        (1, "R1", 0.0, false),
        (2, "R3", 10.0, false),
        (3, "R2", 20.0, false),
        (4, "R9", 30.0, true),
    ]);
    let s = state(&b, false);
    assert_eq!(s.in_order, ["R1"]);
    assert_eq!(s.in_order.len(), s.plan.unchanged);
    assert_eq!(
        s.plan.renames.len() + s.plan.kept.len() + s.in_order.len(),
        b.footprints.len()
    );
    let hidden = rows(&s);
    assert!(
        hidden.iter().all(|r| !matches!(r, PreviewRow::InOrder(_))),
        "hidden by default"
    );
    assert!(matches!(hidden.first(), Some(PreviewRow::Renamed(_))));
    assert!(matches!(hidden.last(), Some(PreviewRow::Kept(_))));
}

#[test]
fn the_toggle_lists_the_parts_already_in_order_last() {
    let b = board(&[
        (1, "R1", 0.0, false),
        (2, "R3", 10.0, false),
        (3, "R2", 20.0, false),
    ]);
    let shown = rows(&state(&b, true));
    assert_eq!(shown.last(), Some(&PreviewRow::InOrder("R1")));
    assert_eq!(shown.len(), 3);
}
