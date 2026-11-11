//! Semantic style references used by scene primitives.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Semantic color slots for schematic rendering.
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSlot {
    Wire = 0,
    Bus = 1,
    Junction = 2,
    SymbolBody = 3,
    Pin = 4,
    Selection = 5,
    Grid = 6,
    Snap = 7,
    ErcError = 8,
    ErcWarning = 9,
    ErcInfo = 10,
    Ghost = 11,
    LassoStroke = 12,
    LassoFill = 13,
    DrcError = 14,
    DrcWarning = 15,
    Ratsnest = 16,
    /// The board outline and the rim of every opening in it.
    BoardEdge = 17,
    /// A cutout or mounting hole: what shows through the board.
    BoardOpening = 18,
    /// A keepout's outline; its hatch is this colour, fainter.
    Keepout = 19,
    /// Top Layer copper.
    TopCopper = 20,
    /// Bottom Layer copper.
    BottomCopper = 21,
    /// The board palette's layer cycle, entry 0..=5: Mid-Layer `n` copper
    /// is entry `n % 6`.
    LayerCycle0 = 22,
    LayerCycle1 = 23,
    LayerCycle2 = 24,
    LayerCycle3 = 25,
    LayerCycle4 = 26,
    LayerCycle5 = 27,
    /// Through-hole pads and vias (the Multi-Layer).
    MultiLayer = 28,
    /// The board area inside the outline, under the copper.
    BoardArea = 29,
}

impl ColorSlot {
    /// The layer-cycle slots, in palette order.
    pub const LAYER_CYCLE: [ColorSlot; 6] = [
        ColorSlot::LayerCycle0,
        ColorSlot::LayerCycle1,
        ColorSlot::LayerCycle2,
        ColorSlot::LayerCycle3,
        ColorSlot::LayerCycle4,
        ColorSlot::LayerCycle5,
    ];
}

/// Compact style reference sent alongside primitive data.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StyleRef {
    pub slot: u16,
    pub flags: u16,
    pub alpha_mul: f32,
    pub _pad: f32,
}

impl StyleRef {
    pub fn new(slot: ColorSlot) -> Self {
        Self {
            slot: slot as u16,
            flags: 0,
            alpha_mul: 1.0,
            _pad: 0.0,
        }
    }
}
