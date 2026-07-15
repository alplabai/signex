use iced::Point;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SmithChart3dCanvasState {
    pub(crate) drag_start: Option<Point>,
    pub(crate) drag_yaw: f32,
    pub(crate) drag_pitch: f32,
}
