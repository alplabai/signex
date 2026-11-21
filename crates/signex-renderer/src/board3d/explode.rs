//! The board taken apart along z for the 3D view's explode, in stack order:
//! the core stays; on each side the inner copper, the outer copper, the
//! mask and the silkscreen move away from it in that order, and the parts
//! lift further. Only vertex heights change, so the triangles, the board
//! range and the parts' ranges still pick and highlight.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs.

use signex_gfx::scene3d::Scene3d;

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
    /// Move each real layer its own step away from the core, in stack
    /// order. Off, they all stay on the core.
    pub separate_copper_and_mask: bool,
    /// Lift the parts one step further than the silkscreen they sit on.
    /// Off, they stay on it.
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
    /// side, down on the bottom. `inner` is how many inner copper layers
    /// move out on the top and on the bottom side.
    fn steps(&self, layer: Layer3d, inner: [u8; 2]) -> f32 {
        let separate = if self.separate_copper_and_mask {
            1.0
        } else {
            0.0
        };
        let copper_step = |face: Face| {
            let count = match face {
                Face::Top => inner[0],
                Face::Bottom => inner[1],
            };
            f32::from(count) + 1.0
        };
        let (face, steps) = match layer {
            Layer3d::Core => return 0.0,
            Layer3d::InnerCopper { face, rank } => (face, separate * f32::from(rank)),
            Layer3d::Copper(face) => (face, separate * copper_step(face)),
            Layer3d::Mask(face) => (face, separate * (copper_step(face) + 1.0)),
            Layer3d::Silk(face) => (face, separate * (copper_step(face) + 2.0)),
            Layer3d::Part(face) => {
                let lift = if self.lift_components { 1.0 } else { 0.0 };
                (face, separate * (copper_step(face) + 2.0) + lift)
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
    /// The laminate, the board edge and the hole walls; never moves.
    #[default]
    Core,
    /// An inner copper layer, moving out on `face`'s side, `rank` steps
    /// from the core.
    InnerCopper { face: Face, rank: u8 },
    /// A face's outer copper and its finish.
    Copper(Face),
    /// A face's solder mask.
    Mask(Face),
    /// A face's silkscreen.
    Silk(Face),
    /// A component on a face.
    Part(Face),
}

impl BoardScene3d {
    /// This scene with its layers `explode` apart. Only vertex heights
    /// change: the indices, `board`, `parts` and `thickness_mm` stay, so
    /// [`Self::pick`] and [`Self::highlight_ranges`] work on the result.
    /// One step is 6% of the board's larger side (its body, else the whole
    /// scene). At amount 1 on a two-layer board the copper is one step off
    /// the core, the mask two, the silkscreen three and a lifted part four
    /// (one with the layers kept on the core); each inner copper layer on a
    /// side adds a step. Assembled, it is an identical clone.
    pub fn exploded(&self, explode: Explode) -> BoardScene3d {
        let mut out = self.clone();
        let amount = explode.clamped_amount();
        if amount <= 0.0 {
            return out;
        }
        let step = amount * self.explode_step_mm();
        for (vertex, layer) in out.scene.vertices.iter_mut().zip(&self.vertex_layers) {
            vertex.position[2] += explode.steps(*layer, self.inner_copper) * step;
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

/// Tag the vertices appended since `layers` was last filled as `layer`.
pub(super) fn tag_new(layers: &mut Vec<Layer3d>, scene: &Scene3d, layer: Layer3d) {
    layers.resize(scene.vertices.len(), layer);
}
