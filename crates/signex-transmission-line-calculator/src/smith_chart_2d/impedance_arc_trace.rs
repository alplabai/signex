use crate::Complex;
use iced::Color;

#[derive(Debug, Clone)]
pub(crate) struct ImpedanceArcTrace {
    pub(crate) label: String,
    pub(crate) color: Color,
    pub(crate) points: Vec<Complex>,
}
