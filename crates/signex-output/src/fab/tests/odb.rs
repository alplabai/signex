//! The ODB++ job (#2485) on a real board and on the fixture board.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_types::format::SnxPcb;
use signex_types::pcb::{BoardGraphic, LayerDef, PcbBoard};

use super::fixture::fixture_board;
use super::{FabExtras, FabOptions, golden_path};
use crate::fab::odb::read_archive;
use crate::fab::{FabFileKind, OdbForm, generate};

fn real_board(name: &str) -> PcbBoard {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real")
        .join(format!("{name}.snxpcb"));
    let text = std::fs::read_to_string(&path).expect("the corpus board");
    SnxPcb::parse(&text).expect("board parses").board
}

fn options(name: &str, form: OdbForm) -> FabOptions {
    FabOptions {
        gerber: false,
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: Some(form),
        ..FabOptions::all(name)
    }
}

/// The job as a folder: each file's path under `<name>-odb/` and its text.
fn job(board: &PcbBoard, extras: &FabExtras, name: &str) -> BTreeMap<String, String> {
    let set = generate(board, extras, &options(name, OdbForm::Directory)).expect("generates");
    let prefix = format!("{name}-odb/");
    set.files
        .iter()
        .filter(|f| f.kind == FabFileKind::Odb)
        .map(|f| {
            let path = f.name.strip_prefix(&prefix).expect("under the job folder");
            (path.to_string(), f.contents.clone())
        })
        .collect()
}

/// Every file of a job, each under a `=== <path>` line.
fn listing(job: &BTreeMap<String, String>) -> String {
    job.iter()
        .map(|(path, text)| format!("=== {path}\n{text}"))
        .collect()
}

#[test]
fn the_four_layer_sensor_board_matches_its_golden_listing() {
    let board = real_board("sensor-4layer");
    let actual = listing(&job(&board, &FabExtras::of_board(&board), "sensor-4layer"));
    let path = golden_path("sensor-4layer-odb.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).expect("golden written");
        return;
    }
    let expected = std::fs::read_to_string(&path).expect("golden file");
    assert_eq!(actual, expected, "the ODB++ job differs from its golden");
}

#[test]
fn the_archive_holds_the_folder_under_the_job_name() {
    let board = real_board("sensor-4layer");
    let extras = FabExtras::of_board(&board);
    let set = generate(&board, &extras, &options("Sensor 4L", OdbForm::Archive)).expect("ok");
    let archive = set
        .files
        .iter()
        .find(|f| f.kind == FabFileKind::Odb)
        .expect("the archive");
    assert_eq!(archive.name, "Sensor 4L-odb.tar");
    let read: BTreeMap<String, String> = read_archive(&archive.contents).into_iter().collect();
    let folder: BTreeMap<String, String> = job(&board, &extras, "Sensor 4L")
        .into_iter()
        .map(|(path, text)| (format!("sensor_4l/{path}"), text))
        .collect();
    assert_eq!(read, folder);
    let gbrjob = set.files.last().expect("the job file");
    assert!(gbrjob.contents.contains("\"Sensor 4L-odb.tar\""));
    assert!(gbrjob.contents.contains("\"ODB++\""));
}

/// The matrix's layers: each `NAME` with its `CONTEXT`, `TYPE`,
/// `START_NAME` and `END_NAME`, in row order.
fn matrix(job: &BTreeMap<String, String>) -> Vec<[String; 5]> {
    let text = &job["matrix/matrix"];
    text.split("LAYER {")
        .skip(1)
        .map(|block| {
            let value = |key: &str| {
                block
                    .lines()
                    .find_map(|l| l.trim().strip_prefix(&format!("{key}=")))
                    .unwrap_or_default()
                    .to_string()
            };
            ["NAME", "CONTEXT", "TYPE", "START_NAME", "END_NAME"].map(value)
        })
        .collect()
}

#[test]
fn the_layers_take_their_type_from_the_typed_layer_in_physical_order() {
    let board = real_board("sensor-4layer");
    let job = job(&board, &FabExtras::of_board(&board), "s");
    let layers = matrix(&job);
    let row = |name: &str| {
        layers
            .iter()
            .position(|l| l[0] == name)
            .unwrap_or_else(|| panic!("{name} in {layers:?}"))
    };
    let kind = |name: &str| layers[row(name)][1..3].join(" ");
    assert_eq!(kind("topcopper"), "BOARD SIGNAL");
    // The layer table names Inner 1 "GND plane": a display name, which the
    // job never reads; its plane type does.
    assert_eq!(kind("inner1copper"), "BOARD POWER_GROUND");
    assert_eq!(kind("topsoldermask"), "BOARD SOLDER_MASK");
    assert_eq!(kind("bottomsilkscreen"), "BOARD SILK_SCREEN");
    assert_eq!(kind("topcourtyard"), "MISC DOCUMENT");
    assert_eq!(kind("dielectric1"), "BOARD DIELECTRIC");
    assert_eq!(kind("comp_+_top"), "BOARD COMPONENT");
    assert_eq!(
        layers[row("drill_pth")][1..].join(" "),
        "BOARD DRILL topcopper bottomcopper"
    );
    assert!(
        !layers.iter().any(|l| l[0].contains("edge")),
        "the outline is the profile"
    );
    assert!(row("comp_+_top") < row("topsilkscreen"));
    assert!(row("topsoldermask") < row("topcopper"));
    assert!(row("topcopper") < row("dielectric1"));
    assert!(row("dielectric1") < row("inner1copper"));
    assert!(row("bottomcopper") < row("bottomsoldermask"));
    for layer in &layers {
        let dir = format!("steps/pcb/layers/{}", layer[0]);
        let has = |file: &str| job.contains_key(&format!("{dir}/{file}"));
        assert!(has("features") || has("components"), "{dir}");
    }
    assert!(job["steps/pcb/layers/drill_pth/tools"].contains("TYPE=PLATED"));
    assert!(!job["matrix/matrix"].to_lowercase().contains("gnd plane"));
}

#[test]
fn every_toeprint_and_feature_reference_resolves() {
    let board = real_board("sensor-4layer");
    let job = job(&board, &FabExtras::of_board(&board), "s");
    let data = &job["steps/pcb/eda/data"];
    let layers: Vec<&str> = data
        .lines()
        .find_map(|l| l.strip_prefix("LYR "))
        .expect("LYR")
        .split(' ')
        .collect();
    let mut nets: Vec<Vec<String>> = Vec::new();
    for line in data.lines() {
        if line.starts_with("NET ") {
            nets.push(Vec::new());
        } else if line.starts_with("SNT ") {
            nets.last_mut().expect("a net").push(line.to_string());
        } else if let Some(fid) = line.strip_prefix("FID C ") {
            let [layer, feature] = [0, 1].map(|i| {
                fid.split(' ')
                    .nth(i)
                    .and_then(|n| n.parse::<usize>().ok())
                    .expect("number")
            });
            let features = &job[&format!("steps/pcb/layers/{}/features", layers[layer])];
            let records = features
                .lines()
                .filter(|l| ["P ", "L ", "A ", "S "].iter().any(|r| l.starts_with(r)))
                .count();
            assert!(feature < records, "FID C {layer} {feature}");
        }
    }
    let mut placed = 0;
    for (side, file) in [("T", "comp_+_top"), ("B", "comp_+_bot")] {
        let Some(components) = job.get(&format!("steps/pcb/layers/{file}/components")) else {
            continue;
        };
        let mut on_side = 0;
        let mut toeprint = 0;
        for line in components.lines() {
            let fields: Vec<&str> = line.split(' ').collect();
            let component = on_side.max(1) - 1;
            match fields[0] {
                "CMP" => {
                    assert_eq!(fields.len(), 8, "{line}");
                    on_side += 1;
                    placed += 1;
                    toeprint = 0;
                }
                "TOP" => {
                    let net: usize = fields[6].parse().expect("net");
                    let subnet: usize = fields[7].parse().expect("subnet");
                    assert_eq!(
                        nets[net][subnet],
                        format!("SNT TOP {side} {component} {toeprint}"),
                        "{line}"
                    );
                    toeprint += 1;
                }
                _ => {}
            }
        }
    }
    let designated = board
        .footprints
        .iter()
        .filter(|f| !f.reference.is_empty() && !f.pads.is_empty())
        .count();
    assert_eq!(placed, designated);
    assert!(data.contains("\nNET GND\n"), "{data}");
}

#[test]
fn a_mechanical_layer_is_a_document_layer_that_keeps_its_name() {
    let mut board = fixture_board();
    board.layers.push(LayerDef {
        id: 72,
        name: "Mechanical3".into(),
        layer_type: "user".into(),
        display_name: "Fold Lines".into(),
    });
    let line: BoardGraphic = serde_json::from_value(serde_json::json!({
        "graphic_type": "line", "layer": "Mechanical3", "width": 0.2,
        "start": {"x": 1.0, "y": 1.0}, "end": {"x": 5.0, "y": 1.0}
    }))
    .expect("a line");
    board.graphics.push(line);
    let job = job(&board, &FabExtras::default(), "fab");
    let layers = matrix(&job);
    assert!(
        layers
            .iter()
            .any(|l| l[..3] == ["fold_lines", "MISC", "DOCUMENT"]),
        "{layers:?}"
    );
    let features = &job["steps/pcb/layers/fold_lines/features"];
    assert!(features.contains("$0 r200\n"), "{features}");
    assert!(features.contains("\nL "), "{features}");
}

#[test]
fn one_board_gives_the_same_bytes_and_the_folder_is_listed_once() {
    let board = fixture_board();
    let first = job(&board, &FabExtras::default(), "fab");
    assert_eq!(first, job(&board, &FabExtras::default(), "fab"));
    assert!(first["misc/info"].contains("JOB_NAME=fab\n"));
    assert!(first["steps/pcb/profile"].contains("\nS P 0\nOB "));
    let set = generate(
        &board,
        &FabExtras::default(),
        &options("fab", OdbForm::Directory),
    )
    .expect("generates");
    let gbrjob = set.files.last().expect("the job file");
    assert_eq!(gbrjob.contents.matches("\"fab-odb\"").count(), 1);
}
