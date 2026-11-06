//! The drawing-shape tools (Line, Rectangle, Circle, Arc, Polyline): one
//! point-by-point state machine behind a click, a double-click and Enter.

use signex_types::schematic::Point;

use super::super::super::*;
use super::pre_placement_shape;
use crate::canvas::ShapePreviewKind;

/// Which gesture delivered the point to a shape tool.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClickKind {
    Click,
    DoubleClick,
}

/// A double-click within this distance (mm) of the last polyline vertex adds
/// no vertex of its own.
const POLYLINE_DUPLICATE_MM: f64 = 0.01;

impl Signex {
    /// Feed one snapped point to a shape tool. `false` when `tool` draws no
    /// shape, so the caller handles the click itself.
    ///
    /// A double-click replaces the second click of a two-click shape, so it
    /// commits exactly as a click does but never seeds an anchor, and it
    /// closes a polyline.
    pub(super) fn shape_tool_point(&mut self, tool: Tool, p: Point, kind: ClickKind) -> bool {
        let seeds = kind == ClickKind::Click;
        match tool {
            Tool::Line => match self.interaction_state.shape_anchor.take() {
                None if seeds => self.set_shape_anchor(Some((p, ShapePreviewKind::Line))),
                None => {}
                Some(start) => {
                    let (width, _) = pre_placement_shape(&self.document_state);
                    self.place_line_at(start, p, width);
                    // Chained segments share the vertex just placed.
                    self.set_shape_anchor(Some((p, ShapePreviewKind::Line)));
                }
            },
            Tool::Rectangle => match self.interaction_state.shape_anchor.take() {
                None if seeds => self.set_shape_anchor(Some((p, ShapePreviewKind::Rect))),
                None => {}
                Some(start) => {
                    let (width, fill) = pre_placement_shape(&self.document_state);
                    self.place_rect_at(start, p, width, fill);
                    self.set_shape_anchor(None);
                }
            },
            Tool::Circle => match self.interaction_state.shape_anchor.take() {
                None if seeds => self.set_shape_anchor(Some((p, ShapePreviewKind::Circle))),
                None => {}
                Some(center) => {
                    // A radius too small to see is not placed.
                    let radius = (p.x - center.x).hypot(p.y - center.y);
                    let (width, fill) = pre_placement_shape(&self.document_state);
                    self.place_circle_at(center, radius, width, fill);
                    self.set_shape_anchor(None);
                }
            },
            Tool::Arc => {
                self.interaction_state.arc_points.push(p);
                if self.interaction_state.arc_points.len() >= 3 {
                    let pts = std::mem::take(&mut self.interaction_state.arc_points);
                    let (width, fill) = pre_placement_shape(&self.document_state);
                    self.place_arc_at([pts[0], pts[1], pts[2]], width, fill);
                }
                self.sync_point_buffers_to_canvas();
            }
            Tool::Polyline if seeds => {
                self.interaction_state.polyline_points.push(p);
                self.sync_point_buffers_to_canvas();
            }
            Tool::Polyline => {
                let is_duplicate =
                    self.interaction_state
                        .polyline_points
                        .last()
                        .is_some_and(|last| {
                            (last.x - p.x).abs() < POLYLINE_DUPLICATE_MM
                                && (last.y - p.y).abs() < POLYLINE_DUPLICATE_MM
                        });
                if !is_duplicate {
                    self.interaction_state.polyline_points.push(p);
                }
                if self.interaction_state.polyline_points.len() >= 2 {
                    self.commit_polyline();
                }
                // The canvas drops its vertices even when the app keeps a
                // lone one.
                let canvas = self.interaction_state.active_canvas_mut();
                canvas.polyline_points.clear();
                canvas.clear_overlay_cache();
            }
            _ => return false,
        }
        true
    }

    /// Place the buffered polyline vertices as one drawing, and empty the
    /// buffer. The caller has checked there are at least two.
    pub(crate) fn commit_polyline(&mut self) {
        let points = std::mem::take(&mut self.interaction_state.polyline_points);
        let (width, fill) = pre_placement_shape(&self.document_state);
        self.place_polyline_at(points, width, fill);
    }
}
