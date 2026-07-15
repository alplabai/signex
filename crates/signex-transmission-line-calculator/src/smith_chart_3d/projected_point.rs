use iced::Point;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectedPoint {
    pub(crate) screen: Point,
    pub(crate) camera_z: f32,
}
