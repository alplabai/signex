//! #88 / AV-13: a DRC finding waived through the panel's waiver form keeps
//! its multi-line reason and its author through a save and a reload.

use signex_app::app::{FileMsg, Message, PcbMsg, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{DrcPanelMsg, PanelMsg};
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

const REASON: &str = "Checked on the prototype\nby the layout review";

fn track(n: u128, y: f64, net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Two tracks 0.15 mm apart edge to edge, under the default 0.2 mm.
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: vec![track(1, 0.0, 1), track(2, 0.4, 2)],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn panel(app: &mut Signex, msg: DrcPanelMsg) {
    send(app, Message::Dock(DockMessage::Panel(PanelMsg::Drc(msg))));
}

#[test]
fn a_drc_waiver_keeps_its_reason_and_author_through_save_and_reload() {
    let _serial = crate::pcb_drc::drc_serial();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    send(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    send(&mut app, Message::Pcb(PcbMsg::DrcRequested));
    let key = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .and_then(|drc| drc.rows.first())
        .map(|row| row.key.clone())
        .expect("the close pair is a finding");

    panel(&mut app, DrcPanelMsg::WaiveStarted(key.clone()));
    panel(
        &mut app,
        DrcPanelMsg::ReasonActed(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Paste(std::sync::Arc::new(REASON.to_string())),
        )),
    );
    panel(&mut app, DrcPanelMsg::AuthorEdited("A. Designer".into()));
    panel(&mut app, DrcPanelMsg::WaiveConfirmed);
    send(&mut app, Message::File(FileMsg::Save));

    let saved = SnxPcb::parse(&std::fs::read_to_string(&path).unwrap())
        .expect("the saved board reads back")
        .board;
    let waiver = saved
        .options
        .drc_waivers
        .iter()
        .find(|w| w.key == key)
        .expect("the waiver is on disk");
    assert_eq!(waiver.note, REASON);
    assert_eq!(waiver.author, "A. Designer");

    // A fresh session opens the saved board and lists the row as waived.
    let (mut again, _boot) = Signex::new();
    again.ui_state.first_run_tour_open = false;
    send(
        &mut again,
        Message::File(FileMsg::Opened(Some(path.clone()))),
    );
    send(&mut again, Message::Pcb(PcbMsg::DrcRequested));
    let row = again
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .and_then(|drc| drc.rows.iter().find(|row| row.key == key).cloned())
        .expect("the finding is reported again");
    assert_eq!(row.waiver.as_deref(), Some(REASON));
}
