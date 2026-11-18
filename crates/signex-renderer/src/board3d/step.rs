//! The board as a STEP file (#2488): the board body as an exact solid on
//! the MCAD kernel, with each component's body placed as the 3D view
//! places it. The slab's outline and openings and the parts come from the
//! same code as the 3D view, so the file and the view agree. Rules:
//! `docs/BOARD_OUTPUT.md`, "STEP export".

use std::collections::HashMap;

use signex_gfx::scene3d::Scene3d;
use signex_mcad::{
    Hole, McadError, StepHeader, StepMesh, StepModel, StepSchema, StepSolid, board_body,
    write_step,
};
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;

use super::copper::{Opening, slab_openings, slab_outline};
use super::{Board3dColors, Board3dInput, ComponentModel, board_thickness_mm, parts};
use crate::pcb::{PcbLayerView, PcbSnapshot};

/// What a board STEP file is made from.
pub struct BoardStepInput<'a> {
    pub board: &'a PcbBoard,
    /// The models read for the board's library footprints; a footprint
    /// whose source has none is a plain body, as in the 3D view.
    pub models: &'a HashMap<FootprintSourceRef, ComponentModel>,
    pub schema: StepSchema,
    pub header: StepHeader,
    /// The product's name, e.g. the board's file stem.
    pub product: String,
}

/// A board STEP file and what went into it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardStep {
    pub bytes: Vec<u8>,
    /// Component bodies written.
    pub parts: usize,
    /// Holes left out of the board body because each meets a hole cut
    /// before it, as in the 3D view.
    pub dropped_holes: usize,
}

/// The STEP file of `input.board`.
///
/// # Errors
///
/// [`McadError::InvalidInput`] when the board has no outline and no copper
/// to frame; any [`McadError`] the board body or the writer gives.
pub fn board_step(input: &BoardStepInput<'_>) -> Result<BoardStep, McadError> {
    let board = input.board;
    let thickness = board_thickness_mm(board);
    let every = PcbSnapshot::from_board(board);
    let outline = slab_outline(&every).ok_or_else(|| McadError::InvalidInput {
        operation: "board STEP",
        reason: "the board has no outline and no copper".to_owned(),
    })?;
    let (openings, dropped_holes) = slab_openings(&every, &outline);
    let holes: Vec<Hole> = openings
        .iter()
        .map(|opening| match opening {
            Opening::Cutout(ring) => Hole::Polygon(ring.iter().map(|&p| xy(p)).collect()),
            Opening::Drill { center, diameter } => Hole::Round {
                center: xy(*center),
                diameter: mm(*diameter),
            },
        })
        .collect();
    let outline: Vec<[f64; 2]> = outline.iter().map(|&p| xy(p)).collect();
    let body = board_body(&outline, &holes, thickness)?;
    let meshes = part_meshes(board, input.models, thickness as f32);
    let parts = meshes.len();
    let bytes = write_step(&StepModel {
        schema: input.schema,
        header: input.header.clone(),
        product: input.product.clone(),
        solids: vec![StepSolid {
            name: "Board",
            solid: &body,
        }],
        meshes,
    })?;
    Ok(BoardStep {
        bytes,
        parts,
        dropped_holes,
    })
}

/// Each footprint's part as the 3D view builds it, with every layer shown,
/// named by its designator.
fn part_meshes(
    board: &PcbBoard,
    models: &HashMap<FootprintSourceRef, ComponentModel>,
    thickness: f32,
) -> Vec<StepMesh> {
    let view = PcbLayerView::default();
    let input = Board3dInput {
        board,
        view: &view,
        models,
        colors: UNUSED_COLORS,
    };
    board
        .footprints
        .iter()
        .enumerate()
        .filter_map(|(n, footprint)| {
            let mut scene = Scene3d::default();
            parts::append_part(&mut scene, footprint, &input, thickness)?;
            let name = if footprint.reference.trim().is_empty() {
                format!("Part {}", n + 1)
            } else {
                footprint.reference.clone()
            };
            Some(StepMesh {
                name,
                positions: scene
                    .vertices
                    .iter()
                    .map(|v| [mm(v.position[0]), mm(v.position[1]), mm(v.position[2])])
                    .collect(),
                triangles: scene
                    .indices
                    .chunks_exact(3)
                    .map(|t| [t[0], t[1], t[2]])
                    .collect(),
            })
        })
        .collect()
}

/// The file carries no colours.
const UNUSED_COLORS: Board3dColors = Board3dColors {
    mask: [0.0; 4],
    edge: [0.0; 4],
    top_copper: [0.0; 4],
    bottom_copper: [0.0; 4],
    multi_layer: [0.0; 4],
    body: [0.0; 4],
};

/// A 3D-frame length, mm, as f64 rounded to the nanometre, so the f32 of
/// the 3D view's `12.7` is written `12.7`, not `12.699999809265137`.
fn mm(x: f32) -> f64 {
    (f64::from(x) * 1e6).round() / 1e6
}

fn xy(p: [f32; 2]) -> [f64; 2] {
    [mm(p[0]), mm(p[1])]
}
