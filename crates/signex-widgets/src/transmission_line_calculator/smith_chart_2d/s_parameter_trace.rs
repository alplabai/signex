use crate::transmission_line_calculator::Complex;
use iced::Color;

/// Stores one sampled S-parameter trace and its Smith-chart styling.
#[derive(Debug, Clone)]
pub(crate) struct SParameterTrace {
    pub(crate) label: &'static str,
    pub(crate) color: Color,
    pub(crate) points: Vec<Complex>,
}
