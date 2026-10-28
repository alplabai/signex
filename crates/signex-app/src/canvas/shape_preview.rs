#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePreviewKind {
    Line,
    Rect,
    Circle,
    /// Measure tool: a line with a live dx / dy / distance readout.
    Measure,
}

/// The way a selection box was dragged, by where it ended against where it
/// started (#1257).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxDirection {
    LeftToRight,
    RightToLeft,
}

impl BoxDirection {
    /// The direction of a drag from `start_x` to `end_x` (world x grows right).
    pub fn of_drag(start_x: f64, end_x: f64) -> Self {
        if end_x < start_x {
            Self::RightToLeft
        } else {
            Self::LeftToRight
        }
    }
}

/// A finished measurement kept on the canvas until Esc or the next click.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasureOverlay {
    pub a: signex_types::schematic::Point,
    pub b: signex_types::schematic::Point,
}
