//! Phase 5.5 — `SketchEdit` / `SketchModeMsg` enums consumed by the
//! solve-on-edit dispatcher and emitted by the Phase 6 UI shell.
//!
//! Designed to be plain-data: the iced `update` path matches on
//! `SketchModeMsg` and dispatches to
//! [`super::super::sketch_dispatch::apply_sketch_edit`] for the
//! `Edit(_)` variant. Tool-state changes are local to the editor
//! state.

use signex_sketch::{
    constraint::Constraint,
    entity::Entity,
    id::{ConstraintId, SketchEntityId},
};

use super::super::state::EditorMode;

/// One atomic edit to a sketch. Ordering matters — the dispatcher
/// applies them in arrival order so a follow-up solve sees the
/// post-edit `SketchData`.
#[derive(Debug, Clone)]
pub enum SketchEdit {
    /// Append a new entity (Point / Line / Arc / Circle plus
    /// optional bake attributes already attached).
    AddEntity(Entity),
    /// Remove an entity by ID. Constraints / arrays referencing the
    /// entity are pruned by the dispatcher.
    DeleteEntity(SketchEntityId),
    /// Translate a free Point by `(dx, dy)` in plane-local mm. The
    /// solver re-runs after the move; if the move violates a
    /// constraint the LM step pulls the point back to the
    /// nearest-feasible position.
    MovePoint {
        id: SketchEntityId,
        dx: f64,
        dy: f64,
    },

    /// Append a new Constraint.
    AddConstraint(Constraint),
    /// Remove a Constraint by ID.
    DeleteConstraint(ConstraintId),

    /// Insert / overwrite a parameter source string. Triggers a
    /// resolve + solve.
    EditParameter { name: String, expr: String },
    /// Drop a parameter from the table.
    DeleteParameter { name: String },

    /// Switch the editor's high-level mode. Re-rendered by the
    /// canvas; no solve.
    SetMode(EditorMode),

    /// Force a solve + bake even if the sketch hasn't changed.
    ForceRebuild,
}

/// The active drawing tool inside Sketch mode. The Phase 6 UI's
/// tool palette emits `SetTool(...)` to switch the active tool;
/// the canvas uses this to interpret pointer events. The placeholder
/// list maps to the SKETCH_MODE_v0.13_PLAN.md §6.3 spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTool {
    /// No tool active — pointer pans / selects only.
    #[default]
    Select,
    /// Click to drop a Point.
    Point,
    /// Two clicks to drop a Line.
    Line,
    /// Three clicks (centre, start, end) to drop an Arc.
    Arc,
    /// Two clicks (centre, edge) to drop a Circle.
    Circle,
    /// Tap two entities + commit to drop a Distance / Angle dim.
    Dimension,
    /// Tap geometry to attach a constraint (sub-mode picks the kind).
    Constraint,
}

/// Pointer / keyboard events forwarded from the canvas into the
/// active tool. Phase 6 UI fills these in; v0.13 schema lands here so
/// the `SketchModeMsg::ToolEvent` arm can route them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolEvent {
    /// Pointer pressed at world coords `(x_mm, y_mm)`.
    Press { x_mm: f64, y_mm: f64 },
    /// Pointer released at world coords.
    Release { x_mm: f64, y_mm: f64 },
    /// Pointer moved to world coords (hover / drag).
    Hover { x_mm: f64, y_mm: f64 },
    /// Escape key — cancel the in-flight tool gesture.
    Cancel,
}

/// Top-level sketch-mode message routed by the iced `update` path.
#[derive(Debug, Clone)]
pub enum SketchModeMsg {
    /// One atomic edit to the underlying SketchData.
    Edit(SketchEdit),
    /// Switch the active drawing tool.
    ToolChanged(ActiveTool),
    /// Pointer / keyboard event for the active tool.
    ToolEvent(ToolEvent),
}
