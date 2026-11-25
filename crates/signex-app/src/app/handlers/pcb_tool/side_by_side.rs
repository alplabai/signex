//! Differential Pair Routing and Multi-Route (#2389): several nets routed
//! side by side on the active copper layer. The first click picks the nets
//! (the pair of the net under it; the nets of the selected copper) and
//! starts each at its copper nearest the click; each later click routes
//! the centre line around other nets (`signex_engine::pcb::route::
//! route_bus_leg`) and redraws every member at its offset. A pair keeps
//! its net class's coupled width and gap, a bus the rule width a rule
//! clearance apart; never closer than the rules allow between the nets.
//! The route lands as one undo step like Interactive Routing's.

use pcb_geom::{CopperItem, ItemRef, PointNm, Shape, layer_bit, nm_to_mm};
use signex_drc::DrcRules;
use signex_engine::pcb::draw::TrackStyle;
use signex_engine::pcb::route::{
    BusMember, BusStyle, RouteDraft, RouteRefusal, bus_draft, bus_start, bus_tracks, copper_under,
    diff_pair_of, draft_fits, nearest_net_point, pair_width_gap, route_bus_leg,
};
use signex_i18n::{ids, t};
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::{PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::BoardToolSession;
use super::router::{BusRoute, RouterState, rule_via, rule_width, walk_rules};
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

impl Signex {
    /// A Differential Pair Routing or Multi-Route click at `at` (already on
    /// the grid or an object point).
    pub(super) fn bus_route_clicked(&mut self, tool: BoardPlaceTool, at: Point) {
        let Some(layer) = self.tool_copper_layer(tool) else {
            return;
        };
        let Some(anchor) = self
            .own_board_tool()
            .map(|session| session.points.last().copied())
        else {
            return;
        };
        let Some(anchor) = anchor else {
            if let Err(why) = self.start_bus_route(tool, at, layer) {
                self.say_route_refused(tool, why);
            }
            return;
        };
        if self.clicks_run_end(at, anchor) {
            self.end_route_shape();
            return;
        }
        if let Err(why) = self.recheck_route() {
            if let Some(router) = self.own_router_mut() {
                router.refused = true;
            }
            let line = t!(self.ui_state.i18n, ids::PCB_ROUTE_STALE, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
            return;
        }
        match self.bus_leg(anchor, at) {
            Ok((centre, draft)) => {
                if let Some(session) = self.own_board_tool_mut() {
                    session.points = centre;
                    if let Some(router) = session.router.as_mut() {
                        router.draft = draft;
                        router.refused = false;
                    }
                }
            }
            Err(refusal) => self.say_route_refused(tool, refusal.to_string()),
        }
    }

    /// The centre line with the leg from `anchor` to `at` added, and every
    /// member's tracks along it, checked against the board.
    fn bus_leg(&self, anchor: Point, at: Point) -> Result<(Vec<Point>, RouteDraft), RouteRefusal> {
        let session = self.own_board_tool().ok_or(RouteRefusal::Blocked)?;
        let router = session.router.as_ref().ok_or(RouteRefusal::Blocked)?;
        let bus = router.bus.as_ref().ok_or(RouteRefusal::Blocked)?;
        let board = self
            .document_state
            .pcb_board(&session.path)
            .ok_or(RouteRefusal::BoardChanged)?;
        let rules = &router.rules;
        let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
        let corner = self.interaction_state.corner_rule;
        let route = walk_rules(board, &clearance, rules, corner);
        let leg = route_bus_leg(board, &bus.members, anchor, at, &bus.style, &route)?;
        let mut centre = session.points.clone();
        centre.extend(leg);
        let draft = RouteDraft {
            vias: bus.start_vias.clone(),
            ..bus_draft(board, &bus.members, &centre, &bus.style, Uuid::now_v7)?
        };
        draft_fits(board, &draft, &clearance, rules.widest_clearance())?;
        Ok((centre, draft))
    }

    /// The first click: the nets, their start points, width and pitch, and
    /// the board's rules. Why the route cannot start, when it cannot.
    fn start_bus_route(
        &mut self,
        tool: BoardPlaceTool,
        at: Point,
        layer: CopperLayer,
    ) -> Result<(), String> {
        let Some((path, board)) = self.active_tab_path().zip(self.active_pcb()) else {
            return Ok(());
        };
        let Some(revision) = self
            .document_state
            .board_engine(&path)
            .map(|engine| engine.revision())
        else {
            return Ok(());
        };
        let (rules, classes) = self.board_design_rules(&path, board)?;
        let nets = match tool {
            BoardPlaceTool::DiffPairRouting => self.pair_nets_at(board, &classes, at, layer)?,
            _ => self.selected_copper_nets(),
        };
        let i18n = &self.ui_state.i18n;
        if nets.len() < 2 {
            return Err(t!(i18n, ids::PCB_ROUTE_BUS_NEEDS_NETS).into_string());
        }
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let mut members = Vec::with_capacity(nets.len());
        let mut start_vias = Vec::new();
        for &net in &nets {
            // A net with no copper on the active layer starts from its
            // copper on another layer with a via (maintainer 2026-11-18).
            let (anchor, off_layer) = match nearest_net_point(board, net, layer, at) {
                Some(anchor) => (Some(anchor), false),
                None => (nearest_on_any_layer(board, net, at), true),
            };
            let anchor = anchor.ok_or_else(|| {
                let name = net_name(board, net);
                t!(i18n, ids::PCB_ROUTE_MEMBER_NO_COPPER, "net" => name).into_string()
            })?;
            if off_layer {
                let via = Via {
                    attributes: Default::default(),
                    net,
                    ..defaults.via(board, Uuid::now_v7(), anchor)
                };
                start_vias.push(rule_via(via, &rules));
            }
            members.push(BusMember { net, anchor });
        }
        let Some(start) = bus_start(&members) else {
            return Ok(());
        };
        let mut track = defaults.track_style(board, at, layer);
        let ask = BusAsk {
            tool,
            nets: &nets,
            layer,
            width_mm: track.width_mm,
        };
        let style = bus_style(board, &classes, &rules, &ask);
        track.net = nets[0];
        track.width_mm = style.width_mm;
        if let Some(session) = self.own_board_tool_mut() {
            session.points = vec![start];
            session.route = Some(track);
            session.router = Some(RouterState {
                draft: RouteDraft::default(),
                rules,
                revision,
                refused: false,
                preview: None,
                bus: Some(BusRoute {
                    members,
                    style,
                    start_vias,
                }),
                joined: Vec::new(),
                joined_lines: Vec::new(),
            });
        }
        Ok(())
    }

    /// The pair of the net under `at` on `layer`, positive first.
    fn pair_nets_at(
        &self,
        board: &PcbBoard,
        classes: &[NetClass],
        at: Point,
        layer: CopperLayer,
    ) -> Result<Vec<u32>, String> {
        let i18n = &self.ui_state.i18n;
        let mut net = signex_engine::pcb::draw::net_under(board, at, 1 << layer_bit(layer));
        if net == 0 {
            // Copper off the active layer: the pair starts there with vias.
            net = copper_under(board, at).map_or(0, |(_, net)| net);
        }
        if net == 0 {
            return Err(t!(i18n, ids::PCB_ROUTE_NO_NET).into_string());
        }
        let pair = diff_pair_of(board, classes, net).ok_or_else(|| {
            let name = net_name(board, net);
            t!(i18n, ids::PCB_ROUTE_NO_PAIR, "net" => name).into_string()
        })?;
        Ok(pair.nets().to_vec())
    }

    /// The nets of the copper selected on the board in view, each once.
    fn selected_copper_nets(&self) -> Vec<u32> {
        let Some(board) = self.active_pcb() else {
            return Vec::new();
        };
        let selection = &self.interaction_state.pcb_selection;
        let path = self.active_tab_path();
        if selection.document() != path.as_deref() {
            return Vec::new();
        }
        let mut nets: Vec<u32> = Vec::new();
        for item in selection.selection().items() {
            let net = match item {
                ItemRef::Track(_) | ItemRef::Via(_) | ItemRef::Pad { .. } => {
                    pcb_geom::item_net(board, item)
                }
                _ => None,
            };
            if let Some(net) = net.filter(|&n| n != 0 && !nets.contains(&n)) {
                nets.push(net);
            }
        }
        nets
    }

    /// Say in Messages why `tool` did nothing.
    fn say_route_refused(&self, tool: BoardPlaceTool, why: String) {
        let i18n = &self.ui_state.i18n;
        let name = t!(i18n, tool.name()).into_string();
        let line = t!(i18n, ids::PCB_ROUTE_REFUSED, "tool" => name, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }

    /// Every member's tracks along the centre line with a straight leg to
    /// the pointer added, drawn into `preview` unchecked: where the next
    /// click routes.
    pub(super) fn bus_route_preview(&self, session: &BoardToolSession, preview: &mut PcbBoard) {
        let Some(bus) = session.router.as_ref().and_then(|r| r.bus.as_ref()) else {
            return;
        };
        let mut centre = session.points.clone();
        let next = session.cursor.filter(|c| centre.last() != Some(c));
        centre.extend(next);
        // Trimmed against the board, as the click lays them (#2492).
        if let Some(board) = self.document_state.pcb_board(&session.path)
            && let Ok(tracks) = bus_tracks(board, &bus.members, &centre, &bus.style, Uuid::nil)
        {
            preview.segments.extend(tracks);
        }
        preview.vias.extend(bus.start_vias.iter().cloned());
    }
}

/// `net`'s copper nearest `at` on the topmost copper layer that has any.
fn nearest_on_any_layer(board: &PcbBoard, net: u32, at: Point) -> Option<Point> {
    CopperLayer::all().find_map(|layer| nearest_net_point(board, net, layer, at))
}

/// The name of `net` on `board`, else its number.
fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map_or_else(|| net.to_string(), |n| n.name.clone())
}

/// What a pair or bus is routed with besides the board and its rules.
struct BusAsk<'a> {
    tool: BoardPlaceTool,
    nets: &'a [u32],
    layer: CopperLayer,
    /// The width the tool's form or the board gives a track.
    width_mm: f64,
}

/// The width and pitch of the tracks of `ask.nets` on its layer: the first
/// net's route width (its class, within the rules), and for a pair its
/// class's coupled width and gap. The gap is never under the clearance the
/// rules ask between any two of the nets.
fn bus_style(board: &PcbBoard, classes: &[NetClass], rules: &DrcRules, ask: &BusAsk) -> BusStyle {
    let BusAsk {
        tool,
        nets,
        layer,
        width_mm,
    } = *ask;
    let style = TrackStyle {
        layer,
        width_mm,
        net: nets[0],
    };
    let width = rule_width(board, classes, rules, &style);
    let layers = 1 << layer_bit(layer);
    let probe = |net| CopperItem {
        item: ItemRef::Track(Uuid::nil()),
        net,
        layers,
        shape: Shape::Capsule {
            a: PointNm::ORIGIN,
            b: PointNm::ORIGIN,
            radius: 0,
        },
        pad_center: None,
    };
    let least_gap = nets
        .iter()
        .flat_map(|&a| nets.iter().map(move |&b| (a, b)))
        .filter(|(a, b)| a != b)
        .map(|(a, b)| rules.clearance_between(&probe(a), &probe(b)))
        .max()
        .map_or(0.0, nm_to_mm);
    let (width, gap) = match tool {
        BoardPlaceTool::DiffPairRouting => {
            let class = first_matching_class(classes, &net_name(board, nets[0]));
            pair_width_gap(class, width, least_gap, least_gap)
        }
        _ => (width, least_gap),
    };
    BusStyle {
        layer,
        width_mm: width,
        pitch_mm: width + gap,
    }
}
