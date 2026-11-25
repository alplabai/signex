/// Canvas-side projection of an ERC violation — just enough to draw
/// its marker without pulling the full Violation type into the render
/// crate.
#[derive(Debug, Clone)]
pub struct ErcMarker {
    pub x: f64,
    pub y: f64,
    pub severity: ErcMarkerSeverity,
    /// `true` for a net-level finding, drawn with the distinct net marker.
    pub net_level: bool,
    /// A short line drawn under the marker, such as `expects CS_ADC` by a
    /// mismatched harness entry (mockup row 152).
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcMarkerSeverity {
    Error,
    Warning,
    Info,
}
