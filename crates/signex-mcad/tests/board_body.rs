//! The board body (#2487): a prism with holes, its names, its tessellation,
//! and refused input.

use std::f64::consts::PI;

use signex_mcad::{
    EdgeName, FaceName, Hole, McadError, Solid, TessellationTolerance, board_body, tessellate,
};

const THICKNESS: f64 = 1.6;
const HOLE_DIAMETER: f64 = 3.2;

/// A 40 x 20 board with two round holes and a 4 x 4 cut-out.
fn sample_board() -> Solid {
    let outline = [[0.0, 0.0], [40.0, 0.0], [40.0, 20.0], [0.0, 20.0]];
    let holes = [
        Hole::Round {
            center: [10.0, 10.0],
            diameter: HOLE_DIAMETER,
        },
        Hole::Round {
            center: [30.0, 10.0],
            diameter: HOLE_DIAMETER,
        },
        Hole::Polygon(vec![[18.0, 8.0], [22.0, 8.0], [22.0, 12.0], [18.0, 12.0]]),
    ];
    board_body(&outline, &holes, THICKNESS).expect("board body")
}

fn expected_volume() -> f64 {
    let r = HOLE_DIAMETER / 2.0;
    (40.0 * 20.0 - 2.0 * PI * r * r - 16.0) * THICKNESS
}

fn count_named(solid: &Solid, name: FaceName) -> usize {
    solid
        .faces()
        .iter()
        .filter(|f| f.names.contains(&name))
        .count()
}

#[test]
fn prism_with_holes_has_the_exact_volume() {
    let mut board = sample_board();
    let volume = board.volume().expect("volume");
    let want = expected_volume();
    assert!(
        (volume - want).abs() <= 1e-5 * want,
        "volume {volume}, want {want}"
    );
}

#[test]
fn faces_keep_their_names_through_the_cuts() {
    let board = sample_board();
    assert_eq!(count_named(&board, FaceName::Top), 1, "{:?}", board.faces());
    assert_eq!(count_named(&board, FaceName::Bottom), 1);
    for side in 0..4 {
        assert_eq!(count_named(&board, FaceName::OutlineSide(side)), 1);
    }
    assert!(count_named(&board, FaceName::HoleWall(0)) >= 1);
    assert!(count_named(&board, FaceName::HoleWall(1)) >= 1);
    assert_eq!(count_named(&board, FaceName::CutoutWall(2)), 4);
    assert!(board.faces().iter().all(|f| !f.names.is_empty()));
}

#[test]
fn edges_are_named_by_the_faces_they_join() {
    let board = sample_board();
    let edges = board.edges().expect("edges");
    assert_eq!(edges.len(), board.edge_count());
    let rim = EdgeName {
        faces: vec![FaceName::Top, FaceName::HoleWall(0)],
    };
    assert!(edges.contains(&rim), "{edges:?}");
    let corner = EdgeName {
        faces: vec![FaceName::OutlineSide(0), FaceName::OutlineSide(1)],
    };
    assert!(edges.contains(&corner), "{edges:?}");
}

#[test]
fn tessellation_is_non_empty_closed_and_outward() {
    let board = sample_board();
    let mesh = tessellate(&board, TessellationTolerance::default()).expect("mesh");
    assert!(!mesh.is_empty());
    assert_eq!(mesh.faces.len(), board.face_count());
    assert!(
        mesh.faces.iter().all(|f| f.triangle_count > 0),
        "{:?}",
        mesh.faces
    );
    assert_eq!(mesh.normals.len(), mesh.positions.len());
    // Polygonal holes are a little smaller than round ones, so allow 1 %.
    let volume = mesh.signed_volume();
    let want = expected_volume();
    assert!(
        (volume - want).abs() <= 0.01 * want,
        "mesh volume {volume}, want {want}"
    );
}

#[test]
fn overlapping_holes_cut() {
    let outline = [[0.0, 0.0], [20.0, 0.0], [20.0, 20.0], [0.0, 20.0]];
    let holes = [
        Hole::Round {
            center: [8.0, 10.0],
            diameter: 4.0,
        },
        Hole::Round {
            center: [10.0, 10.0],
            diameter: 4.0,
        },
    ];
    let mut board = board_body(&outline, &holes, THICKNESS).expect("board body");
    // Two discs of radius 2 whose centres are 2 apart.
    let (r, d) = (2.0_f64, 2.0_f64);
    let lens = 2.0 * r * r * (d / (2.0 * r)).acos() - 0.5 * d * (4.0 * r * r - d * d).sqrt();
    let want = (400.0 - (2.0 * PI * r * r - lens)) * THICKNESS;
    let volume = board.volume().expect("volume");
    assert!(
        (volume - want).abs() <= 1e-5 * want,
        "volume {volume}, want {want}"
    );
}

#[test]
fn refused_input_is_an_error_value() {
    let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let invalid = |r: Result<Solid, McadError>| matches!(r, Err(McadError::InvalidInput { .. }));
    assert!(invalid(board_body(&square, &[], 0.0)));
    assert!(invalid(board_body(&square, &[], f64::NAN)));
    assert!(invalid(board_body(&square[..2], &[], 1.0)));
    let bad_hole = Hole::Round {
        center: [5.0, 5.0],
        diameter: -1.0,
    };
    assert!(invalid(board_body(&square, &[bad_hole], 1.0)));

    let board = board_body(&square, &[], 1.0).expect("board");
    let bad = TessellationTolerance {
        angle_deg: 0.0,
        chord_mm: 0.01,
    };
    assert!(matches!(
        tessellate(&board, bad),
        Err(McadError::InvalidInput { .. })
    ));
}
