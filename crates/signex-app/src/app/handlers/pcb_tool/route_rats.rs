//! The ratsnest follows the route end while routing (Caner, 2026-11-21).
//! While a track run or route of a net is drawn, the board leaves out that
//! net's lines from the copper the route started on, and the route on the
//! pointer draws one line from its end to the nearest pad of the net the
//! start does not reach. The split is `pcb_geom::route_rats`, computed when
//! the route starts or the board changes under it; a pointer move only
//! looks up the nearest pad. When the route lands or is dropped the split
//! goes and the board's lines are drawn whole again.

use pcb_geom::{PointNm, RatLine};
use signex_types::pcb::{PcbBoard, Point};

use super::BoardToolSession;
use crate::app::Signex;
use crate::app::state::RouteRatsKey;

impl Signex {
    /// What the split of the route drawn on the board in view is keyed by;
    /// `None` without a route of a net.
    fn route_rats_key(&self) -> Option<RouteRatsKey> {
        let session = self.own_board_tool()?;
        let net = session.route?.net;
        if net == 0 {
            return None;
        }
        let start = PointNm::from_mm(*session.points.first()?, "route start").ok()?;
        let revision = *self.document_state.pcb_revisions.get(&session.path)?;
        Some(RouteRatsKey {
            board: (session.path.clone(), revision),
            net,
            start,
        })
    }

    /// Keep the split in step with the route: computed when a route starts
    /// or its board changed, dropped when it ends, and the board in view
    /// redrawn then. Otherwise a key compare.
    pub(super) fn sync_route_ratsnest(&mut self) {
        let key = self.route_rats_key();
        let held = self.interaction_state.pcb_ratsnest.route_key();
        if held == key.as_ref() {
            return;
        }
        let touched = key
            .as_ref()
            .or(held)
            .map(|k| k.board.0.clone())
            .unwrap_or_default();
        let ratsnest = &mut self.interaction_state.pcb_ratsnest;
        match key.and_then(|k| self.document_state.pcb_board(&k.board.0).map(|b| (k, b))) {
            Some((key, board)) => ratsnest.set_route(key, board),
            None => ratsnest.clear_route(),
        }
        if self.active_tab_path().as_deref() == Some(touched.as_path()) {
            self.sync_pcb_canvas_from_visible_board();
        }
    }

    /// The routed net's ratsnest line from the end of the route on the
    /// pointer (`preview`, the route as drawn), while the ratsnest is shown.
    pub(super) fn route_ratsnest_line(
        &self,
        session: &BoardToolSession,
        preview: &PcbBoard,
    ) -> Option<RatLine> {
        if !self.pcb_layer_view(&session.path).ratsnest_drawn() {
            return None;
        }
        let end = route_end(preview, session.cursor?);
        let end = PointNm::from_mm(end, "route end").ok()?;
        self.interaction_state
            .pcb_ratsnest
            .route_line(&session.path, end)
    }
}

/// The end of the drawn route nearest the pointer: where the route is now.
/// The pointer itself before any leg is drawn.
fn route_end(preview: &PcbBoard, cursor: Point) -> Point {
    let far = |p: &Point| (p.x - cursor.x).hypot(p.y - cursor.y);
    preview
        .segments
        .iter()
        .flat_map(|s| [s.start, s.end])
        .min_by(|a, b| far(a).total_cmp(&far(b)))
        .unwrap_or(cursor)
}
