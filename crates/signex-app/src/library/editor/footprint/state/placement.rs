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
    /// Line tool — second click pins the segment azimuth to exactly
    /// `buffer` degrees, measured CCW from the +X axis (standard math
    /// convention, world-space). Toggled in via Tab while a Line
    /// placement-input buffer is active; pairs with `LineLength` so
    /// the user can dial in length and angle independently.
    LineAngle,
    /// Rectangle / Rounded-Rectangle tool — second click pins the box
    /// width (mm) along X from the first corner; the sign follows the
    /// cursor's quadrant. Tab-paired with `RectHeight`.
    RectWidth,
    /// Rectangle / Rounded-Rectangle tool — second click pins the box
    /// height (mm) along Y from the first corner; sign follows the
    /// cursor's quadrant. Tab-paired with `RectWidth`.
    RectHeight,
    /// Rounded-Rectangle tool — corner radius (mm) for the commit,
    /// overriding the legacy `dimension_input` source. Third field in
    /// the Rounded-Rect Tab cycle (w → h → r).
    RRectRadius,
    /// Circle tool — radius commit; second click ignores cursor delta.
    CircleRadius,
    /// Arc tool radius — second click ignores cursor delta from centre.
    ArcRadius,
    /// Arc tool sweep angle (degrees) — third click commits at the
    /// typed sweep relative to start.
    ArcSweep,
    /// v0.25 polish — Offset tool: typed buffer is the offset distance.
    OffsetDistance,
    /// v0.27 — Fillet tool: typed buffer is the fillet radius (mm).
    FilletRadius,
}

impl PlacementInputKind {
    /// v0.14-footprint — the ordered Tab-cycle of typed dimension
    /// fields for a tool's current gesture stage. More than one element
    /// for tools whose shape is defined by several dimensions at the
    /// SAME commit click (Line len/angle, Rectangle w/h, Rounded-Rect
    /// w/h/radius); single-element for radius/sweep/distance tools;
    /// empty for tools that take no typed dimensions. Single source of
    /// truth for `from_active_tool` and the Tab field-cycle.
    pub fn placement_fields(tool: SketchTool, pending: &ToolPending) -> Vec<Self> {
        match (tool, pending) {
            (SketchTool::Line, ToolPending::LineFirst { .. }) => {
                vec![Self::LineLength, Self::LineAngle]
            }
            (SketchTool::Rectangle, ToolPending::RectangleFirst { .. }) => {
                vec![Self::RectWidth, Self::RectHeight]
            }
            (SketchTool::RoundedRectangle, ToolPending::RoundedRectangleFirst { .. }) => {
                vec![Self::RectWidth, Self::RectHeight, Self::RRectRadius]
            }
            (SketchTool::Circle, ToolPending::CircleCenter { .. }) => vec![Self::CircleRadius],
            (SketchTool::Arc, ToolPending::ArcCenter { .. }) => vec![Self::ArcRadius],
            (SketchTool::Arc, ToolPending::ArcStart { .. }) => vec![Self::ArcSweep],
            (SketchTool::Offset, _) => vec![Self::OffsetDistance],
            (SketchTool::Fillet, _) => vec![Self::FilletRadius],
            _ => vec![],
        }
    }

    /// v0.24 Track D — the default focused field when a gesture stage
    /// opens: the first of `placement_fields`. Drives the canvas
    /// keyboard guard and the kind minted for the first typed digit.
    pub fn from_active_tool(tool: SketchTool, pending: &ToolPending) -> Option<Self> {
        Self::placement_fields(tool, pending).first().copied()
    }

    /// `true` for fields that belong to a multi-field Tab cycle (Line
    /// len/angle, Rectangle w/h, Rounded-Rect w/h/radius). Tab cycles
    /// these while a buffer is active; for single-field kinds Tab keeps
    /// its placement-pause role.
    pub fn is_tab_switchable(self) -> bool {
        matches!(
            self,
            Self::LineLength
                | Self::LineAngle
                | Self::RectWidth
                | Self::RectHeight
                | Self::RRectRadius
        )
    }

    /// `true` when the buffer accepts a leading minus sign.
    pub fn allows_negative(self) -> bool {
        matches!(self, Self::ArcSweep | Self::LineAngle)
    }

    /// Short label rendered in the cursor overlay.
    pub fn label(self) -> &'static str {
        match self {
            Self::LineLength => "len",
            Self::LineAngle => "ang",
            Self::CircleRadius | Self::ArcRadius => "r",
            Self::ArcSweep => "deg",
            Self::OffsetDistance => "dist",
            Self::FilletRadius => "r",
            Self::RectWidth => "w",
            Self::RectHeight => "h",
            Self::RRectRadius => "r",
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
