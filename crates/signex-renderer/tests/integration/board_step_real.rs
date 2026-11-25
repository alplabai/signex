//! The board STEP export (#2488) on a real board
//! (`signex-types/tests/corpus/pcb/real/mixed-tht-smd.snxpcb`): what the
//! file holds is pinned in a golden summary, and the file reads back through
//! Signex's STEP importer with the board body and parts where the 3D view
//! puts them. Regenerate the golden on purpose only:
//! `UPDATE_GOLDEN=1 cargo test -p signex-renderer --test integration board_step`.

use std::collections::HashMap;
use std::path::Path;

use signex_3d_model_importer::step::p21::parse_to_meshes;
use signex_renderer::board3d::{
    Board3dColors, Board3dInput, BoardStepInput, ComponentModel, StepHeader, StepSchema,
    board_step, build_board_scene,
};
use signex_renderer::pcb::PcbLayerView;
use signex_types::format::SnxPcb;
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;

const BOARD: &str = "mixed-tht-smd";

fn load() -> PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real")
        .join(format!("{BOARD}.snxpcb"));
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("board parses").board
}

fn input(board: &PcbBoard, models: &HashMap<FootprintSourceRef, ComponentModel>) -> Vec<u8> {
    board_step(&BoardStepInput {
        board,
        models,
        schema: StepSchema::Ap214,
        header: StepHeader {
            file_name: format!("{BOARD}.step"),
            timestamp: "2026-11-18T10:00:00".to_owned(),
            system: "Signex test".to_owned(),
        },
        product: BOARD.to_owned(),
    })
    .expect("the board STEP is made")
    .bytes
}

type Bounds = ([f32; 3], [f32; 3]);

fn union(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
    match (a, b) {
        (Some((alo, ahi)), Some((blo, bhi))) => Some((
            [0, 1, 2].map(|k| alo[k].min(blo[k])),
            [0, 1, 2].map(|k| ahi[k].max(bhi[k])),
        )),
        (a, b) => a.or(b),
    }
}

/// The box of the board slab and the parts as the 3D view builds them
/// (the copper on the faces left out: the STEP file has none).
fn view_bounds(board: &PcbBoard) -> Bounds {
    let colors = Board3dColors {
        mask: [0.5; 4],
        substrate: [0.5; 4],
        copper: [0.5; 4],
        finish: [0.5; 4],
        silk: [0.5; 4],
        body: [0.5; 4],
    };
    let models = HashMap::new();
    let view = PcbLayerView::default();
    let scene = build_board_scene(&Board3dInput {
        board,
        view: &view,
        models: &models,
        colors,
    });
    let mut bounds = scene.scene.bounds_of(scene.board.clone());
    for part in &scene.parts {
        bounds = union(bounds, scene.scene.bounds_of(part.indices.clone()));
    }
    bounds.expect("the board has a slab")
}

fn read_back_bounds(bytes: &[u8]) -> Bounds {
    let text = std::str::from_utf8(bytes).expect("ASCII");
    let result = parse_to_meshes(text).expect("the importer reads it");
    let mut bounds = None;
    for mesh in &result.meshes {
        for p in mesh.positions.chunks_exact(3) {
            let p = [p[0], p[1], p[2]];
            bounds = union(bounds, Some((p, p)));
        }
    }
    bounds.expect("geometry read back")
}

fn count(text: &str, kind: &str) -> usize {
    text.matches(&format!("={kind}(")).count()
}

/// What the file holds, one fact per line.
fn summary(bytes: &[u8]) -> String {
    let text = std::str::from_utf8(bytes).expect("ASCII");
    let (lo, hi) = read_back_bounds(bytes);
    let mut out = String::new();
    for kind in [
        "MANIFOLD_SOLID_BREP",
        "ADVANCED_FACE",
        "PLANE",
        "CYLINDRICAL_SURFACE",
        "EDGE_CURVE",
        "LINE",
        "CIRCLE",
        "SHELL_BASED_SURFACE_MODEL",
    ] {
        out.push_str(&format!("{kind} {}\n", count(text, kind)));
    }
    out.push_str(&format!(
        "box {:.3} {:.3} {:.3} .. {:.3} {:.3} {:.3}\n",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    ));
    out
}

#[test]
fn board_step_of_a_real_board_matches_its_golden_summary() {
    let board = load();
    let bytes = input(&board, &HashMap::new());
    let got = summary(&bytes);
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("board_step_{}.txt", BOARD.replace('-', "_")));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&golden, &got).expect("golden written");
    }
    let want = std::fs::read_to_string(&golden).expect("golden summary");
    assert_eq!(got, want, "the board STEP changed: {}", golden.display());
}

#[test]
fn board_step_of_a_real_board_reads_back_where_the_3d_view_puts_it() {
    let board = load();
    let bytes = input(&board, &HashMap::new());
    let (lo, hi) = read_back_bounds(&bytes);
    let (want_lo, want_hi) = view_bounds(&board);
    for k in 0..3 {
        assert!(
            (lo[k] - want_lo[k]).abs() < 1e-3 && (hi[k] - want_hi[k]).abs() < 1e-3,
            "read back {lo:?}..{hi:?}, the 3D view has {want_lo:?}..{want_hi:?}"
        );
    }
    let text = std::str::from_utf8(&bytes).expect("ASCII");
    assert_eq!(
        count(text, "SHELL_BASED_SURFACE_MODEL"),
        board.footprints.len(),
        "a body per footprint"
    );
    assert_eq!(bytes, input(&board, &HashMap::new()), "deterministic");
}
