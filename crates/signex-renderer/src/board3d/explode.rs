//! The board taken apart along z for the 3D view's explode: the core
//! stays, each face's mask and copper move away from it on their side, and
//! the parts lift further. Only vertex heights change, so the triangles,
//! the board range and the parts' ranges still pick and highlight.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs.

use std::ops::Range;

use signex_gfx::scene3d::{MeshVertex, Scene3d};

use super::{BoardScene3d, Face};

/// One explode step as a share of the board's larger side, so the gaps
/// read at the zoom that fits the board.
const STEP_PER_BOARD_SIZE: f32 = 0.06;

/// How far apart the 3D view draws the board's layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Explode {
    /// 0 is assembled, 1 is layers apart; clamped to that, and not a
    /// number counts as 0.
    pub amount: f32,
    /// Move each face's solder mask one step and its copper two steps away
    /// from the core. Off, both stay on the core.
    pub separate_copper_and_mask: bool,
    /// Lift the parts one step further than the copper they sit on. Off,
    /// they stay on their copper.
    pub lift_components: bool,
}

impl Explode {
    /// The board as built: nothing moved.
    pub const ASSEMBLED: Self = Self {
        amount: 0.0,
        separate_copper_and_mask: true,
        lift_components: true,
    };

    /// Whether nothing moves: the amount is 0 or less (or not a number).
    pub fn is_assembled(&self) -> bool {
        self.amount.is_nan() || self.amount <= 0.0
    }

    /// The amount within 0..=1.
    fn clamped_amount(&self) -> f32 {
        if self.is_assembled() {
            0.0
        } else {
            self.amount.min(1.0)
        }
    }

    /// How many steps a vertex of `layer` moves, signed: up on the top
    /// side, down on the bottom.
    fn steps(&self, layer: Layer3d) -> f32 {
        let separate = if self.separate_copper_and_mask {
            1.0
        } else {
            0.0
        };
        let (face, steps) = match layer {
            Layer3d::Core => return 0.0,
            Layer3d::Mask(face) => (face, separate),
            Layer3d::Copper(face) => (face, 2.0 * separate),
            Layer3d::Part(face) => {
                let lift = if self.lift_components { 1.0 } else { 0.0 };
                (face, 2.0 * separate + lift)
            }
        };
        match face {
            Face::Top => steps,
            Face::Bottom => -steps,
        }
    }
}

impl Default for Explode {
    fn default() -> Self {
        Self::ASSEMBLED
    }
}

/// Which slice of the board a scene vertex belongs to, for the explode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer3d {
    /// The board edge and the hole walls; never moves.
    #[default]
    Core,
    /// A face of the slab, in the solder mask colour.
    Mask(Face),
    /// Copper on a face.
    Copper(Face),
    /// A component on a face.
    Part(Face),
}

impl BoardScene3d {
    /// This scene with its layers `explode` apart. Only vertex heights
    /// change: the indices, `board`, `parts` and `thickness_mm` stay, so
    /// [`Self::pick`] and [`Self::highlight_ranges`] work on the result.
    /// One step is 6% of the board's larger side (its body, else the whole
    /// scene); at amount 1 the mask is one step off the core, the copper
    /// two, and a lifted part three (one with the copper and mask kept on
    /// the core). Assembled, it is an identical clone.
    pub fn exploded(&self, explode: Explode) -> BoardScene3d {
        let mut out = self.clone();
        let amount = explode.clamped_amount();
        if amount <= 0.0 {
            return out;
        }
        let step = amount * self.explode_step_mm();
        for (vertex, layer) in out.scene.vertices.iter_mut().zip(&self.vertex_layers) {
            vertex.position[2] += explode.steps(*layer) * step;
        }
        out
    }

    /// One explode step at amount 1, mm.
    fn explode_step_mm(&self) -> f32 {
        let side = match self.board_size_mm() {
            Some([x, y, _]) => x.max(y),
            None => self
                .scene
                .bounds()
                .map_or(0.0, |(lo, hi)| (hi[0] - lo[0]).max(hi[1] - lo[1])),
        };
        if side.is_finite() {
            side * STEP_PER_BOARD_SIZE
        } else {
            0.0
        }
    }
}

/// Tag the slab's vertices appended since `layers` was last filled: those
/// of a level triangle in `slab` are the mask of the face they lie on
/// (above half of `thickness` is the top), the rest (edge and hole walls)
/// are the core.
pub(super) fn tag_slab(
    layers: &mut Vec<Layer3d>,
    scene: &Scene3d,
    slab: Range<u32>,
    thickness: f32,
) {
    let start = layers.len();
    layers.resize(scene.vertices.len(), Layer3d::Core);
    let indices = scene
        .indices
        .get(slab.start as usize..slab.end as usize)
        .unwrap_or_default();
    for tri in indices.chunks_exact(3) {
        let z = |k: usize| scene.vertices.get(tri[k] as usize).map(|v| v.position[2]);
        let (Some(a), Some(b), Some(c)) = (z(0), z(1), z(2)) else {
            continue;
        };
        if a != b || b != c {
            continue;
        }
        let layer = Layer3d::Mask(face_of(a, thickness));
        for &i in tri {
            let i = i as usize;
            if i >= start
                && let Some(slot) = layers.get_mut(i)
            {
                *slot = layer;
            }
        }
    }
}

/// Tag the vertices appended since `layers` was last filled with
/// `layer_of` each.
pub(super) fn tag_new(
    layers: &mut Vec<Layer3d>,
    scene: &Scene3d,
    layer_of: impl Fn(&MeshVertex) -> Layer3d,
) {
    let start = layers.len().min(scene.vertices.len());
    layers.extend(scene.vertices[start..].iter().map(layer_of));
}

/// The face a point at height `z` lies on, for a board `thickness` thick.
pub(super) fn face_of(z: f32, thickness: f32) -> Face {
    if z > thickness * 0.5 {
        Face::Top
    } else {
        Face::Bottom
    }
}
