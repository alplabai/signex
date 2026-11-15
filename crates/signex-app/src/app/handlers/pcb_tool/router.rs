//! Interactive Routing (#2389) on the board in view. The first click starts
//! a route on the active copper layer, on the net of the copper under it,
//! with the width its net class (else the board) gives, held within the
//! written rules' limits. Each later click routes a leg to the point with
//! `signex_engine::pcb::route`: around other nets' copper at the rule
//! clearance, or pushing their tracks aside (`cycle_route_mode`), bent by
//! the corner rule of #2372; a layer change adds a via. The legs stay off
//! the board until the route ends (a click on its end, Esc, a right-click,
//! another tool), then land as one undo step.

use pcb_geom::CopperItem;
use signex_drc::DrcRules;
use signex_engine::pcb::draw::TrackStyle;
use signex_engine::pcb::placement::grid_step_mm;
use signex_engine::pcb::route::{
    LegRequest, RouteDraft, RouteMode, RouteRefusal, RouteRules, RoutedLeg, route_leg,
};
use signex_i18n::{MessageId, ids, t};
use signex_types::corner::CornerRule;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::BoardToolSession;
use super::route::next_leg_style;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// The route being drawn: its legs not yet on the board, and the rules
/// they keep, read once when the route starts.
#[derive(Debug, Clone, PartialEq)]
pub struct RouterState {
    pub draft: RouteDraft,
    pub rules: DrcRules,
}

impl Signex {
    /// An Interactive Routing click at `at` (already on the grid or an
    /// object point).
    pub(super) fn router_clicked(&mut self, at: Point) {
        let Some(layer) = self.tool_copper_layer(BoardPlaceTool::InteractiveRouting) else {
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
        if at == anchor {
            // A second click on the route's end finishes it.
            self.finish_board_route();
            self.drop_board_shape();
            return;
        }
        let Some((board, session)) = self.active_pcb().zip(self.own_board_tool()) else {
            return;
        };
        let Some(router) = session.router.as_ref() else {
            return;
        };
        let (style, via) = next_leg_style(board, session, anchor, at, layer);
        let leg = LegRequest {
            from: anchor,
            to: at,
            style,
            via,
        };
        let interaction = &self.interaction_state;
        let routed = route_on(
            board,
            router,
            leg,
            (interaction.corner_rule, interaction.route_mode),
            Uuid::now_v7,
        );
        match routed {
            Ok(leg) => {
                if let Some(session) = self.own_board_tool_mut() {
                    session.points.push(at);
                    session.route = Some(style);
                    if let Some(router) = session.router.as_mut() {
                        router.draft.add(leg);
                    }
                }
            }
            Err(refusal) => {
                let line = t!(self.ui_state.i18n, refusal_line(refusal));
                crate::diagnostics::log_warning(line.into_string());
            }
        }
    }

    /// The first click: read the board's rules and start the route at `at`.
    fn start_route(&mut self, at: Point, layer: CopperLayer) {
        let Some((path, board)) = self.active_tab_path().zip(self.active_pcb()) else {
            return;
        };
        let (rules, classes) = match self.board_design_rules(&path, board) {
            Ok(found) => found,
            Err(why) => {
                let line = t!(self.ui_state.i18n, ids::PCB_ROUTE_NO_RULES, "why" => why);
                crate::diagnostics::log_warning(line.into_string());
                return;
            }
        };
        let mut style = TrackStyle::starting_at(board, at, layer);
        style.width_mm = rule_width(board, &classes, &rules, &style);
        if let Some(session) = self.own_board_tool_mut() {
            session.points = vec![at];
            session.route = Some(style);
            session.router = Some(RouterState {
                draft: RouteDraft::default(),
                rules,
            });
        }
    }

    /// Land the route being drawn on the board as one undo step. Nothing
    /// when no route is drawn or it has no leg yet.
    pub(super) fn finish_board_route(&mut self) {
        let draft = self
            .own_board_tool_mut()
            .and_then(|session| session.router.as_mut())
            .map(|router| std::mem::take(&mut router.draft));
        let Some(draft) = draft.filter(|d| !d.is_empty()) else {
            return;
        };
        let what = t!(self.ui_state.i18n, ids::PCB_TOOL_ROUTE).into_string();
        self.run_placement(&what, Ok(draft.commands()));
    }

    /// `cycle_route_mode`: walk around other nets, or push their tracks.
    /// The Messages panel says which; a route being drawn redraws its next
    /// leg.
    pub(crate) fn step_route_mode(&mut self) {
        let mode = self.interaction_state.route_mode.next();
        self.interaction_state.route_mode = mode;
        let id = match mode {
            RouteMode::Walkaround => ids::PCB_ROUTE_MODE_WALKAROUND,
            RouteMode::Shove => ids::PCB_ROUTE_MODE_SHOVE,
        };
        crate::diagnostics::log_info(t!(self.ui_state.i18n, id).into_string());
        if self.is_routing_board_track() {
            self.refresh_pcb_highlight();
        }
    }

    /// The route so far and the leg the next click adds, drawn into
    /// `preview`.
    pub(super) fn router_preview(
        &self,
        session: &BoardToolSession,
        board: &PcbBoard,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let Some(router) = session.router.as_ref() else {
            return;
        };
        draw_into(&router.draft, preview);
        let (Some(anchor), Some(layer)) =
            (session.points.last().copied(), self.active_copper_layer())
        else {
            return;
        };
        if cursor == anchor {
            return;
        }
        let (style, via) = next_leg_style(board, session, anchor, cursor, layer);
        let leg = LegRequest {
            from: anchor,
            to: cursor,
            style,
            via,
        };
        let interaction = &self.interaction_state;
        let modes = (interaction.corner_rule, interaction.route_mode);
        if let Ok(leg) = route_on(board, router, leg, modes, Uuid::nil) {
            let mut next = RouteDraft::default();
            next.add(leg);
            draw_into(&next, preview);
        }
    }
}

/// Route `leg` with the route's rules, in the corner rule and mode given.
fn route_on(
    board: &PcbBoard,
    router: &RouterState,
    leg: LegRequest,
    (corner, mode): (CornerRule, RouteMode),
    ids: impl FnMut() -> Uuid,
) -> Result<RoutedLeg, RouteRefusal> {
    let rules = &router.rules;
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        corner,
        mode,
        grid_mm: grid_step_mm(board),
    };
    route_leg(board, &router.draft, leg, &route, ids)
}

/// A draft's tracks, vias and pushed tracks, drawn into `preview`.
fn draw_into(draft: &RouteDraft, preview: &mut PcbBoard) {
    preview.segments.extend(draft.tracks.iter().cloned());
    preview.segments.extend(draft.shoved.iter().cloned());
    preview.vias.extend(draft.vias.iter().cloned());
}

/// The width a route of `style`'s net takes: its net class's track width,
/// else the board's, held within the written rules' minimum and maximum on
/// its layer.
fn rule_width(board: &PcbBoard, classes: &[NetClass], rules: &DrcRules, style: &TrackStyle) -> f64 {
    let class_width = board
        .nets
        .iter()
        .find(|net| net.number == style.net)
        .and_then(|net| first_matching_class(classes, &net.name))
        .map(|class| class.trace_width)
        .filter(|width| width.is_finite() && *width > 0.0);
    let layers = 1 << pcb_geom::layer_bit(style.layer);
    let (min, max) = rules.track_width_limits(style.net, layers);
    let mut width = class_width.unwrap_or(style.width_mm);
    if let Some(max) = max {
        width = width.min(pcb_geom::nm_to_mm(max));
    }
    if let Some(min) = min {
        width = width.max(pcb_geom::nm_to_mm(min));
    }
    width
}

/// The Messages line for a leg that was not routed.
fn refusal_line(refusal: RouteRefusal) -> MessageId {
    match refusal {
        RouteRefusal::Blocked => ids::PCB_ROUTE_BLOCKED,
        RouteRefusal::ViaBlocked => ids::PCB_ROUTE_VIA_BLOCKED,
        RouteRefusal::Unrepresentable => ids::PCB_ROUTE_UNREPRESENTABLE,
    }
}
