//! The Ellipse, Elliptical Arc and Round Rectangle tools (#1267). Each takes
//! one snapped point per click through the annotation tool's buffer; the
//! last click places one drawing as one undo step. The clicks each tool
//! takes and the shape they give are the shared `CurveTool` in
//! `signex_types`, so the preview, the placement and the symbol editor's
//! gestures agree.

use signex_i18n::{MessageId, ids};
use signex_types::schematic::{CurveShape, CurveTool, FillType, Point, SchCurve, SchDrawing};

use super::super::super::*;
use super::pre_placement_shape;

/// The curve tool a schematic `tool` arms; `None` for every other tool.
pub(crate) fn curve_tool_of(tool: Tool) -> Option<CurveTool> {
    match tool {
        Tool::Ellipse => Some(CurveTool::Ellipse),
        Tool::EllipticalArc => Some(CurveTool::EllipticalArc),
        Tool::RoundRectangle => Some(CurveTool::RoundRect),
        _ => None,
    }
}

/// The hint bar's tool name.
pub(super) fn title_id(tool: CurveTool) -> MessageId {
    match tool {
        CurveTool::Ellipse => ids::PROPERTIES_ELLIPSE_TITLE,
        CurveTool::EllipticalArc => ids::PROPERTIES_ELLIPTICAL_ARC_TITLE,
        CurveTool::RoundRect => ids::PROPERTIES_ROUND_RECT_TITLE,
    }
}

/// What the click after `step` clicks does.
pub(super) fn hint_id(tool: CurveTool, step: usize) -> MessageId {
    match (tool, step) {
        (CurveTool::Ellipse, 0) => ids::CANVAS_HINT_ELLIPSE_CENTER,
        (CurveTool::Ellipse, _) => ids::CANVAS_HINT_ELLIPSE_CORNER,
        (CurveTool::RoundRect, 0) => ids::CANVAS_HINT_ROUND_RECT_FIRST,
        (CurveTool::RoundRect, _) => ids::CANVAS_HINT_ROUND_RECT_SECOND,
        (CurveTool::EllipticalArc, 0) => ids::CANVAS_HINT_ARC_CENTER,
        (CurveTool::EllipticalArc, 1) => ids::CANVAS_HINT_ARC_RADIUS_X,
        (CurveTool::EllipticalArc, 2) => ids::CANVAS_HINT_ARC_RADIUS_Y,
        (CurveTool::EllipticalArc, 3) => ids::CANVAS_HINT_ARC_START,
        (CurveTool::EllipticalArc, _) => ids::CANVAS_HINT_ARC_END,
    }
}

impl Signex {
    pub(super) fn curve_tool_point(&mut self, tool: CurveTool, p: Point) {
        let points = &mut self.interaction_state.annotation_tool.points;
        points.push(p);
        let clicked = if points.len() >= tool.clicks() {
            std::mem::take(points)
        } else {
            Vec::new()
        };
        // A shape with no visible size is not placed; the tool starts over.
        if let Some(shape) = tool.placed(&clicked) {
            let (width, fill) = pre_placement_shape(&self.document_state, &self.document_view);
            self.place_curve_at(shape, width, fill);
        }
        self.sync_annotation_draft();
    }

    /// Place `shape` as one drawing and one undo step: the id it got, or
    /// `None` when nothing was placed. An open arc takes no fill.
    pub(crate) fn place_curve_at(
        &mut self,
        shape: CurveShape,
        width: f64,
        fill: FillType,
    ) -> Option<uuid::Uuid> {
        let mut curve = SchCurve::new(uuid::Uuid::now_v7(), shape, width);
        if shape.is_closed() {
            curve.fill = fill;
        }
        let id = curve.uuid;
        self.apply_engine_command(
            signex_engine::Command::PlaceSchDrawing {
                drawing: SchDrawing::Curve(curve),
            },
            false,
            false,
        )
        .then_some(id)
    }
}
