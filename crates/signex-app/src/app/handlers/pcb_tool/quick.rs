//! Quick Route (#2389): the first click starts on the copper under it, at
//! the width Interactive Routing would take (the net class, held within
//! the written rules); the second click routes straight there, bent only by
//! the corner rule, without avoiding other nets, and lands as one undo
//! step. The tool stays armed for the next route. Checking what it crossed
//! is the design rule check's job.

use signex_engine::pcb::draw;
use signex_engine::pcb::route::RoutedLeg;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::BoardToolSession;
use super::route::next_leg_style;
use super::router::rule_via;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

impl Signex {
    /// A Quick Route click at `at` (already on the grid or an object point).
    pub(super) fn quick_route_clicked(&mut self, at: Point) {
        let Some(layer) = self.tool_copper_layer(BoardPlaceTool::QuickRoute) else {
            return;
        };
        let Some(anchor) = self
            .own_board_tool()
            .map(|session| session.points.last().copied())
        else {
            return;
        };
        let Some(anchor) = anchor else {
            self.start_route(at, layer);
            return;
        };
        if !self.clicks_run_end(at, anchor) {
            let Some((board, session)) = self.active_pcb().zip(self.own_board_tool()) else {
                return;
            };
            let Some(router) = session.router.as_ref() else {
                return;
            };
            let defaults = &self.interaction_state.pcb_tool_defaults;
            let classes = self.track_tool_net_classes();
            let (style, via) = next_leg_style(board, session, defaults, anchor, at, layer, classes);
            let corner = self.interaction_state.corner_rule;
            let leg = RoutedLeg {
                tracks: draw::track_run(anchor, at, corner, style, Uuid::now_v7),
                via: via.map(|via| rule_via(via, &router.rules)),
                ..RoutedLeg::default()
            };
            if let Some(session) = self.own_board_tool_mut() {
                session.points.push(at);
                session.route = Some(style);
                if let Some(router) = session.router.as_mut() {
                    router.draft.add(leg);
                }
            }
        }
        // The target reached (or the start clicked again): the route lands.
        self.end_route_shape();
    }

    /// The straight legs from the start to the pointer, drawn into
    /// `preview`.
    pub(super) fn quick_route_preview(&self, session: &BoardToolSession, preview: &mut PcbBoard) {
        let (Some(anchor), Some(cursor), Some(layer)) = (
            session.points.last().copied(),
            session.cursor,
            self.active_copper_layer(),
        ) else {
            return;
        };
        let Some(board) = self.document_state.pcb_board(&session.path) else {
            return;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let classes = self.track_tool_net_classes();
        let (style, via) = next_leg_style(board, session, defaults, anchor, cursor, layer, classes);
        let corner = self.interaction_state.corner_rule;
        preview
            .segments
            .extend(draw::track_run(anchor, cursor, corner, style, Uuid::nil));
        preview.vias.extend(via);
    }
}
