use iced::Color;

use super::ProjectedPoint;

/// Describes a depth-sorted polyline drawn on the Smith sphere.
#[derive(Debug, Clone)]
pub(crate) struct SphereStroke {
    pub(crate) points: Vec<ProjectedPoint>,
    pub(crate) color: Color,
    pub(crate) width: f32,
    pub(crate) depth: f32,
}
