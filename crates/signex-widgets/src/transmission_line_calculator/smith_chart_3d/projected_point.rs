use iced::Point;

/// Stores a sphere point projected into screen space together with its depth.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectedPoint {
    pub(crate) screen: Point,
    pub(crate) camera_z: f32,
}
