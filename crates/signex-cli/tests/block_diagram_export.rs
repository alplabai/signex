//! End to end (#2796): `export_block_diagram` through the real `signex-cli`
//! binary writes a project's block diagram as SVG and PDF, answers once the
//! file is written, needs the `export` capability and writes nothing outside
//! the project.

mod support;

use serde_json::json;
use support::{fixture, jsonl, run_cli, temp_root};

const DCDC: &str = include_str!("../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

#[test]
fn the_cli_writes_a_block_diagram_as_svg_and_pdf() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    std::fs::write(fx.dir.join("dcdc-buck.snxblk"), DCDC).expect("write the diagram");
    let project = fx.project.to_str().unwrap();
    let commands = jsonl(&[
        json!({"id": "export_block_diagram",
               "args": {"document": "dcdc-buck.snxblk", "path": "blocks.svg"}}),
        json!({"id": "export_block_diagram",
               "args": {"document": "dcdc-buck.snxblk", "path": "blocks.pdf",
                        "monochrome": true, "title_block": false}}),
    ]);
    let out = run_cli(&["run", project, "--allow", "export"], &commands);
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    let svg = &out.lines[0];
    assert_eq!(svg["status"], "answered", "{}", out.stdout);
    assert_eq!(svg["data"]["kind"], "svg", "{}", out.stdout);
    assert_eq!(svg["data"]["blocks"], 6, "{}", out.stdout);
    assert_eq!(svg["data"]["connections"], 6, "{}", out.stdout);
    let written = std::fs::read_to_string(fx.dir.join("blocks.svg")).expect("the SVG");
    assert!(written.contains(">DC/DC Buck</text>"));
    assert_eq!(svg["data"]["bytes"], written.len());

    let pdf = &out.lines[1];
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
    let inside = jsonl(&[json!({"id": "export_block_diagram",
        "args": {"document": "dcdc-buck.snxblk", "path": "blocks.svg"}})]);
    let out = run_cli(&["run", project], &inside);
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    let reason = out.lines[0]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("export"), "{reason}");
    assert!(!fx.dir.join("blocks.svg").exists(), "nothing written");

    let outside = root.join("outside.svg");
    let escape = jsonl(&[json!({"id": "export_block_diagram",
        "args": {"document": "dcdc-buck.snxblk", "path": outside}})]);
    let out = run_cli(&["run", project, "--allow", "export"], &escape);
    assert_eq!(out.lines[0]["status"], "refused", "{}", out.stdout);
    assert!(!outside.exists(), "nothing written outside the project");
}
