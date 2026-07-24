use std::fmt;

/// Selects the two-dimensional, admittance, or three-dimensional chart view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmithChartDiagramMode {
    TwoDimensional,
    AdmittanceTwoDimensional,
    ThreeDimensional,
}

impl SmithChartDiagramMode {
    pub const ALL: [Self; 3] = [
        Self::TwoDimensional,
        Self::AdmittanceTwoDimensional,
        Self::ThreeDimensional,
    ];
}

impl Default for SmithChartDiagramMode {
    /// Creates the default value for this type.
    fn default() -> Self {
        Self::TwoDimensional
    }
}

impl fmt::Display for SmithChartDiagramMode {
    /// Formats the value for user-facing display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TwoDimensional => formatter.write_str("2D Smith Chart"),
            Self::AdmittanceTwoDimensional => formatter.write_str("2D Y Smith Chart"),
            Self::ThreeDimensional => formatter.write_str("3D Smith Chart"),
        }
    }
}
