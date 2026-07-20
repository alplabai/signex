//! Pads-mode + sketch-mode tool enums and the multi-click pending-tool
//! state.

/// Pads-mode drawing tool — v0.15. The Pads-mode active bar's
/// "Place Pad" button switches to `PlacePad`; right-click cancels
/// back to `Select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadsTool {
    #[default]
    Select,
    /// Click empty canvas → adds a new pad at the cursor.
    PlacePad,
    /// v0.18.12 — non-plated through hole. 1-click drop.
    PlaceHole,
    /// v0.18.15 — silk-layer text placeholder. 1-click drop.
    PlaceString,
    /// v0.14 — silk-layer text with a drag-defined bounding box
    /// (item ③). Press-drag-release on empty canvas: press sets the
    /// anchor, drag previews a ghost rectangle, release computes
    /// w×h and commits a framed `Text`. Text is aligned/clipped
    /// inside the frame at render time — no auto-wrap/reflow.
    PlaceTextFrame,
    /// v0.18.15.1 — silk-layer line. 2-click gesture.
    PlaceTrack,
    /// v0.18.15.3 — silk-layer arc. 3-click gesture.
    PlaceArc,
    /// v0.18.15.4 — silk-layer closed-loop polygon.
    PlacePolygon,
    /// v0.18.17 — silk-layer filled region.
    PlaceRegion,
    /// v0.13 — through-hole via.
    PlaceVia,
}

/// Sketch-mode drawing tool. Phase 6.3 (v0.13.1) shipped Place Point
/// only; v0.13.2 adds Line, Circle, Arc; v0.15 adds Rectangle; v0.16
/// adds RoundedRectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SketchTool {
    #[default]
    Select,
    Point,
    Line,
    Rectangle,
    RoundedRectangle,
    Circle,
    Arc,
    /// v0.22 Phase B1 — Mirror tool.
    Mirror,
    /// v0.22 Phase B2 — Offset tool.
    Offset,
    /// v0.22 Phase B3 — Rectangular Pattern tool.
    RectPattern,
    /// v0.22 Phase B4 — Circular (Polar) Pattern tool.
    CircularPattern,
    /// v0.24 Track C — Tangent Arc tool.
    TangentArc,
    /// v0.27 — Fillet two adjacent lines with a tangent arc. Click
    /// the first line, then the second; the dispatcher splices a
    /// constant-radius arc across the corner and shortens the lines.
    /// EDA use: rounding silk / courtyard / pad-from-rect corners
    /// without redrawing them by hand.
    Fillet,
    /// v0.27 — Trim a segment of a line/arc up to its nearest
    /// intersections with other sketch entities. Click the segment
    /// you want gone. EDA use: cleaning up overlapping silk/outline
    /// without manual delete + redraw.
    Trim,
    /// #361 — "Drag Track End" (Place ▸ Drag Track End). An
    /// endpoint-biased segment grab: a left-press anywhere on a sketch
    /// `Line` grabs that line's NEARER endpoint and drives the existing
    /// Point-drag path (`try_drag_track_end_grab` → `DragState.sketch_point`
    /// in `canvas/input/`), so the endpoint follows the cursor with the
    /// solver live — no 12 px point-snap precision required. Not a
    /// placement tool: it never mints geometry, it only re-arms the
    /// endpoint drag. Arc / circle endpoints are out of scope.
    DragTrackEnd,
}

/// Transient per-tool gesture state. The canvas Program reads + writes
/// it through editor messages so the iced update loop can persist it
/// across renders without coupling the canvas's internal `cstate`
/// (which is local to the canvas program) to the editor's serialised
/// state.
#[derive(Debug, Clone, Default)]
pub enum ToolPending {
    #[default]
    Idle,
    /// Line tool, first click landed.
    LineFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Rectangle tool, first corner click landed. v0.15.
    RectangleFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Rounded-Rectangle tool, first corner click landed. v0.16.
    RoundedRectangleFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Circle tool, centre click landed.
    CircleCenter {
        center: signex_sketch::id::SketchEntityId,
    },
    /// Arc tool, centre click landed.
    ArcCenter {
        center: signex_sketch::id::SketchEntityId,
    },
    /// Arc tool, centre + start clicks landed; awaiting end click.
    ArcStart {
        center: signex_sketch::id::SketchEntityId,
        start: signex_sketch::id::SketchEntityId,
    },
    /// v0.23 — "Re-pick centre" affordance from the Pattern Properties
    /// sub-form.
    RepickPolarCenter {
        array_id: signex_sketch::array::ArrayId,
    },
    /// v0.24 Track C — Tangent Arc, first endpoint placed.
    TangentArcFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// v0.27 — Fillet, first Line picked. Awaiting second Line.
    FilletFirst {
        line: signex_sketch::id::SketchEntityId,
    },
}
