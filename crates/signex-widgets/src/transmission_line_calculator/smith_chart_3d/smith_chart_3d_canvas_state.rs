use iced::Point;

use crate::transmission_line_calculator::tool::smith_view_navigation::SmithViewNavigationState;

/// Stores drag rotation and shared navigation state for the 3D chart canvas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SmithChart3dCanvasState {
    pub(crate) drag_start: Option<Point>,
    pub(crate) drag_yaw: f32,
    pub(crate) drag_pitch: f32,
    pub(crate) navigation: SmithViewNavigationState,
}
