//! Compare with board (#193d): a package Signex wrote for a board, read
//! back and compared with that board, then with edited copies of it.

use pcb_geom::PointNm;
use serde_json::{Value, json};
use signex_gerber::{
    CompareOptions, Comparison, CopperSide, DifferenceKind, FileFunction, Package, assemble,
    compare_with_board, read_file,
};
use signex_output::fab::{FabExtras, FabFileKind, FabFrame, FabOptions, generate};
use signex_types::pcb::{PcbBoard, Point};

fn id(n: u32) -> String {
    format!("00000000-0000-0000-0000-{n:012}")
}

fn pt(x: f64, y: f64) -> Value {
    json!({ "x": x, "y": y })
}

const THRU: [&str; 4] = [
    "TopCopper",
    "BottomCopper",
    "TopSolderMask",
    "BottomSolderMask",
];

fn thru_part(
    n: u32,
    reference: &str,
    at: (f64, f64),
    size: f64,
    drill: f64,
    net: (u32, &str),
) -> Value {
    json!({
        "uuid": id(n),
        "reference": reference,
        "position": pt(at.0, at.1),
        "layer": "TopCopper",
        "pads": [{
            "uuid": id(n + 1),
            "number": "1",
            "pad_type": "thru",
            "shape": "circle",
            "position": pt(0.0, 0.0),
            "size": pt(size, size),
            "drill": { "diameter": drill },
            "layers": THRU,
            "net": { "number": net.0, "name": net.1 },
        }],
    })
}

fn track(n: u32, from: (f64, f64), to: (f64, f64), width: f64, layer: &str, net: u32) -> Value {
    json!({
        "uuid": id(n),
        "start": pt(from.0, from.1),
        "end": pt(to.0, to.1),
        "width": width,
        "layer": layer,
        "net": net,
    })
}

/// The board the package was written from: 30 x 20 mm, two layers, J1
/// (drill 0.8) and J2 (drill 1.1), a VCC track on top and an unnamed
/// track on the bottom.
fn written_board() -> Value {
    json!({
        "uuid": id(1),
        "thickness": 1.6,
        "outline": [pt(0.0, 0.0), pt(30.0, 0.0), pt(30.0, 20.0), pt(0.0, 20.0)],
        "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" },
        ],
        "nets": [
            { "number": 1, "name": "GND" },
            { "number": 2, "name": "VCC" },
            { "number": 3, "name": "FB" },
        ],
        "footprints": [
            thru_part(10, "J1", (5.0, 10.0), 1.6, 0.8, (1, "GND")),
            thru_part(20, "J2", (25.0, 10.0), 2.0, 1.1, (2, "VCC")),
        ],
        "segments": [
            track(30, (16.0, 10.0), (24.0, 10.0), 0.25, "TopCopper", 2),
            track(31, (8.0, 15.0), (12.0, 15.0), 0.5, "BottomCopper", 0),
        ],
    })
}

fn board(value: Value) -> PcbBoard {
    serde_json::from_value(value).expect("the test board")
}

/// The board edited after the package was written: a new FB track on
/// top, the bottom track removed under a new keep-out, J2 drilled 1.0.
fn edited_board() -> PcbBoard {
    let mut value = written_board();
    value["footprints"][1]["pads"][0]["drill"]["diameter"] = json!(1.0);
    value["segments"] = json!([
        track(30, (16.0, 10.0), (24.0, 10.0), 0.25, "TopCopper", 2),
        track(32, (6.0, 3.0), (10.0, 3.0), 0.25, "TopCopper", 3),
    ]);
    value["zones"] = json!([{
        "uuid": id(40),
        "layer": "BottomCopper",
        "fill_type": "keepout",
        "outline": [pt(7.0, 14.0), pt(13.0, 14.0), pt(13.0, 16.0), pt(7.0, 16.0)],
    }]);
    board(value)
}

/// The Gerber, drill and job files Signex writes for `board`.
fn written_files(board: &PcbBoard) -> Vec<(String, String)> {
    let mut options = FabOptions::all("dcdc-buck");
    options.centroid = false;
    options.test_netlist = false;
    options.ipc2581 = false;
    options.odb = None;
    generate(board, &FabExtras::of_board(board), &options)
        .expect("the board has an outline")
        .files
        .into_iter()
        .filter(|file| {
            matches!(
                file.kind,
                FabFileKind::Gerber { .. } | FabFileKind::Drill { .. } | FabFileKind::Job
            )
        })
        .map(|file| (file.name, file.contents))
        .collect()
}

fn read_back(files: &[(String, String)]) -> Package {
    assemble(
        files
            .iter()
            .map(|(name, contents)| read_file(name, contents.as_bytes()))
            .collect(),
    )
}

/// A drawing with no X2 file function, as a fab adds to a package.
const FAB_NOTES: &str = "G04 fab notes*\n%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.120000*%\nD10*\n\
X0Y-21000000D02*\nX30000000Y-21000000D01*\nM02*\n";

/// `contents` with the aperture defined as `template` turned into the
/// undefined macro THERMAL80, so its flashes cannot be built.
fn break_aperture(contents: &str, template: &str) -> String {
    let suffix = format!("{template}*%");
    let mut broken = 0;
    let lines: Vec<String> = contents
        .lines()
        .map(|line| {
            let code = line
                .strip_prefix("%ADD")
                .and_then(|rest| rest.strip_suffix(suffix.as_str()))
                .filter(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()));
            match code {
                Some(code) => {
                    broken += 1;
                    format!("%ADD{code}THERMAL80*%")
                }
                None => line.to_string(),
            }
        })
        .collect();
    assert_eq!(broken, 1, "one aperture {template} in the file");
    lines.join("\n") + "\n"
}

fn layer_of(package: &Package, function: &FileFunction) -> usize {
    package
        .layers
        .iter()
        .position(|layer| layer.function.as_ref() == Some(function))
        .unwrap_or_else(|| panic!("no {function:?} layer"))
}

fn top() -> FileFunction {
    FileFunction::Copper {
        number: 1,
        side: CopperSide::Top,
    }
}

fn bottom() -> FileFunction {
    FileFunction::Copper {
        number: 2,
        side: CopperSide::Bottom,
    }
}

fn of_kind(comparison: &Comparison, kind: DifferenceKind) -> Vec<&signex_gerber::Difference> {
    comparison
        .differences
        .iter()
        .filter(|d| d.kind == kind)
        .collect()
}

#[test]
fn a_package_compared_with_its_own_board_has_no_differences() {
    let board = board(written_board());
    let package = read_back(&written_files(&board));
    assert!(package.problems().is_empty(), "{:#?}", package.problems());
    let comparison = compare_with_board(&package, &board, &CompareOptions::default(), &[]);
    assert_eq!(comparison.differences, Vec::new());
    assert_eq!(comparison.matching.len(), package.layers.len());
    for (index, found) in comparison.matching.iter().enumerate() {
        assert_eq!(found.layer, index);
        assert!(!found.by_hand);
        assert_eq!(
            found.board, package.layers[index].function,
            "{} is matched with its own layer",
            package.layers[index].image.file
        );
    }
}

#[test]
fn the_overlay_is_the_board_in_the_exporter_frame() {
    let board = edited_board();
    let package = read_back(&written_files(&board));
    let comparison = compare_with_board(&package, &board, &CompareOptions::default(), &[]);
    let frame = FabFrame::of(&board);
    let outline: Vec<PointNm> = board
        .outline
        .iter()
        .map(|p| frame.of_mm(*p, "outline").expect("a finite outline"))
        .collect();
    assert_eq!(comparison.overlay.outline, vec![outline]);
    let keepout: Vec<PointNm> = [(7.0, 14.0), (13.0, 14.0), (13.0, 16.0), (7.0, 16.0)]
        .iter()
        .map(|&(x, y)| frame.of_mm(Point::new(x, y), "keepout").expect("finite"))
        .collect();
    assert_eq!(comparison.overlay.keepouts, vec![keepout]);
    let layers: Vec<&FileFunction> = comparison.overlay.copper.iter().map(|(f, _)| f).collect();
    assert_eq!(layers, vec![&top(), &bottom()]);
    assert!(
        comparison
            .overlay
            .copper
            .iter()
            .all(|(_, shapes)| !shapes.is_empty())
    );
}

#[test]
fn an_edited_board_lists_each_difference_once() {
    let mut files = written_files(&board(written_board()));
    let l1 = files
        .iter_mut()
        .find(|(name, _)| name.ends_with("-copper-l1-top.gbr"))
        .expect("the L1 file");
    l1.1 = break_aperture(&l1.1, "C,1.6");
    files.push(("fab-notes.gbr".to_string(), FAB_NOTES.to_string()));
    let package = read_back(&files);
    let comparison = compare_with_board(&package, &edited_board(), &CompareOptions::default(), &[]);
    let top_layer = layer_of(&package, &top());
    let bottom_layer = layer_of(&package, &bottom());

    let missing = of_kind(&comparison, DifferenceKind::MissingCopper);
    assert_eq!(missing.len(), 1, "{missing:#?}");
    assert_eq!(missing[0].layer, top_layer);
    assert_eq!(missing[0].board_layer, Some(top()));
    assert_eq!(missing[0].net.as_deref(), Some("FB"));
    assert_eq!(missing[0].source, None, "only the board has it");

    let extra = of_kind(&comparison, DifferenceKind::ExtraCopper);
    assert_eq!(extra.len(), 1, "{extra:#?}");
    assert_eq!(extra[0].layer, bottom_layer);
    assert!(extra[0].inside_keepout);
    assert_eq!(extra[0].net, None);
    let source = extra[0].source.as_ref().expect("the package line");
    assert!(source.file.ends_with("-copper-l2-bottom.gbr"), "{source:?}");

    let drills = of_kind(&comparison, DifferenceKind::DrillSizeMismatch);
    assert_eq!(drills.len(), 1, "{drills:#?}");
    assert_eq!(drills[0].size_in_file_nm, Some(1_100_000));
    assert_eq!(drills[0].size_on_board_nm, Some(1_000_000));
    assert_eq!(drills[0].pin, Some(("J2".to_string(), "1".to_string())));
    assert_eq!(drills[0].net.as_deref(), Some("VCC"));

    let unmatched = of_kind(&comparison, DifferenceKind::UnmatchedLayer);
    assert_eq!(unmatched.len(), 1, "{unmatched:#?}");
    let notes = &package.layers[unmatched[0].layer];
    assert_eq!(notes.image.file, "fab-notes.gbr");
    assert_eq!(notes.file_label.as_deref(), Some("Fab notes"));

    // The J1 pad on L1 cannot be built: it is Not readable, never missing.
    let unreadable = of_kind(&comparison, DifferenceKind::NotReadable);
    assert_eq!(unreadable.len(), 1, "{unreadable:#?}");
    assert_eq!(unreadable[0].layer, top_layer);
    assert_eq!(unreadable[0].pin, Some(("J1".to_string(), "1".to_string())));
    assert!(unreadable[0].source.is_some());
    assert!(
        missing
            .iter()
            .all(|d| d.pin.as_ref().map(|p| p.0.as_str()) != Some("J1")),
        "the unreadable pad is not missing copper"
    );

    assert_eq!(
        comparison.differences.len(),
        5,
        "{:#?}",
        comparison.differences
    );
    let order: Vec<(usize, DifferenceKind)> = comparison
        .differences
        .iter()
        .map(|d| (d.layer, d.kind))
        .collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "by layer, then kind");
}

#[test]
fn a_shift_within_the_tolerance_is_not_a_difference() {
    let package = read_back(&written_files(&board(written_board())));
    let mut shifted = written_board();
    shifted["segments"][0] = track(30, (16.0, 10.005), (24.0, 10.005), 0.25, "TopCopper", 2);
    let shifted = board(shifted);
    let default = compare_with_board(&package, &shifted, &CompareOptions::default(), &[]);
    assert_eq!(default.differences, Vec::new(), "5 µm is within 0.010 mm");
    let tight = CompareOptions {
        tolerance_nm: 1_000,
    };
    let strict = compare_with_board(&package, &shifted, &tight, &[]);
    let kinds: Vec<DifferenceKind> = strict.differences.iter().map(|d| d.kind).collect();
    assert_eq!(
        kinds,
        vec![DifferenceKind::MissingCopper, DifferenceKind::ExtraCopper]
    );
    assert!(
        strict
            .differences
            .iter()
            .all(|d| d.net.as_deref() == Some("VCC"))
    );
}

#[test]
fn a_layer_matched_by_hand_overrides_its_function() {
    let mut files = written_files(&board(written_board()));
    files.push(("fab-notes.gbr".to_string(), FAB_NOTES.to_string()));
    let package = read_back(&files);
    let notes = package
        .layers
        .iter()
        .position(|layer| layer.image.file == "fab-notes.gbr")
        .expect("the drawing");
    let top_layer = layer_of(&package, &top());
    let by_hand = [(notes, Some(top())), (top_layer, None)];
    let comparison = compare_with_board(
        &package,
        &board(written_board()),
        &CompareOptions::default(),
        &by_hand,
    );
    assert_eq!(comparison.matching[notes].board, Some(top()));
    assert!(comparison.matching[notes].by_hand);
    assert_eq!(comparison.matching[top_layer].board, None);
    assert!(comparison.matching[top_layer].by_hand);
    let unmatched: Vec<usize> = of_kind(&comparison, DifferenceKind::UnmatchedLayer)
        .iter()
        .map(|d| d.layer)
        .collect();
    assert_eq!(unmatched, vec![top_layer]);
    assert!(
        of_kind(&comparison, DifferenceKind::ExtraCopper)
            .iter()
            .any(|d| d.layer == notes),
        "the notes' line is not on the board's L1"
    );
}

#[test]
fn a_board_with_no_outline_leaves_every_layer_unmatched() {
    let package = read_back(&written_files(&board(written_board())));
    let mut value = written_board();
    value["outline"] = json!([]);
    let comparison = compare_with_board(&package, &board(value), &CompareOptions::default(), &[]);
    assert_eq!(comparison.matching.len(), package.layers.len());
    assert!(comparison.matching.iter().all(|m| m.board.is_none()));
    assert_eq!(comparison.differences.len(), package.layers.len());
    assert!(
        comparison
            .differences
            .iter()
            .all(|d| d.kind == DifferenceKind::UnmatchedLayer)
    );
    assert!(comparison.overlay.outline.is_empty());
}
