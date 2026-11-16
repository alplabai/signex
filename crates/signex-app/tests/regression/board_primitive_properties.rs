//! #2355: the Properties panel of a selected track, via or pad, driven
//! through `Signex::update` with the messages the panel sends. Every edit is
//! one board undo step; the net Length readout is the geometry's.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabDocument};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    DrillDef, Footprint, NetDef, Pad, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType,
};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

const T1: Uuid = Uuid::from_u128(0x101);
const T2: Uuid = Uuid::from_u128(0x102);
const T3: Uuid = Uuid::from_u128(0x103);
const V1: Uuid = Uuid::from_u128(0x201);
const FP: Uuid = Uuid::from_u128(0x301);
const PAD: Uuid = Uuid::from_u128(0x302);

fn track(uuid: Uuid, (ax, ay): (f64, f64), (bx, by): (f64, f64), net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid,
        start: Point::new(ax, ay),
        end: Point::new(bx, by),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// Net 1 (GND): T1 (10,10)-(13,14) and T2 (13,14)-(18,14), 5 mm each, and
/// V1 at (40,10). Net 2 (VCC): T3 (30,30)-(31,30). A footprint at (60,10)
/// with one through-hole pad on no net.
fn board() -> PcbBoard {
    let pad = Pad {
        uuid: PAD,
        number: "1".into(),
        pad_type: PadType::Thru,
        shape: PadShape::Circle,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.2, 1.2),
        drill: Some(DrillDef {
            diameter: 0.5,
            shape: String::new(),
        }),
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: None,
        roundrect_ratio: 0.25,
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: FP,
        reference: "J1".into(),
        value: "Conn".into(),
        footprint_id: "Pin".into(),
        position: Point::new(60.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        options: Default::default(),
        uuid: Uuid::from_u128(1),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: vec![
            NetDef {
                number: 1,
                name: "GND".into(),
            },
            NetDef {
                number: 2,
                name: "VCC".into(),
            },
        ],
        footprints: vec![footprint],
        segments: vec![
            track(T1, (10.0, 10.0), (13.0, 14.0), 1),
            track(T2, (13.0, 14.0), (18.0, 14.0), 1),
            track(T3, (30.0, 30.0), (31.0, 30.0), 2),
        ],
        vias: vec![Via {
            attributes: Default::default(),
            uuid: V1,
            position: Point::new(40.0, 10.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            locked: false,
            layer_sizes: None,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    path: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn open_board() -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    std::fs::write(&path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let task = app.update(Message::File(FileMsg::Opened(Some(path.clone()))));
    crate::drive::drive(&mut app, task);
    Fixture {
        app,
        path,
        _tmp: tmp,
    }
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn click(app: &mut Signex, x: f64, y: f64) {
    send(
        app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

fn facts(app: &Signex) -> PcbPrimitiveFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the primitive panel")
}

fn target(f: &Fixture, items: Vec<ItemRef>) -> PrimitiveTarget {
    PrimitiveTarget {
        board: f.path.clone(),
        items,
    }
}

fn panel(msg: PcbPrimitiveMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)))
}

/// Type `text` into `field`, then press Enter.
fn type_and_enter(f: &mut Fixture, target: &PrimitiveTarget, field: PrimitiveField, text: &str) {
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        }),
    );
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        }),
    );
}

/// The edit `run` makes is one undo step: one undo restores the board,
/// redo replays it, and a second undo leaves nothing of it.
fn assert_one_step(f: &mut Fixture, run: impl FnOnce(&mut Fixture)) -> PcbBoard {
    let before = snapshot(&f.app);
    run(f);
    let after = snapshot(&f.app);
    assert_ne!(before, after, "the board did not change");
    let edited = live(&f.app);
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "one undo restores the board");
    send(&mut f.app, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(&f.app), after, "redo replays it");
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    send(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "it was a single step");
    edited
}

fn segment(board: &PcbBoard, id: Uuid) -> &Segment {
    board.segments.iter().find(|s| s.uuid == id).unwrap()
}

/// Select T1 by a click on its middle.
fn select_t1(f: &mut Fixture) -> PrimitiveTarget {
    click(&mut f.app, 11.5, 12.0);
    let t = target(f, vec![ItemRef::Track(T1)]);
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(track.common.target, t);
    t
}

#[test]
fn the_net_length_readout_is_the_tracks_geometry() {
    let mut f = open_board();
    select_t1(&mut f);
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    // T1 is a 3-4-5 run and T2 a straight 5 mm one, both on GND.
    let geometry = |s: &Segment| (s.end.x - s.start.x).hypot(s.end.y - s.start.y);
    let b = live(&f.app);
    let total = geometry(segment(&b, T1)) + geometry(segment(&b, T2));
    let info = &track.common.net_info;
    assert_eq!(info.name.as_deref(), Some("GND"));
    assert_eq!(info.selected_length_mm, Some(geometry(segment(&b, T1))));
    assert_eq!(info.total_length_mm, Some(total));
    assert_eq!(total, 10.0);
    assert_eq!(track.length_mm, Some(5.0));
    assert_eq!((info.tracks, info.vias, info.pads), (2, 1, 0));
    assert_eq!(track.width, Some(0.25));
    assert_eq!(track.start, (Some(10.0), Some(10.0)));
    assert_eq!(track.layer.map(|l| l.layer), Some(CopperLayer::Top));
}

#[test]
fn editing_the_width_is_one_undo_step() {
    let mut f = open_board();
    let t = select_t1(&mut f);
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::FieldTyped {
            target: t.clone(),
            field: PrimitiveField::Width,
            text: "0.4".into(),
        }),
    );
    assert_eq!(
        segment(&live(&f.app), T1).width,
        0.25,
        "typing edits nothing"
    );
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::FieldSubmitted {
                target: t.clone(),
                field: PrimitiveField::Width,
            }),
        );
    });
    assert_eq!(segment(&edited, T1).width, 0.4);
    assert_eq!(segment(&edited, T2).width, 0.25, "only the selected track");
}

#[test]
fn moving_the_start_is_one_undo_step_and_the_end_stays() {
    let mut f = open_board();
    let t = select_t1(&mut f);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::StartX, "9.5mm");
    });
    let s = segment(&edited, T1);
    assert_eq!(s.start, Point::new(9.5, 10.0));
    assert_eq!(s.end, Point::new(13.0, 14.0));
}

#[test]
fn changing_the_layer_is_one_undo_step() {
    let mut f = open_board();
    let t = select_t1(&mut f);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::LayerPicked {
                target: t.clone(),
                layer: CopperLayer::Bottom,
            }),
        );
    });
    assert_eq!(segment(&edited, T1).layer, "BottomCopper");
}

#[test]
fn changing_the_net_is_one_undo_step_and_the_readout_follows() {
    let mut f = open_board();
    let t = select_t1(&mut f);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::NetPicked {
                target: t.clone(),
                net: 2,
            }),
        );
    });
    assert_eq!(segment(&edited, T1).net, 2);
    send(&mut f.app, Message::Edit(EditMsg::Redo));
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(track.common.net_info.name.as_deref(), Some("VCC"));
    // T1 (5 mm) joined T3 (1 mm) on VCC.
    assert_eq!(track.common.net_info.total_length_mm, Some(6.0));
}

#[test]
fn a_net_picked_on_the_board_is_one_undo_step() {
    let mut f = open_board();
    let t = select_t1(&mut f);
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::NetPickStarted { target: t.clone() }),
    );
    let edited = assert_one_step(&mut f, |f| click(&mut f.app, 30.5, 30.0));
    assert_eq!(segment(&edited, T1).net, 2, "T3's net");
    let selected: Vec<ItemRef> = f
        .app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(
        selected,
        vec![ItemRef::Track(T1)],
        "the pick did not select"
    );
}

#[test]
fn a_mixed_selection_leaves_differing_values_blank() {
    let mut f = open_board();
    click(&mut f.app, 11.5, 12.0);
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(30.5, 30.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Toggle,
        }),
    );
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(track.common.target.items.len(), 2);
    assert_eq!(track.width, Some(0.25), "a shared value shows");
    assert_eq!(track.start, (None, None), "differing values are blank");
    assert_eq!(track.common.net_info.net, None);
    assert_eq!(track.location, Some((10.0, 10.0)));
}

#[test]
fn via_diameter_hole_and_net_are_one_undo_step_each() {
    let mut f = open_board();
    click(&mut f.app, 40.0, 10.0);
    let t = target(&f, vec![ItemRef::Via(V1)]);
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    // The layer numbers are Fluent placeables, inside isolation marks.
    let span = via.span.map(|s| s.replace(['\u{2068}', '\u{2069}'], ""));
    assert_eq!(span.as_deref(), Some("Thru 1:2"));
    assert_eq!((via.diameter, via.hole), (Some(0.6), Some(0.3)));
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::Diameter, "0.8");
    });
    assert_eq!(edited.vias[0].diameter, 0.8);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::HoleSize, "0.4");
    });
    assert_eq!(edited.vias[0].drill, 0.4);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::NetPicked {
                target: t.clone(),
                net: 2,
            }),
        );
    });
    assert_eq!(edited.vias[0].net, 2);
}

#[test]
fn a_hole_as_big_as_the_via_is_refused_and_changes_nothing() {
    let mut f = open_board();
    click(&mut f.app, 40.0, 10.0);
    let t = target(&f, vec![ItemRef::Via(V1)]);
    let before = snapshot(&f.app);
    type_and_enter(&mut f, &t, PrimitiveField::HoleSize, "0.6");
    assert_eq!(snapshot(&f.app), before);
}

#[test]
fn pad_size_shape_hole_and_net_are_one_undo_step_each() {
    let mut f = open_board();
    click(&mut f.app, 60.0, 10.0);
    let t = target(
        &f,
        vec![ItemRef::Pad {
            footprint: FP,
            pad: PAD,
        }],
    );
    let PcbPrimitiveFacts::Pad(pad) = facts(&f.app) else {
        panic!("the pad panel shows");
    };
    assert_eq!((pad.x, pad.y), (Some(60.0), Some(10.0)));
    assert_eq!(pad.hole, Some(0.5));
    let pad_of = |b: &PcbBoard| b.footprints[0].pads[0].clone();
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::SizeX, "1.6");
    });
    assert_eq!(pad_of(&edited).size, Point::new(1.6, 1.2));
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::ShapePicked {
                target: t.clone(),
                shape: PadShape::Rect,
            }),
        );
    });
    assert_eq!(pad_of(&edited).shape, PadShape::Rect);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::HoleSize, "0.7");
    });
    assert_eq!(pad_of(&edited).drill.map(|d| d.diameter), Some(0.7));
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            panel(PcbPrimitiveMsg::NetPicked {
                target: t.clone(),
                net: 1,
            }),
        );
    });
    let net = pad_of(&edited).net.expect("a net");
    assert_eq!((net.number, net.name.as_str()), (1, "GND"));
}

// ─── #2378: lock and mask settings stored per track and via ─────────────

/// The Lock box locks the track in one undo step; a locked track refuses
/// its other fields and shows the locked note; unlocking is one step too.
#[test]
fn a_locked_track_takes_only_its_unlock() {
    use signex_app::panels::pcb_primitive_properties::{Editability, PrimitiveFlag};
    let mut f = open_board();
    let t = select_t1(&mut f);
    let toggle = |on: bool| {
        panel(PcbPrimitiveMsg::FlagToggled {
            target: t.clone(),
            flag: PrimitiveFlag::Locked,
            on,
        })
    };
    let edited = assert_one_step(&mut f, |f| send(&mut f.app, toggle(true)));
    assert!(segment(&edited, T1).locked);
    send(&mut f.app, toggle(true));
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(track.locked, Some(true));
    assert_eq!(track.common.editability, Editability::ItemLocked);
    let before = snapshot(&f.app);
    type_and_enter(&mut f, &t, PrimitiveField::Width, "0.5");
    assert_eq!(snapshot(&f.app), before, "a locked track refuses the width");
    send(&mut f.app, toggle(false));
    assert!(!segment(&live(&f.app), T1).locked);
}

/// A typed solder mask expansion is a manual one, one undo step; Rule puts
/// the rule back.
#[test]
fn a_track_mask_expansion_is_stored_and_rule_restores_it() {
    use signex_types::pcb::MaskExpansion;
    let mut f = open_board();
    let t = select_t1(&mut f);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::SolderMask, "0.05");
    });
    assert_eq!(
        segment(&edited, T1).attributes.solder_mask,
        MaskExpansion::Manual { mm: 0.05 }
    );
    type_and_enter(&mut f, &t, PrimitiveField::PasteMask, "-0.02");
    let PcbPrimitiveFacts::Track(track) = facts(&f.app) else {
        panic!("the track panel shows");
    };
    assert_eq!(track.paste_mask, Some(MaskExpansion::Manual { mm: -0.02 }));
    send(
        &mut f.app,
        panel(PcbPrimitiveMsg::MaskModePicked {
            target: t.clone(),
            field: PrimitiveField::PasteMask,
            manual: false,
        }),
    );
    assert_eq!(
        segment(&live(&f.app), T1).attributes.paste_mask,
        MaskExpansion::Rule
    );
}

/// Tenting, a side's expansion and From Hole Edge are stored on the via,
/// each one undo step, and survive a save and a reload of the file.
#[test]
fn via_tenting_and_masks_are_one_step_each_and_survive_the_file() {
    use signex_app::panels::pcb_primitive_properties::PrimitiveFlag;
    use signex_types::pcb::MaskExpansion;
    let mut f = open_board();
    click(&mut f.app, 40.0, 10.0);
    let t = target(&f, vec![ItemRef::Via(V1)]);
    let flag = |flag: PrimitiveFlag| {
        panel(PcbPrimitiveMsg::FlagToggled {
            target: t.clone(),
            flag,
            on: true,
        })
    };
    let edited = assert_one_step(&mut f, |f| {
        send(&mut f.app, flag(PrimitiveFlag::TentedBottom));
    });
    assert!(edited.vias[0].attributes.bottom.tented);
    let edited = assert_one_step(&mut f, |f| {
        type_and_enter(f, &t, PrimitiveField::SolderMaskTop, "0.1");
    });
    assert_eq!(
        edited.vias[0].attributes.top.expansion,
        MaskExpansion::Manual { mm: 0.1 }
    );
    send(&mut f.app, flag(PrimitiveFlag::TentedBottom));
    type_and_enter(&mut f, &t, PrimitiveField::SolderMaskTop, "0.1");
    send(&mut f.app, flag(PrimitiveFlag::FromHoleEdge));
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    assert_eq!(via.bottom.tented, Some(true));
    assert_eq!(via.from_hole_edge, Some(true));
    let saved = SnxPcb::new(live(&f.app)).write_string().unwrap();
    let back = SnxPcb::parse(&saved).unwrap().board;
    assert_eq!(back.vias[0].attributes, live(&f.app).vias[0].attributes);
}
