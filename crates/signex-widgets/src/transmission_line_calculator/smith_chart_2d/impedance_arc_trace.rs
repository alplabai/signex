use crate::transmission_line_calculator::Complex;
use iced::Color;

/// Stores a solved component arc and its styling for the 2D Smith chart.
#[derive(Debug, Clone)]
pub(crate) struct ImpedanceArcTrace {
    pub(crate) label: String,
    pub(crate) color: Color,
    pub(crate) points: Vec<Complex>,
}
