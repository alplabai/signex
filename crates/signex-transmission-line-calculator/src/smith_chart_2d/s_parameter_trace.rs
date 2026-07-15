use crate::Complex;
use iced::Color;

#[derive(Debug, Clone)]
pub(crate) struct SParameterTrace {
    pub(crate) label: &'static str,
    pub(crate) color: Color,
    pub(crate) points: Vec<Complex>,
}
