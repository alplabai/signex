//! Mockup rows 96 and 97: the Via panel picks one of the board's via types
//! (derived from its vias until the Layer Stack Manager stores them),
//! a value typed away from the type's is that via's own, and a surface pad's
//! stack is its one copper layer. Driven through `Signex::update` with the
//! panel's messages.

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{EditMsg, FileMsg, Message, Signex};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget, ViaMsg,
};
use signex_app::panels::{PanelMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::test_adapter::BoardEngines;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    DrillDef, Footprint, LayerDef, NetDef, Pad, PadShape, PadType, PcbBoard, Point, Via, ViaType,
};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::StackMode;
use uuid::Uuid;

const V1: Uuid = Uuid::from_u128(0x201);
const V2: Uuid = Uuid::from_u128(0x202);
const V3: Uuid = Uuid::from_u128(0x203);
const V4: Uuid = Uuid::from_u128(0x204);
const FP: Uuid = Uuid::from_u128(0x301);
const TH: Uuid = Uuid::from_u128(0x302);
const SMD: Uuid = Uuid::from_u128(0x303);

fn via(uuid: Uuid, x: f64, kind: ViaType, layers: [&str; 2], size: (f64, f64)) -> Via {
    Via {
        attributes: Default::default(),
        uuid,
        position: Point::new(x, 10.0),
        diameter: size.0,
        drill: size.1,
        layers: layers.iter().map(|l| (*l).into()).collect(),
        net: 1,
        locked: false,
        layer_sizes: None,
        via_type: kind,
        unknown_columns: Default::default(),
    }
}

fn pad(uuid: Uuid, number: &str, x: f64, smd: bool) -> Pad {
    Pad {
        uuid,
        number: number.into(),
        pad_type: if smd { PadType::Smd } else { PadType::Thru },
        shape: if smd {
            PadShape::Rect
        } else {
            PadShape::Circle
        },
        position: Point::new(x, 0.0),
        size: if smd {
            Point::new(1.0, 0.6)
        } else {
            Point::new(1.2, 1.2)
        },
        drill: (!smd).then(|| DrillDef {
            diameter: 0.5,
            shape: String::new(),
        }),
        layers: if smd {
            vec!["TopCopper".into()]
        } else {
            vec!["TopCopper".into(), "BottomCopper".into()]
        },
        net: None,
        roundrect_ratio: 0.25,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// Four copper layers. Through vias V1, V2 (0.6/0.3) and V4 (0.8/0.3, an
/// override of the through type) and a blind Top–Inner 1 via V3
/// (0.3/0.15) tented on top. Footprint U1 at (60,10): a through-hole pad
/// 1 at its origin and a surface pad 2 at (65,10).
fn board() -> PcbBoard {
    let footprint = Footprint {
        uuid: FP,
        reference: "U1".into(),
        value: "Part".into(),
        footprint_id: "Pkg".into(),
        position: Point::new(60.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad(TH, "1", 0.0, false), pad(SMD, "2", 5.0, true)],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let mut blind = via(
        V3,
        52.0,
        ViaType::Blind,
        ["TopCopper", "Inner1Copper"],
        (0.3, 0.15),
    );
    blind.attributes.top.tented = true;
    let through = ["TopCopper", "BottomCopper"];
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::from_u128(1),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
            .iter()
            .enumerate()
            .map(|(i, name)| LayerDef {
                id: i as u8,
                name: (*name).into(),
                layer_type: "signal".into(),
                display_name: String::new(),
            })
            .collect(),
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "GND".into(),
        }],
        footprints: vec![footprint],
        segments: Vec::new(),
        vias: vec![
            via(V1, 40.0, ViaType::Through, through, (0.6, 0.3)),
            via(V2, 44.0, ViaType::Through, through, (0.6, 0.3)),
            blind,
            via(V4, 48.0, ViaType::Through, through, (0.8, 0.3)),
        ],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
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
    docs.tab_board_engine(docs.active_tab)
        .map(|engine| engine.board())
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
    app.document_view
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.primitive.clone())
        .expect("the primitive panel")
}

fn via_panel(msg: ViaMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(
        PcbPrimitiveMsg::Via(msg),
    )))
}

fn select_via(f: &mut Fixture, x: f64, id: Uuid) -> PrimitiveTarget {
    click(&mut f.app, x, 10.0);
    let target = PrimitiveTarget {
        board: f.path.clone(),
        items: vec![ItemRef::Via(id)],
    };
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    assert_eq!(via.common.target, target);
    target
}

fn via_on(board: &PcbBoard, id: Uuid) -> &Via {
    board.vias.iter().find(|v| v.uuid == id).expect("the via")
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

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_via_lists_the_boards_types_and_names_its_own() {
    let mut f = open_board();
    select_via(&mut f, 40.0, V1);
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    let names: Vec<String> = via.types.iter().map(|t| plain(&t.name)).collect();
    assert_eq!(names, ["Thru 1:4", "Blind 1:2"], "through types first");
    let own = via.via_type.expect("V1 is the through type");
    assert_eq!(own.kind, ViaType::Through);
    assert_eq!(plain(&own.spec), "Through 1:4 · 0.6/0.3 · open");
    assert_eq!(via.type_size, Some((0.6, 0.3)), "two of three through vias");
    assert_eq!(via.overridden, Some(false));
    assert_eq!(
        via.drill_pair.as_deref().map(plain).as_deref(),
        Some("Through 1–4")
    );
    assert_eq!(via.reaches, (true, true));
    assert_eq!(via.finished_hole.map(|h| (h * 1000.0).round()), Some(250.0));
}

#[test]
fn picking_a_via_type_is_one_undo_step_and_sets_span_sizes_and_tenting() {
    let mut f = open_board();
    let t = select_via(&mut f, 40.0, V1);
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    let blind = via
        .types
        .iter()
        .find(|c| c.kind == ViaType::Blind)
        .expect("the blind type")
        .key;
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            via_panel(ViaMsg::TypePicked {
                target: t.clone(),
                key: blind,
            }),
        );
    });
    let v = via_on(&edited, V1);
    assert_eq!(v.via_type, ViaType::Blind);
    assert_eq!(v.layers, ["TopCopper", "Inner1Copper"]);
    assert_eq!((v.diameter, v.drill), (0.3, 0.15));
    assert!(v.attributes.top.tented && !v.attributes.bottom.tented);
    assert_eq!(v.net, 1, "the net stays");
}

fn type_value(f: &mut Fixture, t: &PrimitiveTarget, field: PrimitiveField, value: &str) {
    let msg = |m| Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(m)));
    send(
        &mut f.app,
        msg(PcbPrimitiveMsg::FieldTyped {
            target: t.clone(),
            field,
            text: value.into(),
        }),
    );
    send(
        &mut f.app,
        msg(PcbPrimitiveMsg::FieldSubmitted {
            target: t.clone(),
            field,
        }),
    );
}

fn overridden(f: &Fixture) -> Option<bool> {
    let PcbPrimitiveFacts::Via(via) = facts(&f.app) else {
        panic!("the via panel shows");
    };
    via.overridden
}

#[test]
fn typing_a_size_overrides_the_type_in_one_step_and_its_value_clears_it() {
    let mut f = open_board();
    let t = select_via(&mut f, 40.0, V1);
    assert_eq!(overridden(&f), Some(false), "V1 is its type");
    let edited = assert_one_step(&mut f, |f| {
        type_value(f, &t, PrimitiveField::Diameter, "0.7");
    });
    assert_eq!(via_on(&edited, V1).diameter, 0.7);

    type_value(&mut f, &t, PrimitiveField::Diameter, "0.7");
    assert_eq!(overridden(&f), Some(true), "its own diameter");
    type_value(&mut f, &t, PrimitiveField::Diameter, "0.6");
    assert_eq!(via_on(&live(&f.app), V1).diameter, 0.6);
    assert_eq!(overridden(&f), Some(false), "the type's value clears it");

    type_value(&mut f, &t, PrimitiveField::HoleSize, "0.35");
    assert_eq!(overridden(&f), Some(true), "its own drill");
    type_value(&mut f, &t, PrimitiveField::HoleSize, "0.3");
    assert_eq!(overridden(&f), Some(false));
}

#[test]
fn a_finished_hole_sets_the_drill_that_leaves_it_in_one_step() {
    let mut f = open_board();
    let t = select_via(&mut f, 40.0, V1);
    let edited = assert_one_step(&mut f, |f| {
        type_value(f, &t, PrimitiveField::FinishedHole, "0.3");
    });
    assert_eq!(via_on(&edited, V1).drill, 0.35, "0.3 plus the plating");

    type_value(&mut f, &t, PrimitiveField::FinishedHole, "0.3");
    assert_eq!(overridden(&f), Some(true));
    type_value(&mut f, &t, PrimitiveField::FinishedHole, "0.25");
    assert_eq!(via_on(&live(&f.app), V1).drill, 0.3);
    assert_eq!(overridden(&f), Some(false), "back on the type's drill");

    let before = snapshot(&f.app);
    type_value(&mut f, &t, PrimitiveField::FinishedHole, "0");
    assert_eq!(snapshot(&f.app), before, "no hole is refused");
}

#[test]
fn a_locked_via_refuses_a_typed_size() {
    let mut f = open_board();
    let t = select_via(&mut f, 40.0, V1);
    send(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(
            PcbPrimitiveMsg::LockToggled {
                target: t.clone(),
                locked: true,
            },
        ))),
    );
    let before = snapshot(&f.app);
    for field in [
        PrimitiveField::Diameter,
        PrimitiveField::HoleSize,
        PrimitiveField::FinishedHole,
    ] {
        type_value(&mut f, &t, field, "0.2");
        assert_eq!(snapshot(&f.app), before, "{field:?} stays");
    }
}

#[test]
fn a_reset_arrow_puts_one_size_back_in_one_step() {
    let mut f = open_board();
    let t = select_via(&mut f, 48.0, V4);
    let edited = assert_one_step(&mut f, |f| {
        send(
            &mut f.app,
            via_panel(ViaMsg::SizeReset {
                target: t.clone(),
                field: PrimitiveField::Diameter,
            }),
        );
    });
    assert_eq!(via_on(&edited, V4).diameter, 0.6);
}

#[test]
fn a_surface_pad_is_one_copper_row_on_its_layer() {
    let mut f = open_board();
    click(&mut f.app, 65.0, 10.0);
    let PcbPrimitiveFacts::Pad(pad) = facts(&f.app) else {
        panic!("the pad panel shows");
    };
    assert!(!pad.has_hole, "no hole row, no stack tiles");
    assert_eq!(pad.mode, Some(StackMode::Simple));
    assert!(pad.layer_sizes.is_empty(), "no Middle or Bottom rows");
    assert_eq!(
        pad.layer.as_ref().and_then(|(_, l)| *l),
        Some(PcbLayer::Copper(CopperLayer::Top))
    );
    assert_eq!(pad.plated, None);
    assert_eq!(pad.component_designator.as_deref(), Some("U1"));
    assert_eq!(pad.designator.as_deref(), Some("2"));
}

#[test]
fn a_through_hole_pad_keeps_its_stack() {
    let mut f = open_board();
    click(&mut f.app, 60.0, 10.0);
    let PcbPrimitiveFacts::Pad(pad) = facts(&f.app) else {
        panic!("the pad panel shows");
    };
    assert!(pad.has_hole, "the stack tiles and the Hole row show");
    assert_eq!(pad.plated, Some(true));
    assert_eq!(
        pad.layer.as_ref().and_then(|(_, l)| *l),
        None,
        "Multi-Layer"
    );
    assert_eq!(pad.component_designator.as_deref(), Some("U1"));
}
