//! #2222 - the design-rule check driven through `Message`s on a real board
//! opened from disk: the report, the Messages panel rows, a click on a row
//! selecting its items, and a board that is never edited.

use pcb_geom::{ItemRef, ViolationKind};
use signex_app::app::{FileMsg, Message, PcbMsg, Signex};
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

fn track(n: u128, y: f64, net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Two tracks 0.4 mm apart centre to centre: a 0.15 mm gap, under the
/// default 0.2 mm; a third 5 mm away.
fn board() -> PcbBoard {
    PcbBoard {
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: vec![track(1, 0.0, 1), track(2, 0.4, 2), track(3, 5.0, 3)],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn open_board() -> (Signex, tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    (app, tmp, path)
}

fn run_drc(app: &mut Signex) {
    let task = app.update(Message::Pcb(PcbMsg::DrcRequested));
    crate::drive::drive(app, task);
}

#[test]
fn the_check_lists_the_close_pair_and_clicking_it_selects_both_tracks() {
    let (mut app, _tmp, path) = open_board();
    run_drc(&mut app);

    let done = app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .expect("a report for the open board");
    assert_eq!(done.report.violations.len(), 1, "{:?}", done.report);
    let v = &done.report.violations[0];
    assert_eq!(v.kind, ViolationKind::Clearance);
    assert_eq!((v.measured_nm, v.required_nm), (150_000, 200_000));

    let panel = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .expect("panel");
    assert!(panel.ran && !panel.running && !panel.stale);
    assert_eq!((panel.clearance, panel.short, panel.unconnected), (1, 0, 0));
    assert_eq!(panel.rows.len(), 1);

    let task = app.update(Message::Pcb(PcbMsg::DrcViolationPicked(0)));
    crate::drive::drive(&mut app, task);
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(
        selected,
        vec![
            ItemRef::Track(Uuid::from_u128(1)),
            ItemRef::Track(Uuid::from_u128(2))
        ]
    );
}

#[test]
fn the_check_does_not_edit_the_board() {
    let (mut app, _tmp, path) = open_board();
    let revision = app.document_state.pcb_revisions.get(&path).copied();
    run_drc(&mut app);
    assert_eq!(
        app.document_state.pcb_revisions.get(&path).copied(),
        revision
    );
    assert_eq!(app.document_state.dirty_document_count(), 0);
}

#[test]
fn run_drc_with_no_board_open_runs_nothing() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    run_drc(&mut app);
    assert!(app.document_state.panel_ctx.pcb_drc.is_none());
}
