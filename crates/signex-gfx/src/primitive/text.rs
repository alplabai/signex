//! Text primitive type.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

/// Text item to be consumed by a text pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextHAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Debug, Default)]
pub struct TextItem {
    pub content: String,
    pub position: [f32; 2],
    pub size_mm: f32,
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
    pub rotation: f32,
    pub h_align: TextHAlign,
    pub v_align: TextVAlign,
}
