//! #2389 — length tuning through the app: Enter in the track panel's Tune
//! Net To field meanders the track until its net reaches the target, on
//! the side clear of other nets, legal by the rule check, as one undo step.

use pcb_geom::ItemRef;
use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget};
use crate::panels::{PanelMsg, PcbPrimitiveMsg};

const TRACK: u128 = 11;

/// Net A's 20 mm track along y = 0 and a net-B track 2 mm above its middle.
fn app() -> (Signex, std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let track = |n: u128, a: (f64, f64), b: (f64, f64), net: u32| {
        json!({
            "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1},
            "end": {"x": b.0, "y": b.1}, "width": 0.25, "layer": "TopCopper", "net": net
        })
    };
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "segments": [
            track(TRACK, (0.0, 0.0), (20.0, 0.0), 1),
            track(12, (5.0, 2.0), (15.0, 2.0), 2)
        ]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path, dir)
}

fn board(app: &Signex, path: &std::path::Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn tune_to(app: &mut Signex, target: &PrimitiveTarget, text: &str) {
    let field = PrimitiveField::TuneLength;
    let panel = |msg| Message::Dock(DockMessage::Panel(PanelMsg::PcbPrimitive(msg)));
    let _ = app.update(panel(PcbPrimitiveMsg::FieldTyped {
        target: target.clone(),
        field,
        text: text.into(),
    }));
    let _ = app.update(panel(PcbPrimitiveMsg::FieldSubmitted {
        target: target.clone(),
        field,
    }));
}

#[test]
fn tuning_reaches_the_target_length_legally_in_one_step() {
    let (mut app, path, _dir) = app();
    let before = serde_json::to_string(&board(&app, &path)).unwrap();
    let target = PrimitiveTarget {
        board: path.clone(),
        items: vec![ItemRef::Track(Uuid::from_u128(TRACK))],
    };
    // The panel edits what it shows: the selected track.
    app.select_pcb_items(&target.items);
    tune_to(&mut app, &target, "26");

    let tuned = board(&app, &path);
    let length = pcb_geom::net_routed_length_mm(&tuned, 1).unwrap();
    assert!((length - 26.0).abs() < 1.0e-3, "net length {length}");
    assert!(
        tuned.segments.iter().filter(|s| s.net == 1).count() > 1,
        "the track became an accordion"
    );
    let (rules, _) = app.board_design_rules(&path, &tuned).unwrap();
    let report = signex_drc::run(&tuned, &rules);
    assert!(report.is_clean(), "{:?}", report.violations);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(serde_json::to_string(&board(&app, &path)).unwrap(), before);
}

#[test]
fn tuning_changes_nothing_for_two_tracks_or_a_net_long_enough() {
    let (mut app, path, _dir) = app();
    let before = serde_json::to_string(&board(&app, &path)).unwrap();
    let two = PrimitiveTarget {
        board: path.clone(),
        items: vec![
            ItemRef::Track(Uuid::from_u128(TRACK)),
            ItemRef::Track(Uuid::from_u128(12)),
        ],
    };
    tune_to(&mut app, &two, "26");
    let one = PrimitiveTarget {
        items: vec![ItemRef::Track(Uuid::from_u128(TRACK))],
        ..two
    };
    tune_to(&mut app, &one, "15");
    assert_eq!(serde_json::to_string(&board(&app, &path)).unwrap(), before);
}
