//! End to end (#2199 PR C): a part from the bundled stock library placed
//! through the real `signex-cli` binary, then wired by pin reference, and
//! what `--dry-run` says about it.

mod support;

use serde_json::{Value, json};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root};

/// The bundled stock library's id (`assets/stock-library/Stock.snxlib`).
const STOCK: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";

fn part() -> Value {
    json!({"id": "place_component", "args": {
        "library": STOCK, "part": "STOCK-R", "at": "63.5,63.5", "document": "main.snxsch"
    }})
}

#[test]
fn a_part_placed_from_the_stock_library_is_wired_by_its_pin_and_saved() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    // The stock resistor stands upright, pin 1 on top: the wire leaves
    // upwards, clear of its body.
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            part(),
            json!({"id": "place_wire", "args": {
                "start": "R2.1", "end": "63.5,50.8", "allow_open": true,
                "document": "main.snxsch"
            }}),
            json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
        ]),
    );
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    let placed = &out.lines[1]["data"];
    // R1 is on the sheet, so the stock resistor is numbered after it.
    assert_eq!(placed["designator"], "R2", "{}", out.stdout);
    assert_eq!(placed["pins"][0]["number"], "1", "{}", out.stdout);
    assert_eq!(out.lines[2]["status"], "answered", "{}", out.stdout);
    assert_eq!(
        out.lines[2]["data"]["ends"][0]["at"], placed["pins"][0]["at"],
        "{}",
        out.stdout
    );
    assert_eq!(out.lines[3]["status"], "ran", "{}", out.stdout);

    let again = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "query_pins", "args": {"document": "main.snxsch", "designator": "R2"}}),
            json!({"id": "read_document", "args": {"document": "main.snxsch", "kinds": "wire"}}),
        ]),
    );
    assert_eq!(again.lines[1]["status"], "answered", "{}", again.stdout);
    assert_eq!(again.lines[2]["data"]["total"], 1, "{}", again.stdout);
}

#[test]
fn dry_run_refuses_a_part_placement() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let sheet = fx.sheet.to_str().unwrap();
    let before = snapshot(&root);
    let out = run_cli(&["run", sheet, "--dry-run"], jsonl(&[part()]));
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[0]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("`dry_run` operand"), "{reason}");
    assert_eq!(snapshot(&root), before);
}
