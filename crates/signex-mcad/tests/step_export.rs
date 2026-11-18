//! STEP export (#2488): the file is ISO 10303-21 in the chosen schema, its
//! B-rep is closed and oriented, and it reads back through Signex's own
//! STEP importer with the geometry it was written from.

mod step_reader;

use std::collections::HashMap;

use signex_3d_model_importer::step::p21::parse_to_meshes;
use signex_mcad::{
    Hole, Solid, StepHeader, StepMesh, StepModel, StepSchema, StepSolid, board_body, write_step,
};
use step_reader::{StepFile, dot, newell};

const THICKNESS: f64 = 1.6;

fn header() -> StepHeader {
    StepHeader {
        file_name: "board.step".to_owned(),
        timestamp: "2026-11-18T10:00:00".to_owned(),
        system: "Signex test".to_owned(),
    }
}

fn model<'a>(
    schema: StepSchema,
    solids: Vec<StepSolid<'a>>,
    meshes: Vec<StepMesh>,
) -> StepModel<'a> {
    StepModel {
        schema,
        header: header(),
        product: "board".to_owned(),
        solids,
        meshes,
    }
}

/// A 40 x 20 board with two round holes and a 4 x 4 cut-out.
fn drilled_board() -> Solid {
    let outline = [[0.0, 0.0], [40.0, 0.0], [40.0, 20.0], [0.0, 20.0]];
    let holes = [
        Hole::Round {
            center: [10.0, 10.0],
            diameter: 3.2,
        },
        Hole::Round {
            center: [30.0, 10.0],
            diameter: 3.2,
        },
        Hole::Polygon(vec![[18.0, 8.0], [22.0, 8.0], [22.0, 12.0], [18.0, 12.0]]),
    ];
    board_body(&outline, &holes, THICKNESS).expect("board body")
}

/// An L-shaped board with two cut-outs: every face flat, every edge
/// straight.
fn flat_board() -> (Solid, f64) {
    let outline = [
        [0.0, 0.0],
        [30.0, 0.0],
        [30.0, 10.0],
        [12.0, 10.0],
        [12.0, 25.0],
        [0.0, 25.0],
    ];
    let cutouts = [
        vec![[2.0, 2.0], [6.0, 2.0], [6.0, 5.0], [2.0, 5.0]],
        vec![[20.0, 3.0], [26.0, 3.0], [23.0, 7.0]],
    ];
    let holes: Vec<Hole> = cutouts.iter().cloned().map(Hole::Polygon).collect();
    let solid = board_body(&outline, &holes, THICKNESS).expect("board body");
    let area = 30.0 * 10.0 + 12.0 * 15.0 - 12.0 - 12.0;
    let perimeter = 30.0 + 10.0 + 18.0 + 15.0 + 12.0 + 25.0 + 14.0 + (6.0 + 5.0 + 5.0);
    (solid, 2.0 * area + perimeter * THICKNESS)
}

fn bounds_of(points: impl Iterator<Item = [f64; 3]>) -> ([f64; 3], [f64; 3]) {
    points.fold(
        ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
        |(lo, hi), p| {
            (
                [0, 1, 2].map(|k| lo[k].min(p[k])),
                [0, 1, 2].map(|k| hi[k].max(p[k])),
            )
        },
    )
}

/// The box and total triangle area of what the importer reads back.
fn read_back(bytes: &[u8]) -> (([f64; 3], [f64; 3]), f64) {
    let text = std::str::from_utf8(bytes).expect("ASCII");
    let result = parse_to_meshes(text).expect("the importer reads it");
    let mut area = 0.0;
    let mut points = Vec::new();
    for mesh in &result.meshes {
        let p: Vec<[f64; 3]> = mesh
            .positions
            .chunks_exact(3)
            .map(|c| [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])])
            .collect();
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]];
            let (u, v) = (
                [0, 1, 2].map(|k| b[k] - a[k]),
                [0, 1, 2].map(|k| c[k] - a[k]),
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            area += dot(n, n).sqrt() / 2.0;
        }
        points.extend(p);
    }
    (bounds_of(points.into_iter()), area)
}

fn assert_close(got: [f64; 3], want: [f64; 3], tolerance: f64) {
    for k in 0..3 {
        assert!(
            (got[k] - want[k]).abs() <= tolerance,
            "{got:?} is not {want:?}"
        );
    }
}

#[test]
fn the_file_is_iso_10303_21_in_the_chosen_schema() {
    let board = drilled_board();
    let solids = || {
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }]
    };
    let ap214 = write_step(&model(StepSchema::Ap214, solids(), Vec::new())).expect("AP214");
    let text = std::str::from_utf8(&ap214).expect("ASCII");
    assert!(text.starts_with("ISO-10303-21;\nHEADER;\n"));
    assert!(text.ends_with("ENDSEC;\nEND-ISO-10303-21;\n"));
    assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));"));
    assert!(text.contains("SI_UNIT(.MILLI.,.METRE.)"));
    // The declared uncertainty is the writer's on-curve/on-surface
    // tolerance (1e-5 mm), so no accepted vertex breaks it.
    assert!(text.contains("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-5),"));

    let ap242 = write_step(&model(StepSchema::Ap242, solids(), Vec::new())).expect("AP242");
    let text = std::str::from_utf8(&ap242).expect("ASCII");
    assert!(text.contains("AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF"));

    let again = write_step(&model(StepSchema::Ap214, solids(), Vec::new())).expect("again");
    assert_eq!(ap214, again, "the same input writes the same bytes");
}

#[test]
fn every_face_is_written_on_its_exact_surface() {
    let board = drilled_board();
    let bytes = write_step(&model(
        StepSchema::Ap214,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        Vec::new(),
    ))
    .expect("STEP");
    let file = StepFile::parse(&bytes);
    assert_eq!(file.count("ADVANCED_FACE"), board.face_count());
    assert_eq!(file.count("EDGE_CURVE"), board.edge_count());
    assert_eq!(file.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(file.count("CLOSED_SHELL"), 1);
    let cylinders = file.of_kind("CYLINDRICAL_SURFACE");
    assert!(
        cylinders.len() >= 2,
        "two hole walls, got {}",
        cylinders.len()
    );
    assert!(file.count("CIRCLE") >= 4, "a rim per hole per face");
    let names: Vec<String> = file
        .of_kind("ADVANCED_FACE")
        .into_iter()
        .map(|f| file.name(f))
        .collect();
    for name in [
        "Top",
        "Bottom",
        "Hole 0 wall",
        "Hole 1 wall",
        "Cut-out 2 wall",
    ] {
        assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
    }
}

/// Every edge is used twice, once each way, once the bounds' flags are
/// applied: the shell is closed and consistently oriented.
fn assert_closed_and_oriented(file: &StepFile) {
    let mut uses: HashMap<u64, (u32, u32)> = HashMap::new();
    for face in file.of_kind("ADVANCED_FACE") {
        for (_, bound) in file.bounds(face) {
            for (edge, forward) in bound {
                let entry = uses.entry(edge).or_default();
                if forward {
                    entry.0 += 1;
                } else {
                    entry.1 += 1;
                }
            }
        }
    }
    assert_eq!(uses.len(), file.count("EDGE_CURVE"));
    for (edge, (forward, backward)) in uses {
        assert_eq!((forward, backward), (1, 1), "edge #{edge}");
    }
}

#[test]
fn the_shell_is_closed_and_consistently_oriented() {
    let board = drilled_board();
    let bytes = write_step(&model(
        StepSchema::Ap214,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        Vec::new(),
    ))
    .expect("STEP");
    assert_closed_and_oriented(&StepFile::parse(&bytes));
}

#[test]
fn flat_faces_face_out_and_their_bounds_turn_the_right_way() {
    let (board, _) = flat_board();
    let bytes = write_step(&model(
        StepSchema::Ap214,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        Vec::new(),
    ))
    .expect("STEP");
    let file = StepFile::parse(&bytes);
    assert_closed_and_oriented(&file);
    for face in file.of_kind("ADVANCED_FACE") {
        let normal = file.plane_normal(face).expect("every face is flat");
        match file.name(face).as_str() {
            "Top" => assert_close(normal, [0.0, 0.0, 1.0], 1e-9),
            "Bottom" => assert_close(normal, [0.0, 0.0, -1.0], 1e-9),
            _ => assert!(normal[2].abs() < 1e-9),
        }
        for (outer, uses) in file.bounds(face) {
            let ring: Vec<[f64; 3]> = uses
                .iter()
                .map(|&(edge, forward)| {
                    let (start, end) = file.edge_ends(edge);
                    if forward { start } else { end }
                })
                .collect();
            let turn = dot(newell(&ring), normal);
            assert_eq!(turn > 0.0, outer, "face {} bound", file.name(face));
        }
    }
    // A wall of the outline faces away from the board, one of a cut-out
    // into the cut-out: the side at x = 30 faces +x.
    let east = file
        .of_kind("ADVANCED_FACE")
        .into_iter()
        .filter_map(|f| file.plane_normal(f))
        .filter(|n| n[0] > 0.5)
        .count();
    assert!(east >= 2, "the x = 30 and x = 12 sides and cut-out walls");
}

#[test]
fn a_drilled_board_reads_back_with_the_same_box() {
    let board = drilled_board();
    let bytes = write_step(&model(
        StepSchema::Ap214,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        Vec::new(),
    ))
    .expect("STEP");
    let ((lo, hi), _) = read_back(&bytes);
    assert_close(lo, [0.0, 0.0, 0.0], 1e-4);
    assert_close(hi, [40.0, 20.0, THICKNESS], 1e-4);
}

#[test]
fn a_flat_board_reads_back_with_the_same_area() {
    let (board, want) = flat_board();
    let bytes = write_step(&model(
        StepSchema::Ap242,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        Vec::new(),
    ))
    .expect("STEP");
    let ((lo, hi), area) = read_back(&bytes);
    assert_close(lo, [0.0, 0.0, 0.0], 1e-4);
    assert_close(hi, [30.0, 25.0, THICKNESS], 1e-4);
    assert!(
        (area - want).abs() <= 1e-3 * want,
        "read back {area} mm², want {want} mm²"
    );
}

/// A unit cube as twelve outward triangles.
fn cube(name: &str, at: [f64; 3]) -> StepMesh {
    let positions = (0..8)
        .map(|i| {
            [
                at[0] + f64::from(i & 1),
                at[1] + f64::from((i >> 1) & 1),
                at[2] + f64::from((i >> 2) & 1),
            ]
        })
        .collect();
    let triangles = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    StepMesh {
        name: name.to_owned(),
        positions,
        triangles,
    }
}

#[test]
fn mesh_bodies_read_back_with_their_triangles() {
    let board = drilled_board();
    let meshes = vec![
        cube("U1", [5.0, 5.0, THICKNESS]),
        cube("R1", [30.0, 15.0, -1.0]),
    ];
    let bytes = write_step(&model(
        StepSchema::Ap214,
        vec![StepSolid {
            name: "Board",
            solid: &board,
        }],
        meshes,
    ))
    .expect("STEP");
    let file = StepFile::parse(&bytes);
    let bodies: Vec<String> = file
        .of_kind("SHELL_BASED_SURFACE_MODEL")
        .into_iter()
        .map(|b| file.name(b))
        .collect();
    assert_eq!(bodies, ["U1", "R1"]);
    assert_eq!(file.count("MANIFOLD_SURFACE_SHAPE_REPRESENTATION"), 1);
    assert_eq!(file.count("ADVANCED_BREP_SHAPE_REPRESENTATION"), 1);
    // A closed cube shares every edge between two of its triangles.
    assert_eq!(
        file.count("EDGE_CURVE"),
        board.edge_count() + 2 * 18,
        "12 cube edges and 6 face diagonals each"
    );

    let ((lo, hi), _) = read_back(&bytes);
    assert_close(lo, [0.0, 0.0, -1.0], 1e-4);
    assert_close(hi, [40.0, 20.0, THICKNESS + 1.0], 1e-4);

    let alone = write_step(&model(
        StepSchema::Ap214,
        Vec::new(),
        vec![cube("U1", [0.0; 3])],
    ))
    .expect("mesh only");
    let (_, area) = read_back(&alone);
    assert!((area - 6.0).abs() < 1e-6, "cube area {area}");
}

#[test]
fn a_mesh_of_two_parts_is_one_body_of_two_shells() {
    let mut part = cube("U1", [0.0; 3]);
    let pin = cube("pin", [3.0, 0.0, 0.0]);
    let offset = u32::try_from(part.positions.len()).expect("few points");
    part.positions.extend(pin.positions);
    part.triangles
        .extend(pin.triangles.iter().map(|t| t.map(|i| i + offset)));
    let bytes = write_step(&model(StepSchema::Ap214, Vec::new(), vec![part])).expect("STEP");
    let file = StepFile::parse(&bytes);
    assert_eq!(file.count("SHELL_BASED_SURFACE_MODEL"), 1);
    assert_eq!(file.count("OPEN_SHELL"), 2, "one shell per connected part");
    assert_eq!(file.count("EDGE_CURVE"), 2 * 18);
}

#[test]
fn a_mesh_with_a_point_that_is_not_a_number_is_refused() {
    let mut bad = cube("U1", [0.0; 3]);
    bad.positions[3][1] = f64::NAN;
    let refused = write_step(&model(StepSchema::Ap214, Vec::new(), vec![bad]));
    assert!(matches!(
        refused,
        Err(signex_mcad::McadError::StepExport { .. })
    ));
}

#[test]
fn names_with_quotes_and_wide_characters_are_escaped() {
    let bytes = write_step(&model(
        StepSchema::Ap214,
        Vec::new(),
        vec![cube("C'1 Ω", [0.0; 3])],
    ))
    .expect("STEP");
    let text = std::str::from_utf8(&bytes).expect("ASCII only");
    assert!(text.is_ascii());
    assert!(text.contains("SHELL_BASED_SURFACE_MODEL('C''1 \\X2\\03A9\\X0\\'"));
}
