//! End to end (#2199, decisions 6817821301 and 6818011610): `check_drc`
//! through the real `signex-cli` binary answers with a check report of the
//! board, compares a later run with an earlier one, refuses a baseline it
//! does not keep and writes nothing. (A board outside the project folder is
//! refused by the CLI's operand check before the app sees it; the app's own
//! confined read is tested in `check_drc_tests`.)

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::pcb::{NetDef, Point, Segment};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// A board whose GND and VCC tracks are 0.1 mm apart, under the Standard
/// setup's 0.15 mm clearance.
fn write_board(path: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = vec![
        NetDef {
            number: 1,
            name: "GND".into(),
        },
        NetDef {
            number: 2,
            name: "VCC".into(),
        },
    ];
    board.segments = [(1, 0.0), (2, 0.35)]
        .into_iter()
        .map(|(net, y)| Segment {
            uuid: uuid::Uuid::from_u128(0x2000 + u128::from(net)),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            width: 0.25,
            layer: "TopCopper".into(),
            net,
            unknown_columns: Default::default(),
        })
        .collect();
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write board");
}

/// List `filename` as the fixture project's board.
fn set_board(project: &Path, filename: &str) {
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some(filename.into());
    signex_types::project::write_project(project, &data).expect("project");
}

#[test]
fn check_drc_answers_with_a_report_of_the_board_and_compares_with_its_baseline() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let check = |extra: serde_json::Value| {
        let mut args = json!({"document": "board.snxpcb"});
        if let Some(map) = extra.as_object() {
            for (k, v) in map {
                args[k] = v.clone();
            }
        }
        json!({"id": "check_drc", "args": args})
    };
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[
            check(json!({})),
            check(json!({"baseline": "1"})),
            check(json!({"baseline": "9"})),
        ]),
    );
    let first = &out.lines[0];
    assert_eq!(first["status"], "answered", "{}", out.stdout);
    assert_eq!(first["data"]["check"], "drc", "{}", out.stdout);
    assert_eq!(first["data"]["run"], 1, "{}", out.stdout);
    let findings = first["data"]["findings"].as_array().expect("findings");
    assert!(
        findings
            .iter()
            .any(|f| f["rule"] == "builtin::clearance" && f["severity"] == "error"),
        "{}",
        out.stdout
    );
    assert_eq!(
        first["data"]["rule_set"]["source"], "machine",
        "{}",
        out.stdout
    );

    let second = &out.lines[1];
    assert_eq!(second["status"], "answered", "{}", out.stdout);
    assert_eq!(second["data"]["findings"], first["data"]["findings"]);
    assert_eq!(
        second["data"]["baseline"],
        json!({"run": 1, "new": [], "gone": []}),
        "{}",
        out.stdout
    );

    let third = &out.lines[2];
    assert_eq!(third["status"], "refused", "{}", out.stdout);
    let reason = third["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("no such run"), "{reason}");

    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}
