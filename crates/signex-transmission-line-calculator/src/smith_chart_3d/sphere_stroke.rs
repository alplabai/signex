use iced::Color;

use super::ProjectedPoint;

#[derive(Debug, Clone)]
pub(crate) struct SphereStroke {
    pub(crate) points: Vec<ProjectedPoint>,
    pub(crate) color: Color,
    pub(crate) width: f32,
    pub(crate) depth: f32,
}
