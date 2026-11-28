//! Pads-mode + sketch-mode tool enums and the multi-click pending-tool
//! state.

/// Pads-mode drawing tool. The Pads-mode active bar's
/// "Place Pad" button switches to `PlacePad`; right-click cancels
/// back to `Select`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadsTool {
    #[default]
    Select,
    /// Click empty canvas → adds a new pad at the cursor.
    PlacePad,
    /// Non-plated through hole. 1-click drop.
    PlaceHole,
    /// Silk-layer text placeholder. 1-click drop.
    PlaceString,
    /// Silk-layer text with a drag-defined bounding box
    /// (item ③). Press-drag-release on empty canvas: press sets the
    /// anchor, drag previews a ghost rectangle, release computes
    /// w×h and commits a framed `Text`. Text is aligned/clipped
    /// inside the frame at render time — no auto-wrap/reflow.
    PlaceTextFrame,
    /// Silk-layer line. 2-click gesture.
    PlaceTrack,
    /// Silk-layer arc. 3-click gesture.
    PlaceArc,
    /// Silk-layer closed-loop polygon.
    PlacePolygon,
    /// Silk-layer filled region.
    PlaceRegion,
    /// Through-hole via.
    PlaceVia,
}

/// Instances a new Linear Pattern makes; edited in the Pattern sub-form.
pub const LINEAR_PATTERN_DEFAULT_COUNT: u32 = 4;
/// Pitch of a new Linear Pattern along X when no dimension is typed.
pub const LINEAR_PATTERN_DEFAULT_SPACING_MM: f64 = 5.0;

/// Sketch-mode drawing tool.
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
    /// #467 — 3-point Edge Arc: click start, click end, click a third
    /// point the arc passes through. Centre + radius are derived from
    /// the circumcircle of the three picks (shared solver, #461/#483)
    /// instead of the centre-first `Arc` tool's gesture.
    EdgeArc,
    /// Mirror tool.
    Mirror,
    /// Offset tool.
    Offset,
    /// Rectangular Pattern tool.
    RectPattern,
    /// Circular (Polar) Pattern tool.
    CircularPattern,
    /// Tangent Arc tool.
    TangentArc,
    /// Fillet two adjacent lines with a tangent arc. Click
    /// the first line, then the second; the dispatcher splices a
    /// constant-radius arc across the corner and shortens the lines.
    /// EDA use: rounding silk / courtyard / pad-from-rect corners
    /// without redrawing them by hand.
    Fillet,
    /// Trim a segment of a line/arc up to its nearest
    /// intersections with other sketch entities. Click the segment
    /// you want gone. EDA use: cleaning up overlapping silk/outline
    /// without manual delete + redraw.
    Trim,
    /// #372 — Break Track. Single click on a sketch `Line`: hit-test
    /// the click against every Line (0.30 mm tolerance, nearest stroke
    /// wins — the same pick as Trim), project it onto that Line to a
    /// parameter `t`, and call the `split_line` primitive (#360) to
    /// divide the Line into two Lines meeting at a new mid `Point`. EDA
    /// use: dropping a vertex into a silk / courtyard / outline segment
    /// so each half can be dragged or re-routed independently — unlike
    /// Trim (which deletes a segment), Break Track keeps both halves.
    /// Splits only sketch Lines — arcs, circles, pads and regions are
    /// out of scope.
    BreakTrack,
    /// #361 — "Drag Track End" (Place ▸ Drag Track End). An
    /// endpoint-biased segment grab: a left-press anywhere on a sketch
    /// `Line` grabs that line's NEARER endpoint and drives the existing
    /// Point-drag path (`try_drag_track_end_grab` → `DragState.sketch_point`
    /// in `canvas/input/`), so the endpoint follows the cursor with the
    /// solver live — no 12 px point-snap precision required. Not a
    /// placement tool: it never mints geometry, it only re-arms the
    /// endpoint drag. Arc / circle endpoints are out of scope.
    DragTrackEnd,
    /// Chamfer two Lines meeting at a corner: click the first Line, then
    /// the second; both are cut back by the chamfer distance and a Line
    /// joins the cut ends. See docs/FOOTPRINT_SKETCH.md, "Modify tools".
    Chamfer,
    /// Extend a Line: click near one end and it grows to the nearest
    /// entity it reaches, the inverse of Trim.
    Extend,
    /// Linear Pattern: click the source, a one-row `ArrayKind::Linear`.
    LinearPattern,
    /// Slot: click one end's centre, then the other's (the dimension input
    /// is the centre distance), then a third click sets the half width.
    Slot,
    /// Regular polygon: click the centre, then the first vertex; the
    /// dimension input is the side count (empty is 6).
    Polygon,
}

impl SketchTool {
    /// The tools that place new geometry, from the Create menu. Holding
    /// Ctrl while one is armed places without inference (mockup row 188).
    pub fn is_drawing(self) -> bool {
        matches!(
            self,
            Self::Point
                | Self::Line
                | Self::Rectangle
                | Self::RoundedRectangle
                | Self::Circle
                | Self::Arc
                | Self::EdgeArc
                | Self::TangentArc
                | Self::Slot
                | Self::Polygon
        )
    }
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
    /// Rectangle tool, first corner click landed.
    RectangleFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Rounded-Rectangle tool, first corner click landed.
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
    /// #467 — Edge Arc tool, start click landed; awaiting end click.
    EdgeArcStart {
        start: signex_sketch::id::SketchEntityId,
    },
    /// #467 — Edge Arc tool, start + end clicks landed; awaiting the
    /// third "point on arc" click. That click's position is fed into
    /// the circumcircle of (start, third, end) to derive the centre +
    /// sweep direction; collinear picks are rejected via
    /// `solve_warnings` instead of minting a degenerate arc.
    EdgeArcEnd {
        start: signex_sketch::id::SketchEntityId,
        end: signex_sketch::id::SketchEntityId,
    },
    /// "Re-pick centre" affordance from the Pattern Properties
    /// sub-form. With `is_kind_switch` the pick switches a Linear or Grid
    /// pattern to Polar about the picked Point (#2596).
    RepickPolarCenter {
        array_id: signex_sketch::array::ArrayId,
        is_kind_switch: bool,
    },
    /// Tangent Arc, first endpoint placed.
    TangentArcFirst {
        first: signex_sketch::id::SketchEntityId,
    },
    /// Fillet, first Line picked. Awaiting second Line.
    FilletFirst {
        line: signex_sketch::id::SketchEntityId,
    },
    /// Chamfer, first Line picked. Awaiting the second Line.
    ChamferFirst {
        line: signex_sketch::id::SketchEntityId,
    },
    /// Slot, the first centre placed; awaiting the second.
    SlotFirst {
        a: signex_sketch::id::SketchEntityId,
    },
    /// Slot, both centres placed; the next click sets the half width.
    SlotSecond {
        a: signex_sketch::id::SketchEntityId,
        b: signex_sketch::id::SketchEntityId,
    },
    /// Polygon, the centre placed; the next click is the first vertex.
    PolygonCentre {
        centre: signex_sketch::id::SketchEntityId,
    },
}
