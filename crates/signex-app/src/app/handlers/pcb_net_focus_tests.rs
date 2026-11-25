//! #2514 — a drag or Move keeps the nets of what it carries bright and dims
//! the others, the board under the drag preview too; a highlighted net
//! wins over a tool; the scene is drawn again when the carry starts and
//! ends, Esc included.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::pcb_canvas::{CarryKind, PcbCarry};

const U1: Uuid = Uuid::from_u128(0x10);
const TRACK: Uuid = Uuid::from_u128(0x40);
const VIA: Uuid = Uuid::from_u128(0x50);

/// U1 at (10, 10) with a pad on net 1 and one on net 2, a net-3 track and
/// a via on net 2.
fn board() -> PcbBoard {
    let pad = |n: u128, x: f64, net: u32| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": n.to_string(), "pad_type": "smd",
            "shape": "rect", "position": {"x": x, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": net, "name": net.to_string()}
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "1"}, {"number": 2, "name": "2"},
                 {"number": 3, "name": "3"}],
        "footprints": [{
            "uuid": U1, "reference": "U1", "position": {"x": 10.0, "y": 10.0},
            "rotation": 0.0, "layer": "TopCopper",
            "pads": [pad(0x11, -2.0, 1), pad(0x12, 2.0, 2)]
        }],
        "segments": [{"uuid": TRACK, "start": {"x": 0.0, "y": 20.0},
                      "end": {"x": 10.0, "y": 20.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 3}],
        "vias": [{"uuid": VIA, "position": {"x": 20.0, "y": 30.0},
                  "diameter": 0.6, "drill": 0.3, "net": 2}]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn drawn_nets(app: &Signex) -> Vec<u32> {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .and_then(|snapshot| snapshot.paint.net_focus())
        .map(|focus| focus.nets.clone())
        .unwrap_or_default()
}

/// A drag of `kind` started at (10, 10) carrying `footprints`, `vias` and
/// `items`, moved 1 mm: what the press and the first move leave.
fn start_carry(
    app: &mut Signex,
    path: &Path,
    kind: CarryKind,
    footprints: &[Uuid],
    vias: &[Uuid],
    items: &[Uuid],
) {
    let set = |ids: &[Uuid]| ids.iter().copied().collect::<BTreeSet<_>>();
    let mut carry = PcbCarry::new(kind, path.to_path_buf(), None, set(footprints));
    carry.vias = set(vias);
    carry.items = set(items);
    carry.anchor = Some(Point::new(10.0, 10.0));
    carry.delta = Point::new(1.0, 0.0);
    app.interaction_state.pcb_carry = Some(carry);
    app.refresh_pcb_highlight();
}

#[test]
fn a_footprint_drag_keeps_its_pads_nets_bright_under_the_preview_until_esc() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    assert!(drawn_nets(&app).is_empty());

    start_carry(&mut app, &path, CarryKind::Drag, &[U1], &[], &[]);
    assert!(app.interaction_state.pcb_canvas.previews_drag());
    assert_eq!(drawn_nets(&app), [1, 2], "the board under the preview");

    app.clear_pcb_selection();
    assert!(app.interaction_state.pcb_carry.is_none(), "Esc drops it");
    assert!(!app.interaction_state.pcb_canvas.previews_drag());
    assert!(drawn_nets(&app).is_empty(), "and the board comes back");
}

#[test]
fn a_move_of_a_track_and_a_via_keeps_their_nets_bright_until_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    start_carry(&mut app, &path, CarryKind::Move, &[], &[VIA], &[TRACK]);
    assert_eq!(drawn_nets(&app), [2, 3]);

    app.end_pcb_carry();
    assert!(drawn_nets(&app).is_empty());
}

#[test]
fn a_highlighted_net_wins_over_a_tool_in_hand() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.interaction_state.pcb_tool_defaults.via_net = Some(2);
    app.interaction_state.pcb_query.net_highlight =
        Some(crate::app::state::pcb_query::BoardNetHighlight {
            path: path.clone(),
            net: 3,
            name: "3".into(),
        });
    app.arm_board_tool(BoardPlaceTool::Via);
    assert_eq!(drawn_nets(&app), [3]);

    app.interaction_state.pcb_query.net_highlight = None;
    app.refresh_pcb_highlight();
    assert_eq!(
        drawn_nets(&app),
        [2],
        "the via's net once the highlight ends"
    );
    app.end_board_tool();
    assert!(drawn_nets(&app).is_empty());
}
