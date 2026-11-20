//! The package model (mockup row 193): layer names from X2, the job file or the file
//! name, the default drawing order, top / bottom pairs, and every problem
//! kept with its file and line.

use serde_json::{Value, json};
use signex_gerber::{
    CopperSide, DrillKind, FileFunction, NameSource, Package, ProblemKind, Side, assemble,
    read_file,
};
use signex_output::fab::{FabExtras, FabFileKind, FabOptions, generate};
use signex_types::pcb::PcbBoard;

fn id(n: u32) -> String {
    format!("00000000-0000-0000-0000-{n:012}")
}

fn pt(x: f64, y: f64) -> Value {
    json!({ "x": x, "y": y })
}

fn silk_line(n: u32, layer: &str) -> Value {
    json!({
        "uuid": id(n),
        "graphic_type": "line",
        "layer": layer,
        "width": 0.15,
        "start": pt(2.0, 18.0),
        "end": pt(10.0, 18.0),
    })
}

/// A four-layer board: a through-hole pin, a top SMD resistor (so there is
/// top paste and no bottom paste), a track on L2 and a legend line on
/// each side.
fn four_layer_board() -> PcbBoard {
    let value = json!({
        "uuid": id(1),
        "thickness": 1.6,
        "outline": [pt(0.0, 0.0), pt(30.0, 0.0), pt(30.0, 20.0), pt(0.0, 20.0)],
        "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 1, "name": "Inner1Copper", "layer_type": "signal" },
            { "id": 2, "name": "Inner2Copper", "layer_type": "signal" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" },
        ],
        "nets": [{ "number": 1, "name": "GND" }, { "number": 2, "name": "VIN_12V" }],
        "footprints": [
            {
                "uuid": id(10),
                "reference": "J1",
                "position": pt(5.0, 10.0),
                "layer": "TopCopper",
                "pads": [{
                    "uuid": id(11),
                    "number": "1",
                    "pad_type": "thru",
                    "shape": "circle",
                    "position": pt(0.0, 0.0),
                    "size": pt(1.6, 1.6),
                    "drill": { "diameter": 0.8 },
                    "layers": ["TopCopper", "BottomCopper", "TopSolderMask", "BottomSolderMask"],
                    "net": { "number": 1, "name": "GND" },
                }],
            },
            {
                "uuid": id(20),
                "reference": "R1",
                "position": pt(15.0, 10.0),
                "layer": "TopCopper",
                "pads": [{
                    "uuid": id(21),
                    "number": "1",
                    "pad_type": "smd",
                    "shape": "rect",
                    "position": pt(0.0, 0.0),
                    "size": pt(1.0, 1.2),
                    "layers": ["TopCopper", "TopSolderMask", "TopPaste"],
                    "net": { "number": 2, "name": "VIN_12V" },
                }],
            },
        ],
        "segments": [{
            "uuid": id(30),
            "start": pt(6.0, 10.0),
            "end": pt(14.0, 10.0),
            "width": 0.25,
            "layer": "Inner1Copper",
            "net": 1,
        }],
        "graphics": [silk_line(40, "TopSilkscreen"), silk_line(41, "BottomSilkscreen")],
    });
    serde_json::from_value(value).expect("the test board")
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

/// A Gerber drawing with no X2 file function.
fn drawing() -> String {
    "G04 no file function*\n%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.120000*%\nD10*\n\
     X0Y0D02*\nX1000000Y0D01*\nM02*\n"
        .to_string()
}

fn copper(number: u32, side: CopperSide) -> FileFunction {
    FileFunction::Copper { number, side }
}

fn layer_named(package: &Package, ending: &str) -> usize {
    package
        .layers
        .iter()
        .position(|layer| layer.image.file.ends_with(ending))
        .unwrap_or_else(|| panic!("no file ending {ending}"))
}

#[test]
fn x2_names_a_layer_and_a_file_name_names_a_drawing() {
    let mut files = written_files(&four_layer_board());
    files.push(("fab-notes.gbr".to_string(), drawing()));
    files.push(("board-silk-bottom.gbr".to_string(), drawing()));
    let package = read_back(&files);
    for layer in &package.layers {
        if layer.image.file.starts_with("dcdc-buck-") {
            assert_eq!(layer.name_source, NameSource::X2, "{}", layer.image.file);
            assert_eq!(layer.file_label, None);
            assert!(layer.function.is_some());
        }
    }
    let notes = &package.layers[layer_named(&package, "fab-notes.gbr")];
    assert_eq!(notes.name_source, NameSource::FileName);
    assert_eq!(notes.file_label.as_deref(), Some("Fab notes"));
    assert_eq!(notes.function, None, "drawn as a drawing");
    assert_eq!(notes.pair, None);
    let silk = &package.layers[layer_named(&package, "board-silk-bottom.gbr")];
    assert_eq!(silk.name_source, NameSource::FileName);
    assert_eq!(silk.file_label.as_deref(), Some("Board silk bottom"));
    assert_eq!(silk.function, Some(FileFunction::Legend(Side::Bottom)));
}

#[test]
fn a_file_without_x2_is_named_by_the_job_file_and_the_warning_stays() {
    let mut files = written_files(&four_layer_board());
    let l2 = files
        .iter_mut()
        .find(|(name, _)| name.ends_with("-copper-l2-inner1.gbr"))
        .expect("the L2 file");
    let stripped: Vec<&str> =
        l2.1.lines()
            .filter(|line| !line.starts_with("%TF.FileFunction"))
            .collect();
    l2.1 = stripped.join("\n") + "\n";
    let l2_name = l2.0.clone();
    let package = read_back(&files);
    let layer = &package.layers[layer_named(&package, "-copper-l2-inner1.gbr")];
    assert_eq!(layer.name_source, NameSource::JobFile);
    assert_eq!(layer.function, Some(copper(2, CopperSide::Inner)));
    assert_eq!(layer.file_label, None);
    let warnings: Vec<_> = package
        .problems()
        .into_iter()
        .filter(|p| p.kind == ProblemKind::NoFileFunction)
        .collect();
    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert_eq!(warnings[0].at.file, l2_name);
    assert!(warnings[0].at.line >= 1);
}

#[test]
fn the_default_order_puts_overlays_on_top_then_the_stack_from_the_top() {
    let mut files = written_files(&four_layer_board());
    files.push(("fab-notes.gbr".to_string(), drawing()));
    let package = read_back(&files);
    let functions: Vec<Option<FileFunction>> =
        package.layers.iter().map(|l| l.function.clone()).collect();
    assert_eq!(
        functions,
        vec![
            Some(FileFunction::Profile { plated: false }),
            Some(FileFunction::Drill {
                plated: true,
                from: 1,
                to: 4,
                kind: DrillKind::Pth
            }),
            None,
            Some(FileFunction::Legend(Side::Top)),
            Some(FileFunction::Paste(Side::Top)),
            Some(FileFunction::Soldermask(Side::Top)),
            Some(copper(1, CopperSide::Top)),
            Some(copper(2, CopperSide::Inner)),
            Some(copper(3, CopperSide::Inner)),
            Some(copper(4, CopperSide::Bottom)),
            Some(FileFunction::Soldermask(Side::Bottom)),
            Some(FileFunction::Legend(Side::Bottom)),
        ]
    );
    assert_eq!(
        package.default_order(),
        (0..functions.len()).collect::<Vec<_>>()
    );
    assert!(package.job.is_some());
}

#[test]
fn far_side_layers_pair_and_copper_pairs_by_position() {
    let package = read_back(&written_files(&four_layer_board()));
    let at = |function: FileFunction| {
        package
            .layers
            .iter()
            .position(|l| l.function.as_ref() == Some(&function))
            .unwrap_or_else(|| panic!("no {function:?}"))
    };
    let pair = |function: FileFunction| package.layers[at(function)].pair;
    assert_eq!(
        pair(copper(1, CopperSide::Top)),
        Some(at(copper(4, CopperSide::Bottom)))
    );
    assert_eq!(
        pair(copper(4, CopperSide::Bottom)),
        Some(at(copper(1, CopperSide::Top)))
    );
    assert_eq!(
        pair(copper(2, CopperSide::Inner)),
        Some(at(copper(3, CopperSide::Inner)))
    );
    assert_eq!(
        pair(FileFunction::Soldermask(Side::Top)),
        Some(at(FileFunction::Soldermask(Side::Bottom)))
    );
    assert_eq!(
        pair(FileFunction::Legend(Side::Bottom)),
        Some(at(FileFunction::Legend(Side::Top)))
    );
    assert_eq!(
        pair(FileFunction::Paste(Side::Top)),
        None,
        "no bottom paste"
    );
    assert_eq!(pair(FileFunction::Profile { plated: false }), None);
}

#[test]
fn files_that_cannot_be_read_are_listed_first_with_their_line() {
    let files = vec![
        read_file("notes.txt", b"hello\n"),
        read_file("blob.gbr", b"ok\n\xff\xfe"),
        read_file("fab-notes.gbr", drawing().as_bytes()),
    ];
    let package = assemble(files);
    assert_eq!(package.not_read.len(), 2, "{:#?}", package.not_read);
    assert_eq!(package.not_read[0].at.file, "notes.txt");
    assert_eq!(package.not_read[0].kind, ProblemKind::NotAFabricationFile);
    assert_eq!(package.not_read[1].at.file, "blob.gbr");
    assert_eq!(package.not_read[1].kind, ProblemKind::NotText);
    assert_eq!(package.not_read[1].at.line, 2);
    let problems = package.problems();
    assert_eq!(problems[0], &package.not_read[0]);
    assert_eq!(problems[1], &package.not_read[1]);
    assert!(problems[2..].iter().all(|p| p.at.file == "fab-notes.gbr"));
    assert!(
        problems
            .iter()
            .any(|p| p.kind == ProblemKind::NoFileFunction)
    );
    assert_eq!(package.layers.len(), 1);
    assert!(!package.has_net_attributes());
}

#[test]
fn x2_nets_are_noticed() {
    let package = read_back(&written_files(&four_layer_board()));
    assert!(package.has_net_attributes());
}
