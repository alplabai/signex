//! #2735: the Via panel and the engine or the design rule check give one
//! answer. Which size is overridden is the engine's judgement, so float
//! noise under the 1 nm the file keeps is no override; the minimum drill is
//! the written rule the check holds the via to, else the board setup's.

use pcb_geom::ItemRef;
use serde_json::json;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::panels::pcb_primitive_properties::{PcbPrimitiveFacts, ViaFacts};

const NOISE: u128 = 3;
const SIGNAL: u128 = 4;
const POWER: u128 = 5;

/// Two copper layers; through vias 0.6/0.3 (two), one 5e-8 mm off that
/// drill, and 0.25 mm drills on net `SIG` and net `PWR`; setup minimum
/// drill 0.3.
fn board() -> PcbBoard {
    let layer = |id: u8, name: &str| json!({"id": id, "name": name, "layer_type": "signal"});
    let via = |n: u128, x: f64, drill: f64, net: u32| {
        json!({"uuid": Uuid::from_u128(n), "position": {"x": x, "y": 0.0},
               "diameter": 0.6, "drill": drill, "net": net,
               "layers": ["TopCopper", "BottomCopper"]})
    };
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "layers": [layer(0, "TopCopper"), layer(31, "BottomCopper")],
        "setup": {"via_diameter": 0.6, "via_drill": 0.3, "via_min_drill": 0.3},
        "nets": [{"number": 1, "name": "SIG"}, {"number": 2, "name": "PWR"}],
        "vias": [
            via(1, 0.0, 0.3, 1),
            via(2, 5.0, 0.3, 1),
            via(NOISE, 10.0, 0.3 + 5e-8, 1),
            via(SIGNAL, 15.0, 0.25, 1),
            via(POWER, 20.0, 0.25, 2)
        ]
    }))
    .unwrap()
}

fn app() -> (Signex, std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let mut power = NetClass::named("Power");
    power.nets = vec!["PWR".into()];
    app.document_state.projects[0].data.net_classes = vec![power];
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path, dir)
}

/// The Via panel's facts with via `n` selected.
fn via_facts(app: &mut Signex, path: &std::path::Path, n: u128) -> ViaFacts {
    app.select_pcb_items(&[ItemRef::Via(Uuid::from_u128(n))]);
    let board = app.document_state.pcb_board(path).expect("board").clone();
    match app.pcb_primitive_facts(path, &board) {
        Some(PcbPrimitiveFacts::Via(facts)) => facts,
        other => panic!("the via panel shows: {other:?}"),
    }
}

#[test]
fn a_via_in_float_noise_of_its_type_is_not_overridden() {
    let (mut app, path, _dir) = app();
    let facts = via_facts(&mut app, &path, NOISE);
    assert_eq!(facts.type_size, Some((0.6, 0.3)));
    assert_eq!(facts.overridden, Some(false));
    let sizes = facts.overridden_sizes.expect("a shared type");
    assert!(!sizes.diameter && !sizes.drill, "{sizes:?}");

    let facts = via_facts(&mut app, &path, SIGNAL);
    assert_eq!(facts.overridden, Some(true));
    let sizes = facts.overridden_sizes.expect("a shared type");
    assert!(!sizes.diameter && sizes.drill, "only the drill: {sizes:?}");
}

#[test]
fn the_minimum_drill_is_the_rule_the_check_applies_else_the_setup() {
    let (mut app, path, _dir) = app();
    // No rule text: the setup's minimum, as before.
    assert_eq!(via_facts(&mut app, &path, SIGNAL).min_drill, Some(0.3));

    // Setup 0.3, a board rule of 0.2: the panel says what the check says.
    app.ui_state.machine_drc_rules.dsl = Some(
        "rule vias on via require drill >= 0.2mm then error\n\
         rule power on via when class == \"Power\" require drill >= 0.3mm then error\n"
            .into(),
    );
    assert_eq!(via_facts(&mut app, &path, SIGNAL).min_drill, Some(0.2));
    // The class rule holds the Power via to 0.3, as the check flags it.
    assert_eq!(via_facts(&mut app, &path, POWER).min_drill, Some(0.3));
    let board = app.document_state.pcb_board(&path).unwrap().clone();
    let (rules, _) = app.board_design_rules(&path, &board).unwrap();
    let flagged: Vec<u32> = signex_drc::run(&board, &rules)
        .violations
        .iter()
        .filter(|v| v.kind == signex_drc::ViolationKind::ViaDrill)
        .map(|v| v.nets.0)
        .collect();
    assert_eq!(flagged, [2], "the check flags the Power via alone");
}
