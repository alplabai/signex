//! #2986: `board_status` counts the waived DRC findings of the whole last
//! report, not the rows the DRC panel lists.

use std::collections::BTreeMap;
use std::time::SystemTime;

use pcb_geom::PointNm;
use serde_json::Value;
use signex_drc::{DrcReport, DrcRules, Violation, ViolationKind};
use signex_types::pcb_options::DrcWaiver;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::DrcInput;
use crate::keymap::AppCommandId;

fn finding(n: i64) -> Violation {
    Violation {
        kind: ViolationKind::Clearance,
        items: Vec::new(),
        nets: (1, 2),
        layers: 1,
        at: PointNm::new(n * 1_000_000, 0),
        measured_nm: 100_000,
        required_nm: 200_000,
        keepout: None,
        rule: None,
    }
}

#[test]
fn waived_counts_every_finding_of_the_report_not_the_panel_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.path().join("board.snxpcb");
    let violations: Vec<Violation> = (0..600).map(finding).collect();
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    let late = &violations[550];
    board.options.drc_waivers = vec![DrcWaiver {
        key: signex_drc::waiver_key(late),
        rule: "builtin::clearance".into(),
        note: String::new(),
        fingerprint: signex_drc::fingerprint(late, &board),
        author: String::new(),
    }];
    app.open_pcb_tab(path.clone(), "board.snxpcb".into(), board);
    let revision = app.board_revision(&path);
    let drc = &mut app.interaction_state.pcb_drc;
    let generation = drc.begin(DrcInput {
        path: path.clone(),
        revision,
        net_classes: Vec::new(),
    });
    let report = DrcReport {
        rules: DrcRules::new(200_000),
        violations,
        skipped: Vec::new(),
        skipped_keepouts: Vec::new(),
    };
    assert!(drc.finish(generation, report, BTreeMap::new(), SystemTime::now()));

    let id = AppCommandId::new("board_status").expect("a catalog id");
    let document = format!("document={}", path.display());
    let args = CommandArgs::parse_operands(&id, &[&document]).expect("operands parse");
    let answer = match app.run_headless_command_in_test(&id, args).0 {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    };
    let drc: &Value = &answer["drc"];
    assert_eq!(drc["ran"], true, "{answer}");
    assert_eq!(drc["violations"], 600, "{answer}");
    assert_eq!(drc["waived"], 1, "the waiver past the panel's rows counts: {answer}");
}
