//! The IPC-2581 file (#2484) on a real board and on the fixture board.

use std::path::{Path, PathBuf};

use signex_types::format::SnxPcb;
use signex_types::pcb::{BoardGraphic, LayerDef, PcbBoard};

use super::fixture::fixture_board;
use super::{FabExtras, FabOptions, golden_path};
use crate::fab::{FabFileKind, generate};

fn real_board(name: &str) -> PcbBoard {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real")
        .join(format!("{name}.snxpcb"));
    let text = std::fs::read_to_string(&path).expect("the corpus board");
    SnxPcb::parse(&text).expect("board parses").board
}

fn ipc2581(board: &PcbBoard, extras: &FabExtras, name: &str) -> String {
    let options = FabOptions {
        gerber: false,
        drill: false,
        centroid: false,
        test_netlist: false,
        ..FabOptions::all(name)
    };
    let set = generate(board, extras, &options).expect("generates");
    let file = set
        .files
        .iter()
        .find(|f| f.kind == FabFileKind::Ipc2581)
        .expect("the IPC-2581 file");
    assert_eq!(file.name, format!("{name}-ipc2581.xml"));
    file.contents.clone()
}

#[test]
fn the_four_layer_sensor_board_matches_its_golden_file() {
    let board = real_board("sensor-4layer");
    let actual = ipc2581(&board, &FabExtras::of_board(&board), "sensor-4layer");
    let path = golden_path("sensor-4layer-ipc2581.xml");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).expect("golden written");
        return;
    }
    let expected = std::fs::read_to_string(&path).expect("golden file");
    assert_eq!(actual, expected, "the IPC-2581 file differs from its golden");
}

#[test]
fn the_layers_take_their_function_from_the_typed_layer() {
    let board = real_board("sensor-4layer");
    let xml = ipc2581(&board, &FabExtras::of_board(&board), "s");
    let layer = |name: &str, function: &str, side: &str| {
        format!("<Layer name=\"{name}\" layerFunction=\"{function}\" side=\"{side}\"")
    };
    for expected in [
        layer("TopCopper", "SIGNAL", "TOP"),
        // The layer table names Inner 1 "GND plane": a display name, which
        // the file never reads; its plane type does.
        layer("Inner1Copper", "PLANE", "INTERNAL"),
        layer("BottomCopper", "SIGNAL", "BOTTOM"),
        layer("TopSolderMask", "SOLDERMASK", "TOP"),
        layer("BottomSilkscreen", "LEGEND", "BOTTOM"),
        layer("EdgeCuts", "BOARD_OUTLINE", "ALL"),
        layer("TopCourtyard", "COURTYARD", "TOP"),
        layer("Dielectric1", "DIELCORE", "INTERNAL"),
        layer("Drill_PTH", "DRILL", "ALL"),
    ] {
        assert!(xml.contains(&expected), "{expected}");
    }
    assert!(!xml.contains("GND plane"));
    assert!(xml.contains("<Span fromLayer=\"TopCopper\" toLayer=\"BottomCopper\"/>"));
    let stack_rows = xml.matches("<StackupLayer ").count();
    assert_eq!(stack_rows, board_stack_rows(&board), "one row per stack row");
}

fn board_stack_rows(board: &PcbBoard) -> usize {
    crate::fab::roles::board_stack(board).layers.len()
}

#[test]
fn every_designated_part_is_placed_listed_and_wired() {
    let board = real_board("sensor-4layer");
    let xml = ipc2581(&board, &FabExtras::of_board(&board), "s");
    let designated = board
        .footprints
        .iter()
        .filter(|f| !f.reference.is_empty())
        .count();
    assert_eq!(xml.matches("<Component ").count(), designated);
    assert_eq!(xml.matches("<RefDes ").count(), designated);
    assert!(xml.contains("<BomRef name=\"s_BOM\"/>"));
    assert!(xml.contains("<LogicalNet name=\"GND\">"), "{xml}");
    assert!(xml.contains("<PinRef componentRef=\"J1\" pin=\"2\"/>"));
    assert!(xml.contains("refDes=\"U1\" packageRef=\"LGA-8_2.5x2.5mm\""));
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
    let xml = ipc2581(&board, &FabExtras::default(), "fab");
    assert!(
        xml.contains(
            "<Layer name=\"Fold Lines\" layerFunction=\"DOCUMENT\" side=\"ALL\" polarity=\"POSITIVE\"/>"
        ),
        "{xml}"
    );
    assert!(xml.contains("<LayerFeature layerRef=\"Fold Lines\">"));
    assert!(xml.contains("<LineDesc lineEnd=\"ROUND\" lineWidth=\"0.2\"/>"));
}

#[test]
fn one_board_gives_the_same_bytes_and_a_well_nested_file() {
    let board = fixture_board();
    let first = ipc2581(&board, &FabExtras::default(), "fab");
    assert_eq!(first, ipc2581(&board, &FabExtras::default(), "fab"));
    assert!(first.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<IPC-2581 revision=\"C\""));
    assert!(first.ends_with("</IPC-2581>\n"));
    let mut open: Vec<&str> = Vec::new();
    for line in first.lines().skip(1).map(str::trim) {
        if let Some(tag) = line.strip_prefix("</") {
            assert_eq!(open.pop(), tag.strip_suffix('>'), "{line}");
        } else if !line.ends_with("/>") {
            let tag = line[1..].split([' ', '>']).next().unwrap_or_default();
            open.push(tag);
        }
    }
    assert!(open.is_empty(), "{open:?}");
}
