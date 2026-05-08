//! Multi-click sketch-tool gesture state — `PlacementInput`,
//! `PlacementInputKind`, and `PlaceArcPending` for in-flight tool
//! state across canvas frames.

use super::tool::{SketchTool, ToolPending};

/// v0.24 Phase 1 (Track D stub) — numeric-input overlay state for
/// sketch-tool placement.
#[derive(Debug, Clone)]
pub struct PlacementInput {
    /// User-typed digits (and optional decimal point / minus).
    pub buffer: String,
    /// Which dimension the buffer represents.
    pub kind: PlacementInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementInputKind {
    /// Line tool — second click commits at exactly `buffer` mm from
    /// the first endpoint, along the cursor's azimuth.
    LineLength,
    /// Circle tool — radius commit; second click ignores cursor delta.
    CircleRadius,
    /// Arc tool radius — second click ignores cursor delta from centre.
    ArcRadius,
    /// Arc tool sweep angle (degrees) — third click commits at the
    /// typed sweep relative to start.
    ArcSweep,
    /// v0.25 polish — Offset tool: typed buffer is the offset distance.
    OffsetDistance,
}

impl PlacementInputKind {
    /// v0.24 Track D — pick the matching numeric-input kind for the
    /// active sketch tool + pending state.
    pub fn from_active_tool(tool: SketchTool, pending: &ToolPending) -> Option<Self> {
        match (tool, pending) {
            (SketchTool::Line, ToolPending::LineFirst { .. }) => Some(Self::LineLength),
            (SketchTool::Circle, ToolPending::CircleCenter { .. }) => Some(Self::CircleRadius),
            (SketchTool::Arc, ToolPending::ArcCenter { .. }) => Some(Self::ArcRadius),
            (SketchTool::Arc, ToolPending::ArcStart { .. }) => Some(Self::ArcSweep),
            (SketchTool::Offset, _) => Some(Self::OffsetDistance),
            _ => None,
        }
    }

    /// `true` when the buffer accepts a leading minus sign.
    pub fn allows_negative(self) -> bool {
        matches!(self, Self::ArcSweep)
    }

    /// Short label rendered in the cursor overlay.
    pub fn label(self) -> &'static str {
        match self {
            Self::LineLength => "len",
            Self::CircleRadius | Self::ArcRadius => "r",
            Self::ArcSweep => "deg",
            Self::OffsetDistance => "dist",
        }
    }
}

/// v0.18.15.3 — Place Arc 3-click gesture state machine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PlaceArcPending {
    #[default]
    Idle,
    /// First click — centre stashed.
    Center { center: (f64, f64) },
    /// Second click — start point stashed.
    Start {
        center: (f64, f64),
        start: (f64, f64),
    },
}
