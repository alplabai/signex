//! End to end (#1299): an Export Job through the real `signex-cli` binary
//! writes its PDF, PNG, BOM and netlist set headless, a second run writes
//! the same bytes, and a run that may not export writes nothing.

mod support;

use serde_json::{Value, json};
use signex_types::output_job::{JobOutput, OutputJob, OutputKind, write_output_job};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root};

fn job() -> OutputJob {
    let mut pages = JobOutput::new("Pages", OutputKind::SchematicPng);
    pages.dpi = Some(40);
    OutputJob {
        outputs: vec![
            JobOutput::new("PDF", OutputKind::SchematicPdf),
            pages,
            JobOutput::new("BOM", OutputKind::Bom),
            JobOutput::new("Netlist", OutputKind::Netlist),
        ],
        ..OutputJob::default()
    }
}

fn commands() -> String {
    jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "run_output_job", "args": {"path": "fab.snxout"}}),
    ])
}

#[test]
fn an_export_job_writes_its_set_headless_and_again_byte_for_byte() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_output_job(&fx.dir.join("fab.snxout"), &job()).expect("the job file");
    let project = fx.project.to_str().unwrap();

    let out = run_cli(&["run", project, "--allow", "export"], commands());
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    let answer = &out.lines[1];
    assert_eq!(answer["status"], "answered", "{}", out.stdout);
    let names: Vec<&str> = answer["data"]["outputs"]
        .as_array()
        .expect("outputs")
        .iter()
        .filter_map(|output| output["name"].as_str())
        .collect();
    assert_eq!(names, ["PDF", "Pages", "BOM", "Netlist"]);
    assert_eq!(answer["data"]["variant"], "Base");

    let folder = fx.dir.join("out");
    let first = snapshot(&folder);
    let mut files: Vec<String> = first
        .keys()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_string))
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["demo-1.png", "demo-bom.csv", "demo.NET", "demo.pdf"]
    );
    let netlist = &first[&folder.join("demo.NET")];
    assert!(netlist.starts_with(b"["), "a .NET netlist");

    let again = run_cli(&["run", project, "--allow", "export"], commands());
    assert_eq!(again.lines[1]["status"], "answered", "{}", again.stdout);
    assert_eq!(snapshot(&folder), first, "two runs give the same files");
}

#[test]
fn one_output_by_name_and_a_run_without_export_rights() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_output_job(&fx.dir.join("fab.snxout"), &job()).expect("the job file");
    let project = fx.project.to_str().unwrap();

    let refused = run_cli(&["run", project], commands());
    assert_eq!(refused.lines[1]["status"], "refused", "{}", refused.stdout);
    let reason = refused.lines[1]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("--allow export"), "{reason}");
    assert!(!fx.dir.join("out").exists(), "nothing written");

    let one = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "run_output_job", "args": {"path": "fab.snxout", "name": "BOM"}}),
    ]);
    let out = run_cli(&["run", project, "--allow", "export"], one);
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    let outputs = &out.lines[1]["data"]["outputs"];
    assert_eq!(
        outputs,
        &json!([{"name": "BOM", "files": ["demo-bom.csv"]}]),
        "{}",
        out.stdout
    );
    let written: Vec<Value> = snapshot(&fx.dir.join("out"))
        .keys()
        .map(|path| json!(path.file_name().and_then(|n| n.to_str())))
        .collect();
    assert_eq!(written, [json!("demo-bom.csv")]);
}

#[test]
fn the_pages_as_png_through_the_cli() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let lines = jsonl(&[
        json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
        json!({"id": "export_sheet_png",
               "args": {"document": "main.snxsch", "path": "png", "dpi": "40"}}),
    ]);
    let out = run_cli(&["run", project, "--allow", "export"], lines);
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(
        out.lines[1]["data"]["outputs"][0]["files"],
        json!(["demo-1.png"])
    );
    let png = std::fs::read(fx.dir.join("png").join("demo-1.png")).expect("the page");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
}
