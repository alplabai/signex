//! The Track tool (#2373): a run drawn on the active copper layer, click by
//! click. The first click starts the run on the net of the copper under it;
//! each later click adds the legs from the run's end to the point, bent by
//! the corner rule of wires (#2372: Space flips the bend, Shift+Space steps
//! the corner mode), as one undo step. Making another copper layer active
//! mid-run puts a via at the run's end in the same step, and the run goes
//! on there. A click on the run's end, Esc or a right-click ends the run.

use signex_engine::pcb::draw::{self, TrackStyle};
use signex_types::pcb::{PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::BoardToolSession;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

impl Signex {
    /// Whether a track run is being drawn on the board this window shows:
    /// Space and Shift+Space bend it.
    pub(crate) fn is_routing_board_track(&self) -> bool {
        self.own_board_tool()
            .is_some_and(BoardToolSession::is_routing)
    }

    /// Space (flip the bend) or Shift+Space (next corner mode) mid-run.
    /// Whether a run took the key.
    pub(crate) fn bend_board_route(&mut self, next_mode: bool) -> bool {
        if !self.is_routing_board_track() {
            return false;
        }
        let rule = self.interaction_state.corner_rule;
        self.set_corner_rule(if next_mode {
            rule.with_next_mode()
        } else {
            rule.flipped()
        });
        true
    }

    /// A Track click at `at` (already on the grid or an object point).
    pub(super) fn route_clicked(&mut self, at: Point) {
        let Some(layer) = self.tool_copper_layer(BoardPlaceTool::Track) else {
            return;
        };
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let Some(anchor) = session.points.last().copied() else {
            let style = TrackStyle::starting_at(board, at, layer);
            self.set_route(vec![at], Some(style));
            return;
        };
        if at == anchor {
            // A second click on the run's end finishes the run.
            self.set_route(Vec::new(), None);
            return;
        }
        let (style, via) = next_leg_style(board, session, anchor, at, layer);
        let rule = self.interaction_state.corner_rule;
        let tracks = draw::track_run(anchor, at, rule, style, Uuid::now_v7);
        if let Some(command) = draw::place_route(tracks, via) {
            self.run_placement("Track", Ok(vec![command]));
        }
        let mut points = self
            .own_board_tool()
            .map(|s| s.points.clone())
            .unwrap_or_default();
        points.push(at);
        self.set_route(points, Some(style));
    }

    fn set_route(&mut self, points: Vec<Point>, style: Option<TrackStyle>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points = points;
            session.route = style;
        }
    }

    /// The legs the next click adds, and the via of a layer change, drawn
    /// into `preview`.
    pub(super) fn route_preview(
        &self,
        session: &BoardToolSession,
        board: &PcbBoard,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let (Some(anchor), Some(layer)) =
            (session.points.last().copied(), self.active_copper_layer())
        else {
            return;
        };
        let (style, via) = next_leg_style(board, session, anchor, cursor, layer);
        let rule = self.interaction_state.corner_rule;
        preview
            .segments
            .extend(draw::track_run(anchor, cursor, rule, style, Uuid::nil));
        preview.vias.extend(via);
    }
}

/// The style of the legs from `anchor` to `to` on `layer`, and the via at
/// `anchor` when `layer` is not the run's. A run with no net yet takes the
/// net of the copper under `to`.
fn next_leg_style(
    board: &PcbBoard,
    session: &BoardToolSession,
    anchor: Point,
    to: Point,
    layer: CopperLayer,
) -> (TrackStyle, Option<Via>) {
    let mut style = session
        .route
        .unwrap_or_else(|| TrackStyle::starting_at(board, anchor, layer));
    if style.net == 0 {
        style.net = draw::net_under(board, to, 1 << pcb_geom::layer_bit(layer));
    }
    let via = (style.layer != layer).then(|| Via {
        net: style.net,
        ..draw::new_via(board, Uuid::now_v7(), anchor)
    });
    style.layer = layer;
    (style, via)
}
