//! End to end (#2943): the requirement kinds `rail_budget` and
//! `layer_count` through `check_requirements`, a project that passes and one
//! that fails, and `project_set_rail` with `query_rail_budget` through the
//! real `signex-cli` binary, which writes nothing without a save.

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::Point;
use signex_types::project::{RailEdit, SetupProfile, parse_project, write_project};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// R1 supplies 1 A to +3V3 and draws 0.1 A from +5V; C1 draws `load` from
/// +3V3 and supplies 0.5 A to +5V.
fn give_rail_parameters(sheet: &Path, load: &str) {
    let text = std::fs::read_to_string(sheet).expect("the sheet");
    let mut sheet_file = SnxSchematic::parse(&text).expect("the sheet reads");
    for symbol in &mut sheet_file.sheet.symbols {
        let fields: Vec<(&str, &str)> = match symbol.reference.as_str() {
            "R1" => vec![("Rail Supply +3V3", "1A"), ("Rail Load +5V", "0.1")],
            "C1" => vec![("Rail Load +3V3", load), ("Rail Supply +5V", "500mA")],
            _ => Vec::new(),
        };
        for (key, value) in fields {
            symbol.fields.insert(key.to_string(), value.to_string());
        }
    }
    let text = SnxSchematic::new(sheet_file.sheet)
        .write_string()
        .expect("encode");
    std::fs::write(sheet, text).expect("write sheet");
}

/// A board of `layers` copper layers, listed as the project's board, the
/// +3V3 rail at 3.3 V and the two requirements.
fn write_board_rail_and_requirements(dir: &Path, project: &Path, layers: u8) {
    let mut board = SetupProfile::Custom.pcb(layers).new_board();
    board.outline = vec![
        Point::new(0.0, 0.0),
        Point::new(50.0, 0.0),
        Point::new(50.0, 50.0),
        Point::new(0.0, 50.0),
    ];
    let text = SnxPcb::new(board).write_string().expect("encode");
    std::fs::write(dir.join("board.snxpcb"), text).expect("write board");
    let mut data = parse_project(project).expect("project");
    data.pcb_file = Some("board.snxpcb".into());
    let rail = RailEdit {
        voltage: Some("3.3".into()),
        ..RailEdit::default()
    };
    data.set_rail("+3V3", &rail).expect("a rail");
    data.requirements = serde_json::from_value(json!([
        {"id": "rails", "kind": "rail_budget", "net": "+3V3", "voltage": 3.3,
         "margin_percent": 10},
        {"id": "layers", "kind": "layer_count", "min_layers": 2, "max_layers": 4},
    ]))
    .expect("requirements");
    write_project(project, &data).expect("project");
}

/// The `check_requirements` answer of a project whose C1 draws `load` and
/// whose board has `layers` copper layers.
fn check(load: &str, layers: u8) -> serde_json::Value {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    give_rail_parameters(&fx.sheet, load);
    write_board_rail_and_requirements(&fx.dir, &fx.project, layers);
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[json!({"id": "check_requirements", "args": {"document": "demo.snxprj"}})]),
    );
    let line = &out.lines[0];
    assert_eq!(line["status"], "answered", "{}", out.stdout);
    line["data"].clone()
}

#[test]
fn a_rail_budget_and_a_layer_count_pass_on_one_project_and_fail_on_another() {
    let met = check("0.4", 4);
    assert_eq!(met["passed"], json!(["req::rails", "req::layers"]), "{met}");
    assert_eq!(met["findings"], json!([]), "{met}");

    let unmet = check("0.95", 6);
    // A check report leaves `passed` out when nothing passed.
    assert!(unmet.get("passed").is_none(), "nothing passed: {unmet}");
    let findings = unmet["findings"].as_array().expect("findings");
    // The report lists findings in its own order (by document); each rule
    // raises exactly one.
    let mut rules: Vec<&str> = findings
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    rules.sort_unstable();
    assert_eq!(rules, ["req::layers", "req::rails"], "{unmet}");
    let of = |rule: &str| {
        &findings
            .iter()
            .find(|f| f["rule"] == rule)
            .expect("the finding")["evidence"]
    };
    let rail = of("req::rails");
    assert_eq!(rail["kind"], "rail_budget");
    assert_eq!(rail["rail"]["source"]["designator"], "R1", "{unmet}");
    assert_eq!(rail["rail"]["loads"][0]["designator"], "C1", "{unmet}");
    assert_eq!(rail["total_a"], 0.95, "{unmet}");
    let layers = of("req::layers");
    assert_eq!(layers["count"], 6, "{unmet}");
    assert_eq!(layers["max_layers"], 4, "{unmet}");
}

#[test]
fn the_rails_are_set_and_their_budget_answered_and_nothing_is_written() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    give_rail_parameters(&fx.sheet, "250mA");
    let before = without_journals(snapshot(&root));
    // `--keep-going`: the refused voltage of 0 does not end the run.
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            json!({"id": "project_set_rail", "args": {"document": "demo.snxprj",
                   "net": "+3V3", "voltage": "3.3", "tolerance_percent": "5"}}),
            json!({"id": "project_set_rail", "args": {"document": "main.snxsch",
                   "net": "+5V", "voltage": "5"}}),
            json!({"id": "project_set_rail", "args": {"document": "demo.snxprj",
                   "net": "+5V", "voltage": "0"}}),
            json!({"id": "query_rail_budget", "args": {"document": "demo.snxprj"}}),
        ]),
    );
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    assert_eq!(
        out.lines[1]["data"]["rails"],
        json!({"+3V3": {"voltage": 3.3, "tolerance_percent": 5}, "+5V": {"voltage": 5}}),
        "{}",
        out.stdout
    );
    assert_ne!(
        out.lines[2]["status"], "answered",
        "a voltage of 0 is refused"
    );
    let budget = &out.lines[3];
    assert_eq!(budget["status"], "answered", "{}", out.stdout);
    let data = &budget["data"];
    assert_eq!(data["count"], 2, "{}", out.stdout);
    let three = &data["rails"][0];
    assert_eq!(three["net"], "+3V3");
    assert_eq!(three["tolerance_percent"], 5.0);
    assert_eq!(three["source"]["designator"], "R1");
    assert_eq!(
        three["loads"],
        json!([{"designator": "C1", "current_a": 0.25}])
    );
    assert_eq!(three["headroom_a"], 0.75);
    let five = &data["rails"][1];
    assert_eq!(five["net"], "+5V");
    assert_eq!(
        five["source"],
        json!({"designator": "C1", "current_a": 0.5})
    );
    assert_eq!(five["total_a"], 0.1);
    assert_eq!(
        without_journals(snapshot(&root)),
        before,
        "an edit without a save and a query write nothing"
    );
}
