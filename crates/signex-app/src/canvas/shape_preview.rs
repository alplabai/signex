#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePreviewKind {
    Line,
    Rect,
    Circle,
    /// Measure tool: a line with a live dx / dy / distance readout.
    Measure,
}

/// A finished measurement kept on the canvas until Esc or the next click.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureOverlay {
    pub a: signex_types::schematic::Point,
    pub b: signex_types::schematic::Point,
}
