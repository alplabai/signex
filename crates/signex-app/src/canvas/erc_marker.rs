/// Canvas-side projection of an ERC violation — just enough to draw
/// its marker without pulling the full Violation type into the render
/// crate.
#[derive(Debug, Clone)]
pub struct ErcMarker {
    pub x: f64,
    pub y: f64,
    pub severity: ErcMarkerSeverity,
    /// Uuid of the primary offending item (label, wire, symbol, …).
    /// Reserved for a future halo/highlight pass; unused today.
    pub primary_uuid: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcMarkerSeverity {
    Error,
    Warning,
    Info,
}
