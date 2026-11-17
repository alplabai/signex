//! End to end (#2339): `check_requirements` through the real `signex-cli`
//! binary runs the project's requirements list and answers with a check
//! report, named by the project file or by one of its documents, compares a
//! later run with an earlier one and writes nothing.

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::pcb::Point;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// A 120 x 50 mm board, listed as the project's board, with two board-size
/// requirements: one it breaks and one it meets.
fn write_board_and_requirements(dir: &Path, project: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = vec![
        Point::new(0.0, 0.0),
        Point::new(120.0, 0.0),
        Point::new(120.0, 50.0),
        Point::new(0.0, 50.0),
    ];
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("board.snxpcb"), text).expect("write board");
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some("board.snxpcb".into());
    data.requirements = serde_json::from_value(json!([
        {"id": "size", "kind": "board_size", "max_width_mm": 100, "max_height_mm": 80},
        {"id": "tall", "kind": "board_size", "max_width_mm": 200, "max_height_mm": 60},
    ]))
    .expect("requirements");
    signex_types::project::write_project(project, &data).expect("project");
}

#[test]
fn check_requirements_answers_with_a_report_and_compares_with_its_baseline() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board_and_requirements(&fx.dir, &fx.project);
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[
            json!({"id": "check_requirements", "args": {"document": "demo.snxprj"}}),
            json!({"id": "check_requirements",
                   "args": {"document": "main.snxsch", "baseline": "1"}}),
        ]),
    );
    let first = &out.lines[0];
    assert_eq!(first["status"], "answered", "{}", out.stdout);
    assert_eq!(first["data"]["check"], "requirements", "{}", out.stdout);
    let findings = first["data"]["findings"].as_array().expect("findings");
    assert_eq!(findings.len(), 1, "{}", out.stdout);
    assert_eq!(findings[0]["rule"], "req::size", "{}", out.stdout);
    assert_eq!(
        first["data"]["passed"],
        json!(["req::tall"]),
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
    assert_eq!(
        without_journals(snapshot(&root)),
        before,
        "a check writes nothing"
    );
}
