//! #2240 / #2223 - the board canvas menu's View group: its Toggle Ratsnest
//! row hides and shows the ratsnest of a board opened from disk.

use signex_app::app::{ContextAction, ContextMenuMsg, FileMsg, Message, Signex};
use signex_app::keymap::AppCommandId;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, LayerDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point};
use uuid::Uuid;

use crate::drive::drive;

fn pad(n: u128, x: f64) -> Pad {
    Pad {
        uuid: Uuid::from_u128(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "N1".into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// Two unrouted pads of one net: one ratsnest line.
fn board() -> PcbBoard {
    let footprint = Footprint {
        uuid: Uuid::from_u128(100),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad(1, 0.0), pad(2, 8.0)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![LayerDef {
            id: 0,
            name: "TopCopper".to_owned(),
            layer_type: "signal".to_owned(),
            display_name: String::new(),
        }],
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn ratsnest_lines(app: &Signex) -> usize {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .ratsnest_lines
        .len()
}

/// The message the board menu's Toggle Ratsnest row sends.
fn press_toggle_ratsnest(app: &mut Signex) {
    let row = Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
        id: AppCommandId::new("toggle_ratsnest").unwrap(),
        project: None,
    }));
    let task = app.update(row);
    drive(app, task);
}

#[test]
fn the_board_menu_row_toggles_the_ratsnest() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let saved = std::fs::read(&path).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    drive(&mut app, task);
    assert_eq!(ratsnest_lines(&app), 1);

    press_toggle_ratsnest(&mut app);
    assert_eq!(ratsnest_lines(&app), 0);
    press_toggle_ratsnest(&mut app);
    assert_eq!(ratsnest_lines(&app), 1);
    // A view change, never a file change.
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}
