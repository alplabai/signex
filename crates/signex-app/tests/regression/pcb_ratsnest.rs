//! #2192 — the ratsnest reaches the PCB canvas through `Signex::update`,
//! and a net can be hidden and shown again.

use signex_app::app::{FileMsg, Message, PcbMsg, Signex};
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point};

use crate::drive::drive;

fn pad(n: u128, x: f64) -> Pad {
    Pad {
        uuid: uuid::Uuid::from_u128(n),
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
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn two_pad_board() -> PcbBoard {
    let footprint = Footprint {
        uuid: uuid::Uuid::from_u128(100),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad(1, 0.0), pad(2, 5.0)],
        graphics: vec![],
        properties: vec![],
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![footprint],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

fn drawn_lines(app: &Signex) -> usize {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("snapshot")
        .ratsnest_lines
        .len()
}

#[test]
fn an_opened_board_shows_its_ratsnest_and_a_net_toggles_off_and_on() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join("board.snxpcb");
    let text = signex_types::format::SnxPcb::new(two_pad_board())
        .write_string()
        .unwrap();
    std::fs::write(&doc, text).unwrap();

    let (mut app, _boot) = Signex::new();
    let task = app.update(Message::File(FileMsg::Opened(Some(doc))));
    drive(&mut app, task);
    assert_eq!(drawn_lines(&app), 1);

    let task = app.update(Message::Pcb(PcbMsg::RatsnestNetVisibilityToggled(1)));
    drive(&mut app, task);
    assert_eq!(drawn_lines(&app), 0);

    let task = app.update(Message::Pcb(PcbMsg::RatsnestNetVisibilityToggled(1)));
    drive(&mut app, task);
    assert_eq!(drawn_lines(&app), 1);
}
