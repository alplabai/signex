//! End to end (#2470): `fabrication_outputs` through the real `signex-cli`
//! binary writes the board's files into a folder of the project, answers
//! with each file's kind and SHA-256, and needs the `export` capability.

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::pcb::{NetDef, Point, Segment};
use support::{fixture, jsonl, run_cli, temp_root};

/// A 40 x 20 mm board with one GND track.
fn write_board(path: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = vec![
        Point::new(0.0, 0.0),
        Point::new(40.0, 0.0),
        Point::new(40.0, 20.0),
        Point::new(0.0, 20.0),
    ];
    board.nets = vec![NetDef {
        number: 1,
        name: "GND".into(),
    }];
    board.segments = vec![Segment {
        locked: false,
        attributes: Default::default(),
        uuid: uuid::Uuid::from_u128(0x2001),
        start: Point::new(5.0, 5.0),
        end: Point::new(30.0, 5.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
    }];
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write board");
}

fn set_board(project: &Path, filename: &str) {
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some(filename.into());
    signex_types::project::write_project(project, &data).expect("project");
}

fn commands() -> String {
    jsonl(&[
        json!({"id": "open_document", "args": {"path": "board.snxpcb"}}),
        json!({"id": "fabrication_outputs", "args": {
            "document": "board.snxpcb", "path": "fab", "kinds": "gerber,drill,centroid"
        }}),
    ])
}

#[test]
fn fabrication_outputs_writes_the_files_and_lists_them_with_their_hashes() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let project = fx.project.to_str().unwrap();

    let out = run_cli(&["run", project, "--allow", "export"], commands());
    let line = &out.lines[1];
    assert_eq!(line["status"], "answered", "{}\n{}", out.stdout, out.stderr);
    let data = &line["data"];
    assert_eq!(data["kinds"], json!(["gerber", "drill", "centroid"]));
    let files = data["files"].as_array().expect("files");
    assert!(
        files.iter().any(|f| f["kind"] == "gerber"),
        "{}",
        out.stdout
    );
    assert!(
        files
            .iter()
            .any(|f| f["name"] == "board-centroid.csv" && f["kind"] == "centroid"),
        "{}",
        out.stdout
    );
    // The app's tests check each hash against the file; here, its form.
    for file in files {
        let path = Path::new(file["path"].as_str().expect("path"));
        assert!(path.starts_with(fx.dir.join("fab")), "{path:?}");
        let size = std::fs::metadata(path).expect("written").len();
        assert_eq!(file["bytes"], size, "{path:?}");
        let hex = file["sha256"].as_str().expect("sha256");
        assert!(
            hex.len() == 64
                && hex
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{hex}"
        );
    }
    assert_eq!(out.code, Some(0), "{}", out.stderr);
}

#[test]
fn fabrication_outputs_needs_the_export_capability() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let project = fx.project.to_str().unwrap();

    let out = run_cli(&["run", project], commands());
    assert_eq!(out.lines[1]["status"], "refused", "{}", out.stdout);
    assert_eq!(
        out.lines[1]["reason"],
        "`fabrication_outputs` needs the `export` capability; pass --allow export"
    );
    assert!(!fx.dir.join("fab").exists(), "nothing written");
}
