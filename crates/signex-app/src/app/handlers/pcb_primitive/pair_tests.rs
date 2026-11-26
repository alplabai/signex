//! #2585 — Track Properties for a differential pair, through
//! `Signex::update` and the headless path: a pair track shows the pair, the
//! paired net and the gap; Width reaches both tracks of the pair as one
//! undo step; `set_diff_pair_width` does the same for an agent.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EditMsg, Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::pcb_primitive_properties::{PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget};
use crate::panels::{PanelMsg, PcbPrimitiveMsg};

/// USB_P and USB_N side by side on Top (0.25 mm tracks, centres 0.4 mm
/// apart: a 0.15 mm gap), a second USB_N track elsewhere, and GND.
fn app() -> (Signex, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let track = |n: u128, a: (f64, f64), b: (f64, f64), net: u32| {
        json!({
            "uuid": Uuid::from_u128(n), "start": {"x": a.0, "y": a.1},
            "end": {"x": b.0, "y": b.1}, "width": 0.25, "layer": "TopCopper", "net": net
        })
    };
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.1},
        "nets": [
            {"number": 1, "name": "USB_P"},
            {"number": 2, "name": "USB_N"},
            {"number": 3, "name": "GND"}
        ],
        "segments": [
            track(11, (0.0, 0.0), (10.0, 0.0), 1),
            track(12, (0.0, 0.4), (10.0, 0.4), 2),
            track(13, (20.0, 5.0), (30.0, 5.0), 2),
            track(14, (0.0, 9.0), (10.0, 9.0), 3)
        ]
    }))
    .unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let path = dir.path().join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board);
    (app, path, dir)
}

fn board(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn width_of(app: &Signex, path: &Path, n: u128) -> f64 {
    board(app, path)
        .segments
        .iter()
        .find(|s| s.uuid == Uuid::from_u128(n))
        .expect("track")
        .width
}

fn target(path: &Path, tracks: &[u128]) -> PrimitiveTarget {
    PrimitiveTarget {
        board: path.to_path_buf(),
        items: tracks
            .iter()
            .map(|&n| ItemRef::Track(Uuid::from_u128(n)))
            .collect(),
    }
}

fn type_width(app: &mut Signex, target: &PrimitiveTarget, text: &str) {
    let field = PrimitiveField::Width;
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

fn track_facts(app: &Signex, path: &Path) -> crate::panels::pcb_primitive_properties::TrackFacts {
    let shown = board(app, path);
    match app.pcb_primitive_facts(path, &shown) {
        Some(PcbPrimitiveFacts::Track(facts)) => facts,
        other => panic!("not a track panel: {other:?}"),
    }
}

#[test]
fn a_pair_track_shows_the_pair_the_paired_net_and_the_gap() {
    let (mut app, path, _dir) = app();
    app.select_pcb_items(&target(&path, &[11]).items);
    let pair = track_facts(&app, &path).pair.expect("a pair track");
    assert_eq!(pair.name, "USB");
    assert_eq!(pair.paired_net.as_deref(), Some("USB_N"));
    assert!((pair.gap_mm.expect("a gap") - 0.15).abs() < 1.0e-9);
    assert_eq!(
        pair.tracks,
        [Uuid::from_u128(11), Uuid::from_u128(12)],
        "Width reaches the coupled track"
    );

    app.select_pcb_items(&target(&path, &[11, 12]).items);
    let both = track_facts(&app, &path).pair.expect("a pair track");
    assert_eq!(
        both.paired_net, None,
        "both nets selected: the row names both"
    );

    app.select_pcb_items(&target(&path, &[14]).items);
    assert_eq!(track_facts(&app, &path).pair, None, "GND is in no pair");
}

#[test]
fn width_on_one_pair_track_reaches_both_in_one_undo_step() {
    let (mut app, path, _dir) = app();
    let before = serde_json::to_string(&board(&app, &path)).unwrap();
    let one = target(&path, &[11]);
    app.select_pcb_items(&one.items);
    type_width(&mut app, &one, "0.2");
    assert_eq!(width_of(&app, &path, 11), 0.2);
    assert_eq!(width_of(&app, &path, 12), 0.2, "the partner too");
    assert_eq!(width_of(&app, &path, 13), 0.25, "an uncoupled track stays");
    assert_eq!(width_of(&app, &path, 14), 0.25, "GND stays");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(serde_json::to_string(&board(&app, &path)).unwrap(), before);
}

#[test]
fn width_on_a_track_of_no_pair_stays_a_plain_track_edit() {
    let (mut app, path, _dir) = app();
    let gnd = target(&path, &[14]);
    app.select_pcb_items(&gnd.items);
    type_width(&mut app, &gnd, "0.3");
    assert_eq!(width_of(&app, &path, 14), 0.3);
    assert_eq!(width_of(&app, &path, 11), 0.25);
}

fn headless(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("set_diff_pair_width").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_headless_command_in_test(&command, args).0
}

#[test]
fn an_agent_sets_the_width_of_every_track_of_a_pair() {
    let (mut app, path, _dir) = app();
    let document = format!("document={}", path.display());
    let outcome = headless(&mut app, &[&document, "net=USB_N", "width=0.18"]);
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["pair"], "USB");
    assert_eq!(answer["positive"], "USB_P");
    assert_eq!(answer["negative"], "USB_N");
    assert_eq!(
        answer["tracks"].as_array().map(Vec::len),
        Some(3),
        "{answer}"
    );
    for n in [11, 12, 13] {
        assert_eq!(width_of(&app, &path, n), 0.18, "track {n}");
    }
    assert_eq!(width_of(&app, &path, 14), 0.25);

    let refused = headless(&mut app, &[&document, "net=GND", "width=0.2"]);
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if signex_i18n::RenderTranslatable::render_en_us(reason).contains("no differential pair")),
        "{refused:?}"
    );
    let refused = headless(&mut app, &[&document, "net=USB_P", "width=-1"]);
    assert!(
        matches!(refused, CommandOutcome::Disabled { .. }),
        "{refused:?}"
    );
}
