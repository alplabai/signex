/// Canvas-side projection of an ERC violation — just enough to draw
/// its marker without pulling the full Violation type into the render
/// crate.
#[derive(Debug, Clone)]
pub struct ErcMarker {
    pub x: f64,
    pub y: f64,
    pub severity: ErcMarkerSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcMarkerSeverity {
    Error,
    Warning,
    Info,
}
