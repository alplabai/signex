//! #2407 / #2401: track and via locks, the via span and per-layer sizes
//! through `Signex::update` (the board menu rows, the Properties panel) and
//! through `run_operands` (an agent). Every edit is one board undo step.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_types::format::SnxPcb;
use signex_types::pcb::{LayerSizes, PcbBoard, ViaType};
use signex_types::pcb_layer::{CopperLayer, InnerCopper};
use signex_types::pcb_stack::StackMode;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget};
use crate::panels::{PanelMsg, PcbPrimitiveMsg};

const TRACK: Uuid = Uuid::from_u128(0x101);
const NEAR: Uuid = Uuid::from_u128(0x102);
const VIA: Uuid = Uuid::from_u128(0x201);

/// Four copper layers; a net-1 track along y = 0 on top, a through via of
/// net 1 at (20, 0), and a net-2 track on Inner 2 passing 0.5 mm from it.
fn board() -> PcbBoard {
    let layer =
        |id: u8, name: &str| serde_json::json!({"id": id, "name": name, "layer_type": "signal"});
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [layer(0, "TopCopper"), layer(1, "Inner1Copper"),
                   layer(2, "Inner2Copper"), layer(31, "BottomCopper")],
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
        "segments": [
            {"uuid": TRACK, "start": {"x": 0.0, "y": 0.0}, "end": {"x": 10.0, "y": 0.0},
             "width": 0.25, "layer": "TopCopper", "net": 1},
            {"uuid": NEAR, "start": {"x": 15.0, "y": 0.5}, "end": {"x": 25.0, "y": 0.5},
             "width": 0.25, "layer": "Inner2Copper", "net": 2}
        ],
        "vias": [{"uuid": VIA, "position": {"x": 20.0, "y": 0.0}, "diameter": 0.6,
                  "drill": 0.3, "layers": ["TopCopper", "BottomCopper"], "net": 1}]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state.pcb_board(path).expect("board")
}

fn run(app: &mut Signex, command: &str) {
    let id = AppCommandId::new(command).expect("a catalog id");
    let _ = app.update(Message::Menu(MenuMessage::Command(id)));
}

fn panel(app: &mut Signex, msg: PcbPrimitiveMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(
        msg,
    ))));
}

fn target(path: &Path, item: ItemRef) -> PrimitiveTarget {
    PrimitiveTarget {
        board: path.to_path_buf(),
        items: vec![item],
    }
}

/// Run `command` with `operands` as an agent does; its answer.
fn agent(app: &mut Signex, path: &Path, command: &str, operands: &[&str]) -> serde_json::Value {
    let id = AppCommandId::new(command).unwrap();
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &all).expect("operands");
    match app.run_operands(&id, &args).0 {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("{command} answered nothing: {other:?}"),
    }
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

#[test]
fn a_locked_track_refuses_move_edits_and_delete_until_unlocked() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.select_pcb_items(&[ItemRef::Track(TRACK)]);
    run(&mut app, "lock_board_items");
    assert!(board_of(&app, &path).segments[0].locked);
    let locked = serde_json::to_string(board_of(&app, &path)).unwrap();

    let track = target(&path, ItemRef::Track(TRACK));
    panel(
        &mut app,
        PcbPrimitiveMsg::FieldTyped {
            target: track.clone(),
            field: PrimitiveField::X,
            text: "5".into(),
        },
    );
    panel(
        &mut app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: track,
            field: PrimitiveField::X,
        },
    );
    run(&mut app, "delete_selection");
    assert_eq!(
        serde_json::to_string(board_of(&app, &path)).unwrap(),
        locked,
        "a locked track is neither moved nor deleted"
    );

    undo(&mut app);
    assert!(
        !board_of(&app, &path).segments[0].locked,
        "the lock is one step"
    );
    app.select_pcb_items(&[ItemRef::Track(TRACK)]);
    run(&mut app, "delete_selection");
    assert!(
        board_of(&app, &path)
            .segments
            .iter()
            .all(|s| s.uuid != TRACK),
        "unlocked, it deletes"
    );
}

#[test]
fn the_lock_box_and_an_agent_lock_and_unlock_a_via() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    panel(
        &mut app,
        PcbPrimitiveMsg::LockToggled {
            target: target(&path, ItemRef::Via(VIA)),
            locked: true,
        },
    );
    assert!(board_of(&app, &path).vias[0].locked);
    let ids = format!("ids={VIA}");
    let answer = agent(&mut app, &path, "unlock_board_items", &[ids.as_str()]);
    assert_eq!(answer["locked"], false);
    assert!(!board_of(&app, &path).vias[0].locked);
    let answer = agent(&mut app, &path, "lock_board_items", &[ids.as_str()]);
    assert_eq!(answer["locked"], true);
    assert!(board_of(&app, &path).vias[0].locked);
}

#[test]
fn a_via_span_change_saves_reloads_and_is_the_span_drc_checks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let rules = signex_drc::DrcRules::new(200_000);
    let clearances =
        |b: &PcbBoard| signex_drc::run(b, &rules).count(signex_drc::ViolationKind::Clearance);
    assert_eq!(
        clearances(board_of(&app, &path)),
        1,
        "a through via meets Inner 2"
    );

    let inner1 = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    panel(
        &mut app,
        PcbPrimitiveMsg::SpanPicked {
            target: target(&path, ItemRef::Via(VIA)),
            top: CopperLayer::Top,
            bottom: inner1,
        },
    );
    let via = &board_of(&app, &path).vias[0];
    assert_eq!(via.layers, ["TopCopper", "Inner1Copper"]);
    assert_eq!(via.via_type, ViaType::Blind);
    assert_eq!(
        clearances(board_of(&app, &path)),
        0,
        "the span stops above Inner 2"
    );

    let text = SnxPcb::new(board_of(&app, &path).clone())
        .write_string()
        .unwrap();
    let back = SnxPcb::parse(&text).unwrap().board;
    assert_eq!(back.vias, board_of(&app, &path).vias, "the span reloads");

    let answer = agent(
        &mut app,
        &path,
        "set_via_span",
        &[
            format!("ids={VIA}").as_str(),
            "layers=TopCopper,BottomCopper",
        ],
    );
    assert_eq!(answer["vias"][0]["via_type"], "through");
    undo(&mut app);
    assert_eq!(
        board_of(&app, &path).vias[0].via_type,
        ViaType::Blind,
        "one step"
    );
}

#[test]
fn a_smaller_inner_via_size_through_the_panel_and_an_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    // The panel edits what it shows: the selected via.
    app.select_pcb_items(&[ItemRef::Via(VIA)]);
    let via = target(&path, ItemRef::Via(VIA));
    panel(
        &mut app,
        PcbPrimitiveMsg::StackModePicked {
            target: via.clone(),
            mode: StackMode::TopMiddleBottom,
        },
    );
    let inner1 = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    let field = PrimitiveField::LayerSize(inner1);
    panel(
        &mut app,
        PcbPrimitiveMsg::FieldTyped {
            target: via.clone(),
            field,
            text: "0.4".into(),
        },
    );
    panel(
        &mut app,
        PcbPrimitiveMsg::FieldSubmitted { target: via, field },
    );
    assert_eq!(
        board_of(&app, &path).vias[0].layer_sizes,
        Some(LayerSizes::TopMiddleBottom {
            top: 0.6,
            middle: 0.4,
            bottom: 0.6
        })
    );
    assert_eq!(board_of(&app, &path).vias[0].diameter_on(inner1), 0.4);

    let ids = format!("ids={VIA}");
    agent(
        &mut app,
        &path,
        "set_layer_sizes",
        &[ids.as_str(), "sizes=simple"],
    );
    assert_eq!(board_of(&app, &path).vias[0].layer_sizes, None);
    undo(&mut app);
    assert!(
        board_of(&app, &path).vias[0].layer_sizes.is_some(),
        "one step"
    );
}

/// Type `text` into `field` of `target`'s panel and press Enter.
fn submit(app: &mut Signex, target: &PrimitiveTarget, field: PrimitiveField, text: &str) {
    panel(
        app,
        PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.into(),
        },
    );
    panel(
        app,
        PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        },
    );
}

#[test]
fn a_top_middle_bottom_via_is_as_small_as_its_parts_not_its_left_over_diameter() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    // The panel edits what it shows: the selected via.
    app.select_pcb_items(&[ItemRef::Via(VIA)]);
    let via = target(&path, ItemRef::Via(VIA));
    panel(
        &mut app,
        PcbPrimitiveMsg::StackModePicked {
            target: via.clone(),
            mode: StackMode::TopMiddleBottom,
        },
    );
    let inner1 = CopperLayer::Inner(InnerCopper::new(1).unwrap());
    for layer in [CopperLayer::Top, inner1, CopperLayer::Bottom] {
        submit(&mut app, &via, PrimitiveField::LayerSize(layer), "0.7");
    }
    let b = board_of(&app, &path);
    assert_eq!(
        b.vias[0].layer_sizes,
        Some(LayerSizes::TopMiddleBottom {
            top: 0.7,
            middle: 0.7,
            bottom: 0.7
        })
    );
    assert_eq!(b.vias[0].diameter, 0.6, "the Simple diameter is left over");
    let copper = signex_types::pcb_stack::board_copper_layers(&b.layers);
    assert_eq!(
        b.vias[0].min_diameter(&copper),
        0.7,
        "the hidden diameter is no layer's size"
    );
}

#[test]
fn an_empty_size_list_from_an_agent_is_refused_and_the_board_still_saves() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let command = AppCommandId::new("set_layer_sizes").unwrap();
    let document = format!("document={}", path.display());
    let ids = format!("ids={VIA}");
    let args = CommandArgs::parse_operands(&command, &[document.as_str(), ids.as_str(), "sizes=;"])
        .expect("operands");
    let (outcome, _) = app.run_operands(&command, &args);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "a list naming no layer is refused: {outcome:?}"
    );
    assert_eq!(board_of(&app, &path).vias[0].layer_sizes, None);
    SnxPcb::new(board_of(&app, &path).clone())
        .write_string()
        .expect("the board saves");
}

const FOOTPRINT: Uuid = Uuid::from_u128(0x301);
const PAD: Uuid = Uuid::from_u128(0x302);

/// The board with one footprint holding one 2 x 1 mm rectangular pad.
fn board_with_pad() -> PcbBoard {
    use signex_types::pcb::{Footprint, Pad, PadShape, PadType, Point};
    let mut b = board();
    b.footprints.push(Footprint {
        uuid: FOOTPRINT,
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(30.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![Pad {
            uuid: PAD,
            number: "1".into(),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            position: Point::new(0.0, 0.0),
            size: Point::new(2.0, 1.0),
            drill: None,
            layers: vec!["TopCopper".into()],
            net: None,
            roundrect_ratio: 0.0,
            trapezoid_delta: Point::ZERO,
            custom_outline: Vec::new(),
            rotation: 0.0,
            layer_sizes: None,
            net_locked: false,
            thermal: Default::default(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    b
}

#[test]
fn an_agent_makes_a_pad_a_trapezoid_then_a_custom_pad_in_one_step_each() {
    // #2242: `set_pad_shape` sets the shape with the parameters it reads.
    use signex_types::pcb::{PadShape, Point};
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board_with_pad());
    let ids = format!("ids={PAD}");
    let pad = |app: &Signex| board_of(app, &path).footprints[0].pads[0].clone();

    agent(
        &mut app,
        &path,
        "set_pad_shape",
        &[ids.as_str(), "shape=trapezoid", "delta=1,0"],
    );
    let held = pad(&app);
    assert_eq!(
        (held.shape, held.trapezoid_delta),
        (PadShape::Trapezoid, Point::new(1.0, 0.0))
    );

    agent(
        &mut app,
        &path,
        "set_pad_shape",
        &[
            ids.as_str(),
            "shape=custom",
            "corners=0,0;1,0;1,0.4;0.4,0.4;0.4,1;0,1",
        ],
    );
    assert_eq!(pad(&app).shape, PadShape::Custom);
    assert_eq!(pad(&app).custom_outline.len(), 6);
    let text = SnxPcb::new(board_of(&app, &path).clone())
        .write_string()
        .expect("the board saves");
    assert!(text.contains("custom_outline"), "{text}");
    undo(&mut app);
    assert_eq!(pad(&app).shape, PadShape::Trapezoid, "one step");

    // A custom pad with no outline makes no copper: refused, nothing moves.
    let command = AppCommandId::new("set_pad_shape").unwrap();
    let document = format!("document={}", path.display());
    let args =
        CommandArgs::parse_operands(&command, &[document.as_str(), ids.as_str(), "shape=custom"])
            .expect("operands");
    let (outcome, _) = app.run_operands(&command, &args);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert_eq!(pad(&app).shape, PadShape::Trapezoid);
}

/// #2326: an item named twice is locked once and answered once, in the
/// order first given.
#[test]
fn a_lock_naming_an_item_twice_acts_on_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let ids = format!("ids={VIA},{TRACK},{VIA}");
    let answer = agent(&mut app, &path, "lock_board_items", &[ids.as_str()]);
    assert_eq!(answer["ids"], serde_json::json!([VIA, TRACK]));
    assert!(board_of(&app, &path).vias[0].locked);
}
