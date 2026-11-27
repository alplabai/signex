//! End to end (#3013): a footprint file of the project opened through the
//! real `signex-cli` binary, with no window, and its pads moved by the
//! footprint editor's command.

mod support;

use std::path::PathBuf;

use serde_json::{Value, json};
use support::{fixture, jsonl, run_cli, temp_root};

/// The stock SOIC-8 footprint, copied into a project as a file of its own.
fn stock_soic8() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/stock-library/footprints/019df45d-af5f-7bb0-914a-26b538180e51.snxfpt")
}

fn move_pads(by: &str) -> Value {
    json!({"id": "footprint_move_pads", "args": {
        "document": "soic.snxfpt", "pads": "1,2", "by": by
    }})
}

#[test]
fn a_footprint_opened_headless_takes_the_pad_commands() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::copy(stock_soic8(), fx.dir.join("soic.snxfpt")).expect("copy the footprint");
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "soic.snxfpt"}}),
            move_pads("0,0"),
            move_pads("1,0"),
        ]),
    );
    assert_eq!(out.lines.len(), 3, "{}\n{}", out.stdout, out.stderr);
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    assert_eq!(out.lines[0]["data"]["kind"], "footprint", "{}", out.stdout);
    for line in &out.lines[1..] {
        assert_eq!(line["status"], "answered", "{}", out.stdout);
    }
    let (before, after) = (&out.lines[1]["data"], &out.lines[2]["data"]);
    assert_eq!(after["changed"], true, "{}", out.stdout);
    for pad in 0..2 {
        let x = |data: &Value| data["pads"][pad]["x"].as_f64().expect("x");
        assert!(
            (x(after) - x(before) - 1.0).abs() < 1e-9,
            "pad {pad}: {}",
            out.stdout
        );
    }
}
