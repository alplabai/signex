//! End to end (#2796): `export_block_diagram` through the real `signex-cli`
//! binary writes a project's block diagram as SVG and PDF, answers once the
//! file is written, needs the `export` capability and writes nothing outside
//! the project. The command line names a document it has open or that its
//! project lists, so each run opens the diagram first.

mod support;

use serde_json::{Value, json};
use support::{fixture, jsonl, run_cli, temp_root};

const DCDC: &str = include_str!("../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

fn open_the_diagram() -> Value {
    json!({"id": "open_document", "args": {"path": "dcdc-buck.snxblk"}})
}

#[test]
fn the_cli_writes_a_block_diagram_as_svg_and_pdf() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::write(fx.dir.join("dcdc-buck.snxblk"), DCDC).expect("write the diagram");
    let project = fx.project.to_str().unwrap();
    let commands = jsonl(&[
        open_the_diagram(),
        json!({"id": "export_block_diagram",
               "args": {"document": "dcdc-buck.snxblk", "path": "blocks.svg"}}),
        json!({"id": "export_block_diagram",
               "args": {"document": "dcdc-buck.snxblk", "path": "blocks.pdf",
                        "monochrome": true, "title_block": false}}),
    ]);
    let out = run_cli(&["run", project, "--allow", "export"], &commands);
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    let svg = &out.lines[1];
    assert_eq!(svg["status"], "answered", "{}", out.stdout);
    assert_eq!(svg["data"]["kind"], "svg", "{}", out.stdout);
    assert_eq!(svg["data"]["blocks"], 6, "{}", out.stdout);
    assert_eq!(svg["data"]["connections"], 6, "{}", out.stdout);
    let written = std::fs::read_to_string(fx.dir.join("blocks.svg")).expect("the SVG");
    assert!(written.contains(">DC/DC Buck</text>"));
    assert_eq!(svg["data"]["bytes"], written.len());

    let pdf = &out.lines[2];
    assert_eq!(pdf["status"], "answered", "{}", out.stdout);
    assert_eq!(pdf["data"]["kind"], "pdf", "{}", out.stdout);
    let bytes = std::fs::read(fx.dir.join("blocks.pdf")).expect("the PDF");
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn the_export_needs_its_capability_and_stays_inside_the_project() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::write(fx.dir.join("dcdc-buck.snxblk"), DCDC).expect("write the diagram");
    let project = fx.project.to_str().unwrap();
    let inside = jsonl(&[
        open_the_diagram(),
        json!({"id": "export_block_diagram",
            "args": {"document": "dcdc-buck.snxblk", "path": "blocks.svg"}}),
    ]);
    let out = run_cli(&["run", project], &inside);
    assert_eq!(out.lines[1]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[1]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("export"), "{reason}");
    assert!(!fx.dir.join("blocks.svg").exists(), "nothing written");

    let outside = root.join("outside.svg");
    let escape = jsonl(&[
        open_the_diagram(),
        json!({"id": "export_block_diagram",
            "args": {"document": "dcdc-buck.snxblk", "path": outside}}),
    ]);
    let out = run_cli(&["run", project, "--allow", "export"], &escape);
    assert_eq!(out.lines[1]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[1]["reason"].as_str().unwrap_or_default();
    assert!(
        !reason.contains("not an open or referenced document"),
        "refused for its path, not its document: {reason}"
    );
    assert!(!outside.exists(), "nothing written outside the project");
}
