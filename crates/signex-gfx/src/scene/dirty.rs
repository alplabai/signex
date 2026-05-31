//! Dirty flag definitions for partial scene updates.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DirtyFlags: u32 {
        const LINES = 1 << 0;
        const CIRCLES = 1 << 1;
        const ARCS = 1 << 2;
        const POLYGONS = 1 << 3;
        const TEXT = 1 << 4;
        const GRID = 1 << 5;
        const OVERLAY = 1 << 6;
        const THEME = 1 << 7;
        const ALL = 0xFFFF;
    }
}

impl Default for DirtyFlags {
    fn default() -> Self {
        Self::empty()
    }
}
