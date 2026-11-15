//! The board in 3D (#2390): the board lowered to one triangle mesh for the
//! 3D view. The board outline is extruded to its thickness with the drill
//! holes, cutouts and mounting holes through it; the copper the flat view
//! shows lies on the two faces; each component is its attached model, or a
//! plain body extruded from its 3D Body outline. Rules and frames:
//! `docs/RENDERING_RULES.md`, "Board 3D view".
//!
//! The copper and the board's own geometry come from the same
//! [`PcbSnapshot`] the flat view draws, so the two views cannot disagree
//! about what is on the board or which layers are hidden.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs, IPC-2221 board stack conventions.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use signex_gfx::scene3d::Scene3d;
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{PCB_DEFAULT_THICKNESS_MM, PcbBoard};
use uuid::Uuid;

use crate::pcb::{PcbLayerView, PcbSnapshot};

mod copper;
mod model_file;
mod parts;

pub use model_file::read_model_file;

/// One mesh of a component model: positions in the model's own frame
/// (mm, z up), triangles into them, one colour.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub color: [f32; 4],
}

/// A model file attached to a library footprint, read, with where it sits
/// on the footprint.
#[derive(Clone, Debug, PartialEq)]
pub struct AttachedModel {
    pub meshes: Vec<ModelMesh>,
    /// Offset from the footprint origin, mm, in the model's frame.
    pub offset_mm: [f64; 3],
    /// Turns about the model's x, then y, then z axis, degrees.
    pub rotation_deg: [f64; 3],
}

/// A plain body: an outline in the footprint's frame extruded upward from
/// the board face.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyExtrusion {
    /// The 3D Body outline; `None` takes the footprint's body box (its
    /// courtyard, else its pads), the one a click selects it by.
    pub outline: Option<Vec<[f64; 2]>>,
    pub height_mm: f64,
    /// How far above the face the body starts.
    pub offset_z_mm: f64,
    /// The top face's colour and the walls' colour; `None` takes the 3D
    /// Body layer colour.
    pub colors: Option<([f32; 4], [f32; 4])>,
}

/// What a component looks like in 3D.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentModel {
    Model(AttachedModel),
    Body(BodyExtrusion),
}

/// The colours of the board in 3D, from the board layer palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Board3dColors {
    /// The solder mask over the two faces.
    pub mask: [f32; 4],
    /// The board edge and the hole walls.
    pub edge: [f32; 4],
    pub top_copper: [f32; 4],
    pub bottom_copper: [f32; 4],
    /// Vias and through-hole pads, on both faces.
    pub multi_layer: [f32; 4],
    /// A body with no colour of its own.
    pub body: [f32; 4],
}

/// What the scene is built from.
pub struct Board3dInput<'a> {
    pub board: &'a PcbBoard,
    pub view: &'a PcbLayerView,
    /// The models read for the library footprints the board's footprints
    /// came from. A footprint whose source has none is a plain body.
    pub models: &'a HashMap<FootprintSourceRef, ComponentModel>,
    pub colors: Board3dColors,
}

/// Which face a component sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Top,
    Bottom,
}

/// One component in the scene: its footprint, its triangles and its face.
#[derive(Clone, Debug, PartialEq)]
pub struct Part3d {
    pub footprint: Uuid,
    pub indices: Range<u32>,
    pub face: Face,
}

/// The board as one mesh, with the triangles of the board itself and of
/// each component.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardScene3d {
    pub scene: Scene3d,
    /// The board slab: faces, edge and hole walls.
    pub board: Range<u32>,
    pub parts: Vec<Part3d>,
    /// The board's thickness, mm: the top face is at this height, the
    /// bottom face at 0.
    pub thickness_mm: f32,
}

/// Height of the copper above its face, mm (1 oz copper). Keeps the copper
/// off the mask in the depth buffer.
pub(crate) const COPPER_LIFT_MM: f32 = 0.035;

/// A board point (mm, y down as the flat view draws it) in the 3D frame
/// (x right, y up, z up out of the top face).
pub(crate) fn board_xy(p: [f32; 2]) -> [f32; 2] {
    [p[0], -p[1]]
}

impl BoardScene3d {
    /// The footprint whose part a ray from `origin` along `dir` meets
    /// first; `None` when it meets the board first or nothing.
    pub fn pick(&self, origin: [f32; 3], dir: [f32; 3]) -> Option<Uuid> {
        let board = self.scene.ray_hit(self.board.clone(), origin, dir);
        let (footprint, t) = self
            .parts
            .iter()
            .filter_map(|part| {
                let t = self.scene.ray_hit(part.indices.clone(), origin, dir)?;
                Some((part.footprint, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        board.is_none_or(|b| t <= b).then_some(footprint)
    }

    /// The triangles of the parts of `footprints`, for the selection wash.
    pub fn highlight_ranges(&self, footprints: &BTreeSet<Uuid>) -> Vec<Range<u32>> {
        self.parts
            .iter()
            .filter(|part| footprints.contains(&part.footprint))
            .map(|part| part.indices.clone())
            .collect()
    }

    /// The part of `footprint`, if it has one.
    pub fn part(&self, footprint: Uuid) -> Option<&Part3d> {
        self.parts.iter().find(|part| part.footprint == footprint)
    }

    /// The box around everything; the board's box when it is empty.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.scene.bounds()
    }
}

/// Build the 3D scene of `input.board` as `input.view` shows it.
pub fn build_board_scene(input: &Board3dInput<'_>) -> BoardScene3d {
    let board = input.board;
    let thickness = if board.thickness.is_finite() && board.thickness > 0.0 {
        board.thickness as f32
    } else {
        PCB_DEFAULT_THICKNESS_MM as f32
    };
    // Holes go through whatever layers are hidden; copper shows as the view
    // shows it.
    let every = PcbSnapshot::from_board(board);
    let shown = PcbSnapshot::from_board_with_view(board, input.view);
    let mut out = BoardScene3d {
        thickness_mm: thickness,
        ..BoardScene3d::default()
    };
    out.board = copper::append_board_slab(&mut out.scene, &every, thickness, &input.colors);
    copper::append_copper(&mut out.scene, &shown, thickness, &input.colors);
    for footprint in &board.footprints {
        if let Some(part) = parts::append_part(&mut out.scene, footprint, input, thickness) {
            out.parts.push(part);
        }
    }
    out
}

#[cfg(test)]
mod tests;
