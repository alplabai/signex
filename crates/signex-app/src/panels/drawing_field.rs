//! Which drawing field is being edited: plain state, UI-free (ADR-0006).

/// Stable identifiers for every numeric drawing-field editor so the
/// panel keeps a transient string buffer per field across rerenders.
/// Erasing a text_input leaves an empty string in the buffer until
/// the user types a valid f64, at which point UpdateDrawingEdit fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingFieldId {
    LineStartX,
    LineStartY,
    LineEndX,
    LineEndY,
    LineWidth,
    RectStartX,
    RectStartY,
    RectWidth,
    RectHeight,
    RectBorder,
    CircleCenterX,
    CircleCenterY,
    CircleRadius,
    CircleBorder,
    ArcCenterX,
    ArcCenterY,
    ArcRadius,
    ArcStartAngle,
    ArcEndAngle,
    ArcWidth,
    PolyBorder,
}

/// The drawing field whose typed value is previewed on a sheet, not yet an
/// undo step (#2768). The preview itself is in that sheet's engine.
#[derive(Debug, Clone)]
pub(crate) struct LiveDrawingEdit {
    pub(crate) path: std::path::PathBuf,
    pub(crate) drawing: uuid::Uuid,
    pub(crate) field: DrawingFieldId,
    /// Whether Properties was open when typing began: closing it commits.
    pub(crate) panel_open: bool,
}
