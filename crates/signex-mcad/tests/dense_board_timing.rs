//! Timing of the board body and its STEP export on a dense board (#2488).
//! Ignored by default; run in release with
//! `cargo test --release -p signex-mcad --test dense_board_timing -- --ignored --nocapture`.

use std::time::Instant;

use signex_mcad::{
    Hole, StepHeader, StepMesh, StepModel, StepSchema, StepSolid, board_body, write_step,
};

/// A 100 x 80 board with `count` 0.3 mm vias on a 1.27 mm grid.
fn vias(count: usize) -> Vec<Hole> {
    let columns = 70;
    (0..count)
        .map(|i| Hole::Round {
            center: [
                5.0 + 1.27 * (i % columns) as f64,
                5.0 + 1.27 * (i / columns) as f64,
            ],
            diameter: 0.3,
        })
        .collect()
}

/// A flat grid of `side` x `side` quads as one mesh: 2 x side² triangles.
fn grid_mesh(side: u32) -> StepMesh {
    let n = side + 1;
    let positions = (0..n * n)
        .map(|i| [f64::from(i % n) * 0.01, f64::from(i / n) * 0.01, 1.6])
        .collect();
    let triangles = (0..side * side)
        .flat_map(|q| {
            let (x, y) = (q % side, q / side);
            let a = y * n + x;
            [[a, a + 1, a + n + 1], [a, a + n + 1, a + n]]
        })
        .collect();
    StepMesh {
        name: "U1".to_owned(),
        positions,
        triangles,
    }
}

#[test]
#[ignore = "timing run, not a check"]
fn dense_board_timing() {
    let outline = [[0.0, 0.0], [100.0, 0.0], [100.0, 80.0], [0.0, 80.0]];
    for count in [250, 500, 1000, 2000] {
        let holes = vias(count);
        let start = Instant::now();
        let board = board_body(&outline, &holes, 1.6).expect("board body");
        let body = start.elapsed();
        let start = Instant::now();
        let bytes = write_step(&StepModel {
            schema: StepSchema::Ap214,
            header: StepHeader {
                file_name: "board.step".to_owned(),
                timestamp: "2026-11-18T10:00:00".to_owned(),
                system: "Signex timing".to_owned(),
            },
            product: "board".to_owned(),
            solids: vec![StepSolid {
                name: "Board",
                solid: &board,
            }],
            meshes: Vec::new(),
        })
        .expect("STEP");
        println!(
            "vias {count}: board_body {body:?}, write_step {:?}, {} bytes",
            start.elapsed(),
            bytes.len()
        );
    }
    let mesh = grid_mesh(316);
    let triangles = mesh.triangles.len();
    let start = Instant::now();
    let bytes = write_step(&StepModel {
        schema: StepSchema::Ap214,
        header: StepHeader {
            file_name: "mesh.step".to_owned(),
            timestamp: "2026-11-18T10:00:00".to_owned(),
            system: "Signex timing".to_owned(),
        },
        product: "mesh".to_owned(),
        solids: Vec::new(),
        meshes: vec![mesh],
    })
    .expect("STEP");
    println!(
        "mesh {triangles} triangles: write_step {:?}, {} bytes",
        start.elapsed(),
        bytes.len()
    );
}
