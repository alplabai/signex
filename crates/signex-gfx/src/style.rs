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
