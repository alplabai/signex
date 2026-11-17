//! #2403, #2405, #2406 through the app: Measure, Find and Replace, Find
//! Similar, Select and Highlight Net and the Net Inspector on a board, the
//! board menu's rows, the read-only board, and the board keys. Each feature
//! is driven once through `Signex::update` and once through its agent query.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use pcb_geom::{ClickMode, ItemRef};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::command::queries::tests::answer;
use crate::app::command::{CommandArgs, CommandOutcome, is_enabled};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::selection_request::SelectionRequest;
use crate::app::{ContextAction, ContextMenuMsg, Message, OverlayMsg, Signex, UiMsg};
use crate::find_replace::FindReplaceMsg;
use crate::find_similar::FindSimilarMsg;
use crate::keymap::{AppCommandId, KeyStroke, ShortcutBindingAction, ShortcutContext};
use crate::panels::{NetInspectorMsg, PanelKind, PanelMsg};
use crate::pcb_canvas::{PcbCanvasMsg, SnapScope};

const R1: Uuid = Uuid::from_u128(0x10);
const R2: Uuid = Uuid::from_u128(0x20);
const C1: Uuid = Uuid::from_u128(0x30);
const GND_TRACK: Uuid = Uuid::from_u128(0x40);
const V1: Uuid = Uuid::from_u128(0x51);
const V2: Uuid = Uuid::from_u128(0x52);

/// R1 (10k) at (10, 10) and R2 (10k) at (20, 10), each with pad 1 on GND
/// one mm above its origin and pad 2 on VCC one mm below; C1 (100n) at
/// (30, 10). A 10 mm track joins the GND pads; VCC is not routed. Two vias
/// on no net, 0.6 mm across, sit 5 mm apart at y = 30.
fn board() -> PcbBoard {
    let pad = |n: u128, y: f64, net: u32, name: &str| {
        serde_json::json!({
            "uuid": Uuid::from_u128(n), "number": if y < 0.0 { "1" } else { "2" },
            "pad_type": "smd", "shape": "rect", "position": {"x": 0.0, "y": y},
            "size": {"x": 1.0, "y": 1.0}, "drill": null, "layers": ["TopCopper"],
            "net": {"number": net, "name": name}
        })
    };
    let fp = |id: Uuid, reference: &str, value: &str, x: f64, pads: serde_json::Value| {
        serde_json::json!({
            "uuid": id, "reference": reference, "value": value, "footprint_id": "R_0603",
            "position": {"x": x, "y": 10.0}, "rotation": 0.0, "layer": "TopCopper",
            "pads": pads
        })
    };
    let via = |id: Uuid, x: f64| {
        serde_json::json!({"uuid": id, "position": {"x": x, "y": 30.0},
                           "diameter": 0.6, "drill": 0.3, "net": 0})
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 0, "name": ""}, {"number": 1, "name": "GND"},
                 {"number": 2, "name": "VCC"}],
        "footprints": [
            fp(R1, "R1", "10k", 10.0,
               serde_json::json!([pad(0x11, -1.0, 1, "GND"), pad(0x12, 1.0, 2, "VCC")])),
            fp(R2, "R2", "10k", 20.0,
               serde_json::json!([pad(0x21, -1.0, 1, "GND"), pad(0x22, 1.0, 2, "VCC")])),
            fp(C1, "C1", "100n", 30.0, serde_json::json!([]))
        ],
        "segments": [{"uuid": GND_TRACK, "start": {"x": 10.0, "y": 9.0},
                      "end": {"x": 20.0, "y": 9.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 1}],
        "vias": [via(V1, 0.0), via(V2, 5.0)]
    }))
    .unwrap()
}

/// The board open in its own tab, nothing snapping, the Classic keys.
fn app_with(dir: &Path, board: PcbBoard) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state
        .keymap_profiles
        .set_active_profile("classic")
        .expect("classic profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app.ui_state.pcb_snap.to_grids = false;
    app.ui_state.pcb_snap.scope = SnapScope::Off;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path)
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn run(app: &mut Signex, id: &str) {
    let _ = app.dispatch_command(&command(id), CommandArgs::none(), None);
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    }));
}

fn selected(app: &Signex) -> Vec<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn footprint(app: &Signex, path: &Path, id: Uuid) -> signex_types::pcb::Footprint {
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id)
        .unwrap()
        .clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn document(path: &Path) -> String {
    format!("document={}", path.display())
}

#[test]
fn measure_on_a_board_reads_point_to_point_and_edge_to_edge() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    assert!(is_enabled(
        &command("measure_distance"),
        &app.command_context(None)
    ));
    run(&mut app, "measure_distance");
    assert!(app.interaction_state.pcb_query.measure.is_some(), "armed");
    // From via V1's centre to V2's: 5 mm apart, 4.4 mm between the rings.
    click(&mut app, 0.0, 30.0);
    click(&mut app, 5.0, 30.0);
    let measure = app.interaction_state.pcb_query.measure.as_ref().unwrap();
    let reading = measure.result.expect("a reading");
    assert!(close(reading.span.distance_mm, 5.0), "{reading:?}");
    let gap = reading.gap.expect("both clicks were on vias");
    assert!(close(gap.distance_mm, 4.4), "{gap:?}");
    // The ruler runs edge to edge.
    let (from, to) = app
        .interaction_state
        .pcb_canvas
        .highlight()
        .measure
        .expect("a ruler");
    assert!(close(from.x, 0.3) && close(to.x, 4.7), "{from:?} {to:?}");
    // What Esc runs on a board: the reading stays until Measure ends.
    app.clear_pcb_selection();
    assert!(app.interaction_state.pcb_query.measure.is_none(), "ended");
    assert!(
        app.interaction_state
            .pcb_canvas
            .highlight()
            .measure
            .is_none()
    );

    // The agent's form reads the same numbers.
    let points = answer(
        &mut app,
        "measure_board",
        &[document(&path).as_str(), "start=0,30", "end=3,34"],
    );
    assert_eq!(points["kind"], "point_to_point");
    assert!(close(points["distance_mm"].as_f64().unwrap(), 5.0));
    let ids = format!("ids={V1},{V2}");
    let edges = answer(
        &mut app,
        "measure_board",
        &[document(&path).as_str(), ids.as_str()],
    );
    assert_eq!(edges["kind"], "edge_to_edge");
    assert!(close(edges["distance_mm"].as_f64().unwrap(), 4.4));
}

#[test]
fn measuring_upward_reads_a_positive_dy_from_the_board_origin() {
    let dir = tempfile::tempdir().unwrap();
    let mut with_origin = board();
    with_origin.options.origin =
        signex_types::pcb_options::BoardOrigin::new(Point::new(0.0, 80.0));
    let (mut app, path) = app_with(dir.path(), with_origin);
    run(&mut app, "measure_distance");
    // 5 mm up the screen: model y falls from 50 to 45.
    click(&mut app, 40.0, 50.0);
    click(&mut app, 40.0, 45.0);
    let measure = app.interaction_state.pcb_query.measure.as_ref().unwrap();
    let span = measure.result.expect("a reading").span;
    assert!(close(span.dy_mm, 5.0), "{span:?}");
    assert!(close(span.dx_mm, 0.0), "{span:?}");

    // The agent types and reads points from the origin, Y up.
    let points = answer(
        &mut app,
        "measure_board",
        &[document(&path).as_str(), "start=10,10", "end=10,15"],
    );
    assert!(close(points["dy_mm"].as_f64().unwrap(), 5.0), "{points}");
    assert!(close(points["dx_mm"].as_f64().unwrap(), 0.0), "{points}");
    assert_eq!(points["from"], serde_json::json!([10.0, 10.0]), "{points}");
    assert_eq!(points["to"], serde_json::json!([10.0, 15.0]), "{points}");
}

#[test]
fn find_on_a_board_lists_values_and_nets_and_replace_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    let _ = app.update(Message::Overlay(OverlayMsg::OpenReplace));
    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::QueryChanged(
        "gnd".into(),
    )));
    let find = &app.ui_state.find_replace;
    assert_eq!(find.result_count(), 1, "the net GND");
    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::SelectResult(0)));
    // The net's track and its two pads.
    assert_eq!(selected(&app).len(), 3, "{:?}", selected(&app));

    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::QueryChanged(
        "10K".into(),
    )));
    assert_eq!(app.ui_state.find_replace.result_count(), 2, "both values");
    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::ReplacementChanged(
        "4k7".into(),
    )));
    let _ = app.update(Message::FindReplaceMsg(FindReplaceMsg::ReplaceAll));
    assert_eq!(footprint(&app, &path, R1).value, "4k7");
    assert_eq!(footprint(&app, &path, R2).value, "4k7");
    // One step takes both back.
    run(&mut app, "undo");
    assert_eq!(footprint(&app, &path, R1).value, "10k");
    assert_eq!(footprint(&app, &path, R2).value, "10k");

    // The agent finds footprints by value with `find_items`.
    let found = answer(
        &mut app,
        "find_items",
        &[document(&path).as_str(), "kind=footprint", "value=10k"],
    );
    assert_eq!(found["count"], 2);
}

#[test]
fn find_similar_on_a_board_selects_the_footprints_of_the_same_value() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    run(&mut app, "find_similar_objects");
    let state = app
        .ui_state
        .find_similar
        .as_ref()
        .expect("the dialog opened");
    assert!(state.board);
    assert_eq!(state.seed_label, "Footprint R1");
    let similar = |msg| Message::Selection(SelectionRequest::FindSimilar(msg));
    let _ = app.update(similar(FindSimilarMsg::PropertySet(
        signex_engine::SimilarProperty::Value,
        true,
    )));
    let _ = app.update(similar(FindSimilarMsg::ApplyPressed));
    assert!(app.ui_state.find_similar.is_none(), "OK closes it");
    let footprints = app.interaction_state.pcb_selection.selection().footprints();
    assert_eq!(footprints.into_iter().collect::<Vec<_>>(), [R1, R2]);

    let id = format!("id={R1}");
    let found = answer(
        &mut app,
        "find_similar_items",
        &[document(&path).as_str(), id.as_str(), "fields=kind,value"],
    );
    assert_eq!(found["count"], 2);
    assert_eq!(found["seed"], "Footprint R1");
}

#[test]
fn select_and_highlight_net_act_on_the_net_of_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    app.select_pcb_items(&[ItemRef::Track(GND_TRACK)]);
    run(&mut app, "select_board_net");
    assert_eq!(selected(&app).len(), 3, "the track and the two GND pads");

    run(&mut app, "highlight_board_net");
    let lit = app.interaction_state.pcb_query.net_highlight.clone();
    assert_eq!(lit.map(|h| (h.net, h.name)), Some((1, "GND".to_string())));
    assert_eq!(app.interaction_state.pcb_canvas.highlight().net.len(), 3);
    // Again on the same net: no longer bright.
    run(&mut app, "highlight_board_net");
    assert!(app.interaction_state.pcb_query.net_highlight.is_none());
    assert!(app.interaction_state.pcb_canvas.highlight().net.is_empty());
    assert_eq!(
        app.document_state.pcb_board(&path).unwrap().segments.len(),
        1
    );
}

const GND_ARC: Uuid = Uuid::from_u128(0x60);
const GND_FREE_PAD: Uuid = Uuid::from_u128(0x61);

/// The board with a copper arc on GND, a half turn of radius 5 about
/// (15, 20), and a free pad on GND at (40, 10).
fn board_with_free_copper() -> PcbBoard {
    let mut b = board();
    b.free.arcs.push(signex_types::pcb_arc::TrackArc {
        uuid: GND_ARC,
        start: Point::new(10.0, 20.0),
        mid: Point::new(15.0, 25.0),
        end: Point::new(20.0, 20.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
    });
    b.free.pads.push(
        serde_json::from_value(serde_json::json!({
            "uuid": GND_FREE_PAD, "number": "1", "pad_type": "smd", "shape": "rect",
            "position": {"x": 40.0, "y": 10.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": 1, "name": "GND"}
        }))
        .unwrap(),
    );
    b
}

fn net_message(msg: crate::app::BoardNetMsg) -> Message {
    Message::Pcb(crate::app::PcbMsg::Net(msg))
}

/// #2440: Select Net takes the net's copper arc as one item and its free
/// pad, one selection Delete acts on.
#[test]
fn select_net_takes_the_copper_arcs_and_free_pads_of_the_net() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board_with_free_copper());
    let select = crate::app::BoardNetMsg::SelectRequested(Some("GND".into()));
    let _ = app.update(net_message(select));
    let picked = selected(&app);
    assert_eq!(
        picked.len(),
        5,
        "the track, two pads, the arc, the free pad"
    );
    assert!(picked.contains(&ItemRef::Arc(GND_ARC)));
    assert!(picked.contains(&ItemRef::FreePad(GND_FREE_PAD)));

    let _ = app.update(Message::Edit(crate::app::EditMsg::DeleteSelected));
    let board = app.document_state.pcb_board(&path).unwrap();
    assert!(board.free.arcs.is_empty(), "Delete took the arc");
    assert!(board.free.pads.is_empty(), "Delete took the free pad");
}

/// #2440: Highlight Net draws every chord of the net's copper arc and its
/// free pad bright.
#[test]
fn highlight_net_draws_the_copper_arcs_and_free_pads_of_the_net() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board_with_free_copper());
    let highlight = crate::app::BoardNetMsg::HighlightRequested(Some("GND".into()));
    let _ = app.update(net_message(highlight));
    let board = app.document_state.pcb_board(&path).unwrap();
    let chords = pcb_geom::arc_shapes(&board.free.arcs[0]).unwrap();
    let pad = pcb_geom::free_pad_shape(&board.free.pads[0]).unwrap();
    let lit = &app.interaction_state.pcb_canvas.highlight().net;
    assert_eq!(lit.len(), 3 + chords.len() + 1);
    assert!(chords.iter().all(|c| lit.contains(c)), "every chord");
    assert!(lit.contains(&pad));
}

#[test]
fn the_net_inspector_lists_the_board_nets_and_selects_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(
        PanelKind::NetInspector,
    )));
    let view = app.document_state.panel_ctx.net_inspector.board.clone();
    let view = view.expect("the board's nets");
    assert_eq!(view.total, 2);
    let gnd = &view.nets[0];
    assert_eq!((gnd.name.as_str(), gnd.pads, gnd.unrouted), ("GND", 2, 0));
    assert!(close(gnd.routed_length_mm, 10.0));
    let vcc = &view.nets[1];
    assert_eq!((vcc.name.as_str(), vcc.pads, vcc.unrouted), ("VCC", 2, 1));

    let inspector =
        |msg| Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::NetInspector(msg)));
    let _ = app.update(inspector(NetInspectorMsg::BoardNetSelected("VCC".into())));
    assert_eq!(selected(&app).len(), 2, "the two VCC pads");
    let _ = app.update(inspector(NetInspectorMsg::BoardNetHighlighted(
        "VCC".into(),
    )));
    assert_eq!(
        app.interaction_state
            .pcb_query
            .net_highlight
            .as_ref()
            .map(|h| h.net),
        Some(2)
    );

    let nets = answer(&mut app, "query_board_nets", &[document(&path).as_str()]);
    assert_eq!(nets["count"], 2);
    assert_eq!(nets["nets"][1]["name"], "VCC");
    assert_eq!(nets["nets"][1]["routed"], false);
    assert_eq!(nets["nets"][0]["routed"], true);
}

#[test]
fn a_board_menu_row_runs_its_command_on_the_footprint() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    let row = |id: &str| {
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command(id),
            project: None,
        }))
    };
    let _ = app.update(row("flip_selection"));
    assert_eq!(footprint(&app, &path, R1).layer, "BottomCopper");
    let _ = app.update(row("undo"));
    assert_eq!(footprint(&app, &path, R1).layer, "TopCopper");
    let _ = app.update(row("find_similar_objects"));
    assert!(
        app.ui_state.find_similar.as_ref().is_some_and(|s| s.board),
        "Find Similar opened for the footprint"
    );
}

#[test]
fn a_read_only_board_greys_its_editing_commands() {
    let dir = tempfile::tempdir().unwrap();
    let mut read_only = board();
    read_only.unknown.newer_minor_source = Some("snxpcb/9.9".into());
    let (mut app, _path) = app_with(dir.path(), read_only);
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    let ctx = app.command_context(None);
    assert!(ctx.pcb.read_only);
    for id in [
        "board_sketch",
        "place_board_via",
        "place_board_keepout",
        "flip_selection",
        "rotate_clockwise",
        "move_selection",
        "delete_selection",
        "cut",
        "change_footprint",
    ] {
        assert!(!is_enabled(&command(id), &ctx), "{id} on a read-only board");
    }
    for id in [
        "copy",
        "select_board_net",
        "measure_distance",
        "toggle_ratsnest",
    ] {
        assert!(is_enabled(&command(id), &ctx), "{id} only reads the board");
    }
}

/// #2406: every binding a shipped profile gives the board context names a
/// command that is enabled on a board with an unlocked footprint selected.
#[test]
fn every_board_context_binding_resolves_to_a_command_enabled_on_a_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), board());
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    let ctx = app.command_context(None);
    let set = crate::keymap::ShortcutProfileSet::built_ins().expect("built-ins");
    let mut checked = 0;
    for profile in set.profiles() {
        for binding in &profile.bindings {
            if binding.context != ShortcutContext::Pcb {
                continue;
            }
            let ShortcutBindingAction::Command(id) = &binding.action else {
                continue;
            };
            assert!(
                is_enabled(id, &ctx),
                "{} binds {id} on a board, where it is greyed",
                profile.id
            );
            checked += 1;
        }
    }
    assert!(checked >= 30, "only {checked} board bindings were checked");
}

/// #2406: in the Classic profile `L` flips the selection, `Q` toggles the
/// unit and `Ctrl+M` measures on a board.
#[test]
fn classic_l_q_and_ctrl_m_act_on_a_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    let press = |app: &mut Signex, text: &str| {
        let stroke = KeyStroke::from_str(text).expect("a stroke");
        let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
            window: None,
            stroke,
        }));
    };
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    press(&mut app, "L");
    assert_eq!(footprint(&app, &path, R1).layer, "BottomCopper");
    let unit = app.ui_state.unit;
    press(&mut app, "Q");
    assert_ne!(app.ui_state.unit, unit, "Q toggled the unit");
    press(&mut app, "Ctrl+M");
    assert!(
        app.interaction_state.pcb_query.measure.is_some(),
        "Measure armed"
    );
}

/// #2403: a Move, a paste or a net pick started while Measure is armed
/// ends Measure, so its clicks reach it: the footprint moves.
#[test]
fn a_move_started_after_measure_takes_the_clicks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with(dir.path(), board());
    app.select_pcb_items(&[ItemRef::Footprint(R1)]);
    run(&mut app, "measure_distance");
    assert!(app.interaction_state.pcb_query.measure.is_some(), "armed");
    run(&mut app, "move_selection");
    assert!(app.interaction_state.pcb_query.measure.is_none(), "ended");
    click(&mut app, 10.0, 10.0);
    click(&mut app, 15.0, 10.0);
    assert!(app.interaction_state.pcb_carry.is_none(), "dropped");
    let moved = footprint(&app, &path, R1).position;
    assert!(close(moved.x, 15.0) && close(moved.y, 10.0), "{moved:?}");

    // A paste on the cursor ends Measure too.
    run(&mut app, "copy");
    run(&mut app, "measure_distance");
    run(&mut app, "paste");
    assert!(app.interaction_state.pcb_carry.is_some(), "on the cursor");
    assert!(app.interaction_state.pcb_query.measure.is_none());
}

/// #2403: Select Net and Highlight Net take the net by name, from a typed
/// command line or an agent, and refuse a name the board does not have.
#[test]
fn select_and_highlight_net_take_a_net_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with(dir.path(), board());
    let args = |id: &str, net: &str| {
        let net = format!("net={net}");
        CommandArgs::parse_operands(&command(id), &[net.as_str()]).expect("operands parse")
    };
    // Nothing selected: the name alone picks the net.
    let _ = app.dispatch_command(
        &command("select_board_net"),
        args("select_board_net", "vcc"),
        None,
    );
    assert_eq!(selected(&app).len(), 2, "the two VCC pads");

    let (outcome, _task) = app.run_operands(
        &command("highlight_board_net"),
        &args("highlight_board_net", "GND"),
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    let lit = app.interaction_state.pcb_query.net_highlight.clone();
    assert_eq!(lit.map(|h| (h.net, h.name)), Some((1, "GND".to_string())));

    let (outcome, _task) = app.run_operands(
        &command("select_board_net"),
        &args("select_board_net", "NOPE"),
    );
    assert_eq!(
        outcome,
        CommandOutcome::Disabled {
            reason: "the board has no net named `NOPE`".into()
        }
    );
    assert_eq!(selected(&app).len(), 2, "the selection is kept");
}

/// #2403: results left from a board no longer in view are not acted on:
/// Replace All searches the board now in view instead, and the next one
/// replaces there.
#[test]
fn find_results_of_a_board_out_of_view_are_searched_again() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, first) = app_with(dir.path(), board());
    let _ = app.update(Message::Overlay(OverlayMsg::OpenReplace));
    let find = Message::FindReplaceMsg;
    let _ = app.update(find(FindReplaceMsg::QueryChanged("10K".into())));
    let _ = app.update(find(FindReplaceMsg::ReplacementChanged("4k7".into())));
    let mut other = board();
    other.uuid = Uuid::from_u128(2);
    let second = dir.path().join("other.snxpcb");
    app.open_pcb_tab(second.clone(), "other".into(), other);

    let _ = app.update(find(FindReplaceMsg::ReplaceAll));
    assert_eq!(footprint(&app, &first, R1).value, "10k", "not in view");
    assert_eq!(footprint(&app, &second, R1).value, "10k", "not yet shown");
    let listed = app
        .ui_state
        .find_replace
        .board
        .as_ref()
        .map(|b| b.path.clone());
    assert_eq!(listed, Some(second.clone()), "the board in view's results");

    let _ = app.update(find(FindReplaceMsg::ReplaceAll));
    assert_eq!(footprint(&app, &second, R1).value, "4k7");
    assert_eq!(footprint(&app, &first, R1).value, "10k");
}
