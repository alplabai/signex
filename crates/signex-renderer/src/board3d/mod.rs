//! The board in 3D (#2390): the board lowered to one triangle mesh for the
//! 3D view, as its physical stack. On each side, from the laminate out:
//! copper, solder mask with its real openings, silkscreen. Each component
//! is its attached model, or a plain body extruded from its 3D Body
//! outline. Every vertex knows its slice of the board, so the view can draw
//! the layers apart ([`BoardScene3d::exploded`]). Rules and frames:
//! `docs/RENDERING_RULES.md`, "board-3d".
//!
//! The geometry comes from the board and the same [`PcbSnapshot`] the flat
//! view draws, so the two views agree on what is on the board and which
//! layers are hidden. The colours are physical materials, never the flat
//! view's layer colours.
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

use crate::pcb::{BOARD_OUTLINE, PcbLayerView, PcbSnapshot};
use layers::{ArtInput, BoardArt};
use stack::StackHeights;

mod copper;
mod drawings;
mod explode;
mod layers;
mod model_file;
mod parts;
mod region;
mod stack;
mod step;

pub use explode::{Explode, Layer3d};
pub use layers::mask_over_copper;
pub use model_file::{read_model_file, read_step_bytes};
pub use parts::model_point_on_footprint;
pub use signex_mcad::{McadError, StepHeader, StepSchema};
pub use step::{BoardStep, BoardStepInput, board_step};

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
    /// The top face's colour and the walls' colour; `None` takes
    /// [`Board3dColors::body`].
    pub colors: Option<([f32; 4], [f32; 4])>,
}

/// What a component looks like in 3D.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentModel {
    Model(AttachedModel),
    Body(BodyExtrusion),
}

/// The materials of the board in 3D. None is a flat-view layer colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Board3dColors {
    /// The solder mask coat; its alpha lets the copper under it show.
    pub mask: [f32; 4],
    /// The FR-4 laminate, its edge and unplated hole walls.
    pub substrate: [f32; 4],
    /// Bare copper: under the mask and on inner layers.
    pub copper: [f32; 4],
    /// The surface finish on exposed copper and plated drill walls.
    pub finish: [f32; 4],
    /// Silkscreen ink.
    pub silk: [f32; 4],
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
    /// The board from mask to mask: laminate, walls, copper and mask.
    pub board: Range<u32>,
    pub parts: Vec<Part3d>,
    /// The board's thickness, mm: the top face is at this height, the
    /// bottom face at 0.
    pub thickness_mm: f32,
    /// Holes left out of the slab because they meet a hole cut before
    /// them (`signex_gfx::ring_fill::disjoint_holes`).
    pub dropped_holes: usize,
    /// The slice of the board each of `scene.vertices` belongs to, in the
    /// same order, for [`Self::exploded`]. A vertex with no entry is core
    /// and never moves.
    pub vertex_layers: Vec<Layer3d>,
    /// How many inner copper layers move out on the top and on the bottom
    /// side in [`Self::exploded`].
    pub inner_copper: [u8; 2],
}

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

    /// The board body's x and y extent and its thickness, mm; `None` when
    /// the scene has no board body.
    pub fn board_size_mm(&self) -> Option<[f32; 3]> {
        let (lo, hi) = self.scene.bounds_of(self.board.clone())?;
        Some([hi[0] - lo[0], hi[1] - lo[1], self.thickness_mm])
    }
}

/// Build the 3D scene of `input.board` as `input.view` shows it.
pub fn build_board_scene(input: &Board3dInput<'_>) -> BoardScene3d {
    let board = input.board;
    let thickness = board_thickness_mm(board) as f32;
    // Holes go through whatever layers are hidden; copper shows as the
    // layers' eyes show it. Single-layer mode is a flat-view editing aid and
    // does not reach 3D: see `docs/RENDERING_RULES.md`, "board-3d".
    let every = PcbSnapshot::from_board(board);
    let view = all_layers_of(input.view);
    let shown = PcbSnapshot::from_board_with_view(board, &view)
        .with_zone_fills(&pcb_geom::zone_fills(board));
    let heights = StackHeights::of(board, thickness);
    let mut out = BoardScene3d {
        thickness_mm: thickness,
        inner_copper: heights.inner_per_side(),
        ..BoardScene3d::default()
    };
    let (slab, dropped) =
        copper::append_board_slab(&mut out.scene, &every, &heights, &input.colors);
    out.dropped_holes = dropped;
    explode::tag_new(&mut out.vertex_layers, &out.scene, Layer3d::Core);
    let outline = copper::slab_outline(&every).unwrap_or_default();
    let art = layers::board_art(&ArtInput {
        board,
        every: &every,
        shown: &shown,
        view: &view,
        outline: &outline,
        heights: &heights,
    });
    append_layers(&mut out, &art, &heights, &input.colors);
    out.board = slab.start..board_end(&mut out, &art, &heights, &input.colors);
    for footprint in &board.footprints {
        let part = parts::append_part(&mut out.scene, footprint, input, thickness);
        let layer = part
            .as_ref()
            .map_or(Layer3d::Core, |part| Layer3d::Part(part.face));
        explode::tag_new(&mut out.vertex_layers, &out.scene, layer);
        out.parts.extend(part);
    }
    out
}

/// Append the copper, innermost first, so the translucent mask drawn after
/// it blends over it from either side.
fn append_layers(
    out: &mut BoardScene3d,
    art: &BoardArt,
    heights: &StackHeights,
    colors: &Board3dColors,
) {
    for (index, polygons) in art.inner.iter().enumerate() {
        let Some((_, z)) = heights.inner.get(index) else {
            continue;
        };
        region::append_polygons(&mut out.scene, polygons, *z, colors.copper);
        let (face, rank) = heights.inner_place(index);
        let layer = Layer3d::InnerCopper { face, rank };
        explode::tag_new(&mut out.vertex_layers, &out.scene, layer);
    }
    for face in [Face::Top, Face::Bottom] {
        let side = art.face(face);
        let z = heights.copper(face);
        region::append_polygons(&mut out.scene, &side.covered_copper, z, colors.copper);
        region::append_polygons(&mut out.scene, &side.finish, z, colors.finish);
        explode::tag_new(&mut out.vertex_layers, &out.scene, Layer3d::Copper(face));
    }
}

/// Append the two masks, close the board's index range there, then append
/// the silkscreen, which stands proud of the board's faces. Returns the
/// range's end.
fn board_end(
    out: &mut BoardScene3d,
    art: &BoardArt,
    heights: &StackHeights,
    colors: &Board3dColors,
) -> u32 {
    let raised = layers::mask_over_copper(colors.mask);
    for face in [Face::Top, Face::Bottom] {
        let side = art.face(face);
        let z = heights.mask(face);
        region::append_polygons(&mut out.scene, &side.mask, z, colors.mask);
        region::append_polygons(&mut out.scene, &side.mask_over_copper, z, raised);
        explode::tag_new(&mut out.vertex_layers, &out.scene, Layer3d::Mask(face));
    }
    let end = out.scene.indices.len() as u32;
    for face in [Face::Top, Face::Bottom] {
        let silk = &art.face(face).silk;
        region::append_polygons(&mut out.scene, silk, heights.silk(face), colors.silk);
        explode::tag_new(&mut out.vertex_layers, &out.scene, Layer3d::Silk(face));
    }
    end
}

/// The board's thickness, mm: its layer stack's total (the board engine
/// keeps `thickness` equal to it), else the default.
pub(crate) fn board_thickness_mm(board: &PcbBoard) -> f64 {
    if board.thickness.is_finite() && board.thickness > 0.0 {
        board.thickness
    } else {
        PCB_DEFAULT_THICKNESS_MM
    }
}

/// `view` with single-layer mode off and no active copper layer: the hidden
/// layers stay hidden, and a via or pad keeps its own size, not the size it
/// has on the layer being worked on.
fn all_layers_of(view: &PcbLayerView) -> PcbLayerView {
    let mut view = view.clone();
    if view.single_layer() {
        view.toggle_single_layer();
    }
    view.set_active(BOARD_OUTLINE);
    view
}

#[cfg(test)]
mod tests;
