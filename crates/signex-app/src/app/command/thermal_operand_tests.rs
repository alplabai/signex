//! #2580 / #2593: `set_pour_thermal_relief` and `set_pad_thermal_relief`
//! through `run_operands`, the headless path an agent and the CLI take. Each
//! edit is one board undo step, a refused one changes nothing, and the fill
//! the board then holds follows the values.

use std::path::{Path, PathBuf};

use signex_types::format::SnxPcb;
use signex_types::pad_thermal::PadThermal;
use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Zone};
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::keymap::AppCommandId;

const POUR: Uuid = Uuid::from_u128(0x401);
const FOOTPRINT: Uuid = Uuid::from_u128(0x402);
const PAD: Uuid = Uuid::from_u128(0x403);

/// A net-1 pour over (-5,-5)..(5,5) on the top copper, Thermal Relief off,
/// round a 1 mm square SMD pad of net 1 at the origin.
fn board() -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"},
                   {"id": 31, "name": "BottomCopper", "layer_type": "signal"}],
        "nets": [{"number": 1, "name": "GND"}],
    }))
    .unwrap();
    board.footprints.push(Footprint {
        uuid: FOOTPRINT,
        reference: "J1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
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
            size: Point::new(1.0, 1.0),
            drill: None,
            layers: vec!["TopCopper".into()],
            net: Some(PadNet {
                number: 1,
                name: "GND".into(),
            }),
            roundrect_ratio: 0.0,
            trapezoid_delta: Point::ZERO,
            custom_outline: Vec::new(),
            rotation: 0.0,
            layer_sizes: None,
            net_locked: false,
            thermal: PadThermal::RULE,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    });
    board.zones.push(Zone {
        uuid: POUR,
        net: 1,
        net_name: "GND".into(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(-5.0, -5.0),
            Point::new(5.0, -5.0),
            Point::new(5.0, 5.0),
            Point::new(-5.0, 5.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.5,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        corners: Default::default(),
        corner_size: 0.0,
        fill_mode: Default::default(),
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    });
    board
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

fn run(app: &mut Signex, path: &Path, command: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(command).unwrap();
    let document = format!("document={}", path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &all).expect("operands");
    app.run_operands(&id, &args).0
}

fn answer(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("answered nothing: {other:?}"),
    }
}

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

/// Whether the board's pour holds copper at (x, y) mm.
fn copper_at(board: &PcbBoard, x: f64, y: f64) -> bool {
    let at = pcb_geom::PointNm::from_mm(Point::new(x, y), "test").unwrap();
    pcb_geom::zone_fills(board)
        .of(0)
        .expect("the pour is filled")
        .pieces
        .iter()
        .any(|piece| piece.contains_point(at))
}

#[test]
fn an_agent_switches_a_pours_thermal_relief_on_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let ids = format!("ids={POUR}");
    assert!(
        copper_at(board_of(&app, &path), 0.75, 0.4),
        "solid at first"
    );

    let said = answer(run(
        &mut app,
        &path,
        "set_pour_thermal_relief",
        &[ids.as_str(), "on=true", "air_gap=0.5", "spoke_width=0.3"],
    ));
    assert_eq!(
        said["pours"],
        serde_json::json!([{
            "id": POUR, "thermal_relief": true, "spoke_width": 0.3, "air_gap": 0.5
        }])
    );
    let held = board_of(&app, &path);
    assert!(!copper_at(held, 0.75, 0.4), "the air gap");
    assert!(copper_at(held, 0.75, 0.0), "a spoke");

    undo(&mut app);
    let zone = &board_of(&app, &path).zones[0];
    assert!(!zone.thermal_relief, "one step undoes all three");
    assert_eq!((zone.thermal_gap, zone.thermal_width), (0.0, 0.0));
}

#[test]
fn a_pour_command_naming_no_value_or_no_pour_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let pour = format!("ids={POUR}");
    let outcome = run(&mut app, &path, "set_pour_thermal_relief", &[pour.as_str()]);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    let pad = format!("ids={PAD}");
    let outcome = run(
        &mut app,
        &path,
        "set_pour_thermal_relief",
        &[pad.as_str(), "on=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(!board_of(&app, &path).zones[0].thermal_relief);
}

/// #2593: a pad's own air gap and spokes are saved and the pour uses them;
/// `rule` follows the pour again; a count above 4 is refused.
#[test]
fn an_agent_sets_a_pads_own_thermal_values_and_the_pour_uses_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let pour = format!("ids={POUR}");
    let pad = format!("ids={PAD}");
    answer(run(
        &mut app,
        &path,
        "set_pour_thermal_relief",
        &[pour.as_str(), "on=true", "air_gap=0.5", "spoke_width=0.3"],
    ));

    let said = answer(run(
        &mut app,
        &path,
        "set_pad_thermal_relief",
        &[pad.as_str(), "air_gap=1", "spokes=2"],
    ));
    assert_eq!(
        said["pads"],
        serde_json::json!([{
            "id": PAD, "air_gap": 1.0, "spoke_width": "rule", "spokes": 2
        }])
    );
    let held = board_of(&app, &path);
    assert!(copper_at(held, 1.25, 0.0), "a spoke along x");
    assert!(!copper_at(held, 0.0, 1.25), "two spokes: none along y");
    let text = SnxPcb::new(held.clone()).write_string().expect("saves");
    let back = SnxPcb::parse(&text).expect("reopens");
    assert_eq!(
        back.board.footprints[0].pads[0].thermal, held.footprints[0].pads[0].thermal,
        "the values survive save and reopen"
    );

    let outcome = run(
        &mut app,
        &path,
        "set_pad_thermal_relief",
        &[pad.as_str(), "spokes=5"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    answer(run(
        &mut app,
        &path,
        "set_pad_thermal_relief",
        &[pad.as_str(), "air_gap=rule"],
    ));
    let thermal = board_of(&app, &path).footprints[0].pads[0].thermal;
    assert_eq!((thermal.air_gap, thermal.spokes), (None, Some(2)));

    undo(&mut app);
    assert_eq!(
        board_of(&app, &path).footprints[0].pads[0].thermal.air_gap,
        Some(1.0),
        "one step each"
    );
}
