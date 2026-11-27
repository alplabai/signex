//! #2470 — an area tool's clicks in the window and its operand form (the
//! board, the layer and the corners from the board origin with Y up) add
//! the same area, each as one undo step; the key or menu row without
//! operands still arms the tool.

use signex_types::pcb::Zone;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::tests::{app_with_board, board_of, click, grid_only, live, right_click, run, undo};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, PcbMsg, PcbViewMsg, Signex};
use crate::keymap::AppCommandId;

/// How the clicks of a shape end it.
#[derive(Clone, Copy)]
enum Close {
    /// The last click is the second corner (a Fill).
    LastClick,
    /// A right-click once it has three corners.
    RightClick,
    /// A click on its first corner.
    FirstCorner,
}

struct Case {
    id: &'static str,
    /// The active layer for the clicks; `None` keeps the top copper.
    active: Option<PcbLayer>,
    clicks: &'static [(f64, f64)],
    close: Close,
    /// The same shape as operands, besides `document`.
    operands: &'static [&'static str],
}

/// The board of `tests::app_with_board` has its origin at the model's zero,
/// so a user point (x, y) is the model point (x, -y). Each first corner is
/// on the net-3 track along y = 0, so the copper areas take net 3.
const CASES: &[Case] = &[
    Case {
        id: "place_board_fill",
        active: None,
        clicks: &[(1.0, 0.0), (4.0, 3.0)],
        close: Close::LastClick,
        operands: &["layer=TopCopper", "start=1,0", "end=4,-3"],
    },
    Case {
        id: "place_board_region",
        active: None,
        clicks: &[(1.0, 0.0), (5.0, 0.0), (3.0, 3.0)],
        close: Close::RightClick,
        operands: &["layer=TopCopper", "corners=1,0;5,0;3,-3"],
    },
    Case {
        id: "place_board_polygon_pour",
        active: None,
        clicks: &[(1.0, 0.0), (1.0, -4.0), (9.0, -4.0), (9.0, 4.0), (1.0, 4.0)],
        close: Close::RightClick,
        operands: &["layer=TopCopper", "corners=1,0;1,4;9,4;9,-4;1,-4"],
    },
    Case {
        id: "place_board_keepout",
        active: Some(PcbLayer::Technical(TechnicalLayer::KeepoutTop)),
        clicks: &[(1.0, -1.0), (4.0, -1.0), (4.0, 1.0)],
        close: Close::RightClick,
        operands: &["layer=TopKeepout", "corners=1,1;4,1;4,-1"],
    },
    Case {
        id: "place_board_pour_cutout",
        active: None,
        clicks: &[(2.0, 2.0), (6.0, 2.0), (6.0, 3.0), (2.0, 3.0)],
        close: Close::FirstCorner,
        operands: &["layer=TopCopper", "corners=2,-2;6,-2;6,-3;2,-3"],
    },
];

/// The one area on the board in view, its id cleared to compare.
fn only_area(app: &Signex, path: &std::path::Path) -> Zone {
    let zones = &board_of(app, path).zones;
    assert_eq!(zones.len(), 1, "one area");
    Zone {
        uuid: Uuid::nil(),
        ..zones[0].clone()
    }
}

fn clicked(case: &Case) -> Zone {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    if let Some(layer) = case.active {
        let picked = PcbViewMsg::ActiveLayerPicked(layer);
        let _ = app.update(Message::Pcb(PcbMsg::View(picked)));
    }
    run(&mut app, case.id);
    assert!(app.interaction_state.pcb_tool.is_some(), "{} arms", case.id);
    for &(x, y) in case.clicks {
        click(&mut app, x, y);
    }
    match case.close {
        Close::LastClick => {}
        Close::RightClick => right_click(&mut app),
        Close::FirstCorner => click(&mut app, case.clicks[0].0, case.clicks[0].1),
    }
    only_area(&app, &path)
}

#[test]
fn the_operand_form_adds_the_area_the_clicks_add() {
    for case in CASES {
        let by_click = clicked(case);
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_board(dir.path());
        let before = live(&app, &path);
        let command = AppCommandId::new(case.id).unwrap();
        let document = format!("document={}", path.display());
        let mut operands = vec![document.as_str()];
        operands.extend(case.operands);
        let args = CommandArgs::parse_operands(&command, &operands).expect("operands parse");
        let (outcome, _task) = app.run_operands(&command, &args);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{}: {outcome:?}",
            case.id
        );
        assert!(app.interaction_state.pcb_tool.is_none(), "no tool armed");
        assert_eq!(only_area(&app, &path), by_click, "{}", case.id);
        undo(&mut app);
        assert_eq!(live(&app, &path), before, "{}: one undo step", case.id);
    }
}
