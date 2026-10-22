use super::*;

#[derive(Debug, Default)]
pub(in crate::gerber_viewer) struct GerberCanvasState {
    pub(super) drag_start: Option<Point>,
    pub(super) zoom_selection_start: Option<Point>,
    pub(super) zoom_selection_current: Option<Point>,
    pub(super) item_selection_start: Option<Point>,
    pub(super) item_selection_current: Option<Point>,
    pub(super) measurement_dragging: bool,
}
