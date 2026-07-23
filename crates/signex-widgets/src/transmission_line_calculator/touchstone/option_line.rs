use crate::transmission_line_calculator::ScalarUnit;

use super::TouchstoneFormat;

/// Stores values parsed from the Touchstone option line.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OptionLine {
    pub(super) frequency_unit: ScalarUnit,
    pub(super) format: TouchstoneFormat,
    pub(super) reference_impedances_ohm: Vec<f64>,
}
