//! Where each layer of the board sits in z: from the stored layer stack, or
//! the stack derived from the board's copper list and thickness. The outer
//! mask surfaces are the board's two faces. Rules:
//! `docs/RENDERING_RULES.md`, "board-3d".

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{LayerStack, PcbBoard};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_stack::copper_at;

use super::Face;

/// The thinnest a mask or outer copper skin is drawn, mm, so a stack row of
/// no thickness still keeps its order in the depth buffer.
const MIN_SKIN_MM: f32 = 0.005;
/// The same floor as a share of the board, for a very thin board.
const MIN_SKIN_SHARE: f32 = 0.05;
/// The most of the board one skin may take.
const MAX_SKIN_SHARE: f32 = 0.2;
/// How far silkscreen ink stands off the mask, mm.
const SILK_MM: f32 = 0.015;

/// The heights of a board's layers, mm, z up from the bottom face.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct StackHeights {
    pub(super) thickness: f32,
    /// The outer surface of the top copper.
    pub(super) top_copper: f32,
    /// The top of the laminate.
    pub(super) top_core: f32,
    /// The bottom of the laminate.
    pub(super) bottom_core: f32,
    /// The outer surface of the bottom copper.
    pub(super) bottom_copper: f32,
    /// The inner copper layers, top to bottom, each at its row's middle.
    pub(super) inner: Vec<(CopperLayer, f32)>,
}

impl StackHeights {
    /// The heights of `board`, `thickness` mm thick.
    pub(super) fn of(board: &PcbBoard, thickness: f32) -> Self {
        let derived;
        let stack = match board.stackup.as_ref().filter(|s| usable(s)) {
            Some(stored) => stored,
            None => {
                derived = LayerStack::derived(&board.layers, f64::from(thickness));
                &derived
            }
        };
        let total = stack.total_thickness_mm() as f32;
        let scale = if total > 0.0 { thickness / total } else { 1.0 };
        // Each copper row's distance below the top face and its thickness.
        let mut rows = Vec::new();
        let mut depth = 0.0_f32;
        for row in &stack.layers {
            let row_mm = row.thickness_mm as f32 * scale;
            if row.kind.is_copper() {
                rows.push((row, depth, row_mm));
            }
            depth += row_mm;
        }
        let floor = MIN_SKIN_MM.min(thickness * MIN_SKIN_SHARE);
        let ceiling = (thickness * MAX_SKIN_SHARE).max(floor);
        let skin = |mm: f32| mm.clamp(floor, ceiling);
        let (top_mask, top_metal) = rows.first().map_or((0.0, 0.0), |r| (r.1, r.2));
        let (bottom_mask, bottom_metal) = rows
            .last()
            .map_or((0.0, 0.0), |r| (thickness - r.1 - r.2, r.2));
        let top_copper = thickness - skin(top_mask);
        let top_core = top_copper - skin(top_metal);
        let bottom_copper = skin(bottom_mask);
        let bottom_core = bottom_copper + skin(bottom_metal);
        let count = rows.len();
        let inner = rows
            .iter()
            .enumerate()
            .filter(|(index, _)| *index > 0 && *index + 1 < count)
            .filter_map(|(index, (row, row_depth, row_mm))| {
                let named = parse_pcb_layer(&row.layer).and_then(|layer| layer.copper());
                let layer = named.or_else(|| copper_at(index, count))?;
                let z = thickness - *row_depth - *row_mm * 0.5;
                Some((layer, z.clamp(bottom_core, top_core)))
            })
            .collect();
        Self {
            thickness,
            top_copper,
            top_core,
            bottom_core,
            bottom_copper,
            inner,
        }
    }

    /// The outer surface of `face`'s copper.
    pub(super) fn copper(&self, face: Face) -> f32 {
        match face {
            Face::Top => self.top_copper,
            Face::Bottom => self.bottom_copper,
        }
    }

    /// The outer surface of `face`'s mask: the board's face.
    pub(super) fn mask(&self, face: Face) -> f32 {
        match face {
            Face::Top => self.thickness,
            Face::Bottom => 0.0,
        }
    }

    /// The height of `face`'s silkscreen ink.
    pub(super) fn silk(&self, face: Face) -> f32 {
        match face {
            Face::Top => self.thickness + SILK_MM,
            Face::Bottom => -SILK_MM,
        }
    }

    /// Every copper layer of the stack, top to bottom.
    pub(super) fn copper_layers(&self) -> Vec<CopperLayer> {
        std::iter::once(CopperLayer::Top)
            .chain(self.inner.iter().map(|(layer, _)| *layer))
            .chain(std::iter::once(CopperLayer::Bottom))
            .collect()
    }

    /// How many inner copper layers move out on the top and on the bottom
    /// side when the board is taken apart.
    pub(super) fn inner_per_side(&self) -> [u8; 2] {
        let count = self.inner.len();
        let upper = count.div_ceil(2);
        [upper as u8, (count - upper) as u8]
    }

    /// The side inner layer `index` moves to and its steps from the core:
    /// the layers nearest the middle of the stack move least.
    pub(super) fn inner_place(&self, index: usize) -> (Face, u8) {
        let upper = self.inner.len().div_ceil(2);
        if index < upper {
            (Face::Top, (upper - index) as u8)
        } else {
            (Face::Bottom, (index - upper + 1) as u8)
        }
    }
}

/// Whether a stored stack can place the layers: two copper rows or more,
/// valid numbers and some thickness.
fn usable(stack: &LayerStack) -> bool {
    stack.copper_count() >= 2
        && stack.layers.iter().all(|row| row.numbers_valid())
        && stack.total_thickness_mm() > 0.0
}
