//! Interactive Routing (#2389) on the board in view. The first click starts
//! a route on the active copper layer, on the net of the copper under it,
//! with the width its net class (else the board) gives, held within the
//! written rules' limits. Each later click routes a leg to the point with
//! `signex_engine::pcb::route`: around other nets' copper at the rule
//! clearance, or pushing their tracks aside (`cycle_route_mode`), bent by
//! the corner rule of #2372; a layer change adds a via no smaller than the
//! rules allow. The legs stay off the board until the route ends (a click
//! on its end, Esc, a right-click, another tool), then land as one undo
//! step, checked against the board revision they were routed on. How a
//! board that changed under the route is handled: `docs/PCB_TOOLS.md`,
//! "Interactive Routing".

use pcb_geom::CopperItem;
use signex_drc::DrcRules;
use signex_engine::pcb::draw::{self, TrackStyle};
use signex_engine::pcb::placement::grid_step_mm;
use signex_engine::pcb::route::{
    BusMember, BusStyle, LegRequest, PREVIEW_BUDGET, RouteDraft, RouteMode, RouteRefusal,
    RouteRules, RoutedLeg, SEARCH_BUDGET, draft_fits, join_pads, route_leg,
};
use signex_i18n::{MessageId, ids, t};
use signex_types::corner::CornerRule;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::{PCB_GRID_MM, PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::BoardToolSession;
use super::route::{next_leg_style, start_on_copper};
use super::route_end::{joined_line, leg_end, router_line, with_end_via};
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::pcb_edit::CheckedStep;

/// The route being drawn: its legs not yet on the board, the rules they
/// keep (read once when the route starts) and the board revision they were
/// last checked against.
#[derive(Debug, Clone, PartialEq)]
pub struct RouterState {
    pub draft: RouteDraft,
    pub rules: DrcRules,
    pub revision: u64,
    /// Landing the draft was refused once: the next Esc discards it.
    pub refused: bool,
    /// The leg the next click adds, routed for the pointer.
    pub preview: Option<RouterPreview>,
    /// The nets a Differential Pair or Multi-Route route runs side by side
    /// (`side_by_side.rs`); `None` for a one-net route.
    pub bus: Option<BusRoute>,
    /// Pads of no net the route ends on, `(footprint, pad, net)`: each
    /// joins the route's net when it lands (maintainer 2026-11-18).
    pub joined: Vec<(Uuid, Uuid, u32)>,
    /// What each pad of `joined` says once the route lands: the status bar
    /// says it at the click, Messages when the net is given.
    pub joined_lines: Vec<String>,
}

/// The nets of a pair or bus route and the tracks they run in. The centre
/// line is the session's points; the draft holds every member's tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct BusRoute {
    pub members: Vec<BusMember>,
    pub style: BusStyle,
    /// The vias that take members starting off the active layer to it.
    pub start_vias: Vec<Via>,
}

/// The leg on the pointer and what it was routed from: routed again only
/// when one of those changes, not on every repaint.
#[derive(Debug, Clone, PartialEq)]
pub struct RouterPreview {
    pub key: PreviewKey,
    pub leg: Option<RoutedLeg>,
    /// Why no leg was routed to the pointer, for the status bar; the
    /// straight legs it was aimed along are drawn in `aimed` instead of
    /// nothing.
    pub refused: Option<String>,
    pub aimed: Vec<Segment>,
    /// The via at the leg's end that reaches a pad off its layer.
    pub end_via: Option<Via>,
}

/// What the leg on the pointer depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewKey {
    pub anchor: Point,
    pub cursor: Point,
    /// Points placed so far: a new leg changes the draft.
    pub placed: usize,
    pub revision: u64,
    pub corner: CornerRule,
    pub mode: RouteMode,
    pub layer: CopperLayer,
}

/// How ending a route went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RouteEnd {
    /// It landed, or there was nothing to land.
    Landed,
    /// It no longer fits the board and is kept for another try.
    Kept,
    /// It no longer fits the board and was dropped, said in Messages.
    Discarded,
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
        if self.clicks_run_end(at, anchor) {
            // A second click on the route's end finishes it.
            self.end_route_shape();
            return;
        }
        if let Err(why) = self.recheck_route() {
            if let Some(router) = self.own_router_mut() {
                router.refused = true;
            }
            let line = t!(self.ui_state.i18n, ids::PCB_ROUTE_STALE, "why" => why);
            self.note_board_tool(line.into_string());
            return;
        }
        let Some((board, session)) = self.active_pcb().zip(self.own_board_tool()) else {
            return;
        };
        let Some(router) = session.router.as_ref() else {
            return;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let classes = self.board_net_classes(&session.path);
        let (style, via) = next_leg_style(board, session, defaults, anchor, at, layer, classes);
        let i18n = &self.ui_state.i18n;
        let end = match leg_end(
            i18n,
            board,
            defaults,
            &router.rules,
            at,
            via.as_ref(),
            style,
        ) {
            Ok(end) => end,
            Err(line) => {
                self.note_board_tool(line);
                return;
            }
        };
        let style = end.style;
        let leg = LegRequest {
            from: anchor,
            to: at,
            style,
            via: via.map(|via| rule_via(via, &router.rules)),
        };
        let interaction = &self.interaction_state;
        let corner = interaction.corner_rule;
        let modes = (corner, interaction.route_mode, SEARCH_BUDGET);
        let routed = route_on(board, router, leg, modes, Uuid::now_v7)
            .and_then(|leg| with_end_via(board, router, leg, end.via, corner));
        let joined = end.joins.map(|pad| {
            let line = joined_line(i18n, board, &pad, style.net);
            ((pad.footprint, pad.pad, style.net), line)
        });
        let legs = match routed {
            Ok(legs) => legs,
            Err(refusal) => {
                let line = t!(self.ui_state.i18n, refusal_line(refusal));
                self.note_board_tool(line.into_string());
                return;
            }
        };
        if let Some(session) = self.own_board_tool_mut() {
            session.points.push(at);
            session.route = Some(style);
            if let Some(router) = session.router.as_mut() {
                for leg in legs {
                    router.draft.add(leg);
                }
                router.refused = false;
                router.joined.extend(joined.as_ref().map(|(join, _)| *join));
                router
                    .joined_lines
                    .extend(joined.as_ref().map(|(_, line)| line.clone()));
            }
            // The status bar says which pad took the route's net.
            session.note = joined.map(|(_, line)| line);
        }
    }

    /// The first click: read the board's rules and start the route at `at`.
    pub(super) fn start_route(&mut self, at: Point, layer: CopperLayer) {
        let Some((path, board)) = self.active_tab_path().zip(self.active_pcb()) else {
            return;
        };
        let Some(revision) = self
            .document_state
            .board_engine(&path)
            .map(|engine| engine.revision())
        else {
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
        // The Properties form's width and net come first (#2393); a width
        // set there replaces the net class's, and the rules still bound it.
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let style = defaults.track_style(board, at, layer);
        let mut style = start_on_copper(board, defaults, at, style);
        let classes: &[NetClass] = if defaults.track_width_mm.is_some() {
            &[]
        } else {
            &classes
        };
        style.width_mm = rule_width(board, classes, &rules, &style);
        if let Some(session) = self.own_board_tool_mut() {
            session.points = vec![at];
            session.route = Some(style);
            session.router = Some(RouterState {
                draft: RouteDraft::default(),
                rules,
                revision,
                refused: false,
                preview: None,
                bus: None,
                joined: Vec::new(),
                joined_lines: Vec::new(),
            });
        }
    }

    /// End the route on the board in view (its end clicked, Esc or a
    /// right-click): the tool stays armed for a new route, unless the route
    /// no longer fits and is kept for another try.
    pub(super) fn end_route_shape(&mut self) {
        if self.finish_board_route(false) != RouteEnd::Kept {
            self.drop_board_shape();
        }
    }

    /// Land the route being drawn, on whichever board it is drawn, as one
    /// undo step. Nothing when no route is drawn or it has no leg yet. When
    /// the board changed under it and it no longer fits, nothing lands: the
    /// route is kept for one more try, or dropped when `last_chance` (the
    /// tool is ending) or a try was refused already; Messages says which.
    pub(super) fn finish_board_route(&mut self, last_chance: bool) -> RouteEnd {
        let Some(router) = self.drawn_router() else {
            return RouteEnd::Landed;
        };
        if router.draft.is_empty() {
            return RouteEnd::Landed;
        }
        let refused_before = router.refused;
        let why = match self.land_route() {
            Ok(()) => {
                if let Some(router) = self.drawn_router_mut() {
                    router.draft = RouteDraft::default();
                    router.joined.clear();
                    // A pad that took the route's net is said in Messages
                    // too: the net is now the pad's, not the status bar's.
                    for line in std::mem::take(&mut router.joined_lines) {
                        crate::diagnostics::log_info(line);
                    }
                }
                return RouteEnd::Landed;
            }
            Err(why) => why,
        };
        let i18n = &self.ui_state.i18n;
        if last_chance || refused_before {
            let line = t!(i18n, ids::PCB_ROUTE_DISCARDED, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
            if let Some(router) = self.drawn_router_mut() {
                router.draft = RouteDraft::default();
                router.joined.clear();
                router.joined_lines.clear();
            }
            return RouteEnd::Discarded;
        }
        let line = t!(i18n, ids::PCB_ROUTE_NOT_PLACED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
        if let Some(router) = self.drawn_router_mut() {
            router.refused = true;
        }
        RouteEnd::Kept
    }

    /// Run the draft on its board as one step checked against the revision
    /// it fits; why not, when it does not.
    fn land_route(&mut self) -> Result<(), String> {
        self.recheck_route()?;
        let Some(session) = self.interaction_state.pcb_tool.as_ref() else {
            return Ok(());
        };
        let Some(router) = session.router.as_ref() else {
            return Ok(());
        };
        let path = session.path.clone();
        let mut commands = router.draft.commands();
        if !router.joined.is_empty() {
            // The pads of no net it ends on join its net in the same step.
            let board = self.document_state.pcb_board(&path);
            let Some(join) = board.and_then(|board| join_pads(board, &router.joined)) else {
                return Err(t!(self.ui_state.i18n, ids::PCB_ROUTE_JOIN_GONE).into_string());
            };
            commands.push(join);
        }
        let expected_revision = router.revision;
        let label = t!(self.ui_state.i18n, session.tool.name()).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision,
        };
        self.execute_board_commands(&path, commands, Some(step))
            .map_err(|error| error.to_string())?;
        self.refresh_pcb_highlight();
        Ok(())
    }

    /// Check the draft against its board when the board moved past the
    /// revision it was last checked on (an undo, an edit from another
    /// window or panel); a draft that still fits is then held against the
    /// new revision. Why it does not fit, when it does not.
    pub(super) fn recheck_route(&mut self) -> Result<(), String> {
        let Some(session) = self.interaction_state.pcb_tool.as_ref() else {
            return Ok(());
        };
        let Some(router) = session.router.as_ref() else {
            return Ok(());
        };
        let Some(engine) = self.document_state.board_engine(&session.path) else {
            let gone = crate::app::BoardEditError::NotLoaded(session.path.clone());
            return Err(gone.to_string());
        };
        let live = engine.revision();
        if live == router.revision {
            return Ok(());
        }
        let rules = &router.rules;
        let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
        draft_fits(
            engine.board(),
            &router.draft,
            &clearance,
            rules.widest_clearance(),
        )
        .map_err(|refusal| refusal.to_string())?;
        if let Some(router) = self.drawn_router_mut() {
            router.revision = live;
        }
        Ok(())
    }

    /// The route drawn with the tool armed, on whichever board.
    fn drawn_router(&self) -> Option<&RouterState> {
        self.interaction_state.pcb_tool.as_ref()?.router.as_ref()
    }

    fn drawn_router_mut(&mut self) -> Option<&mut RouterState> {
        self.interaction_state.pcb_tool.as_mut()?.router.as_mut()
    }

    pub(super) fn own_router_mut(&mut self) -> Option<&mut RouterState> {
        self.own_board_tool_mut()?.router.as_mut()
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

    /// Route the leg on the pointer again when what it depends on changed
    /// (the pointer's grid point, a leg, the board, the corner rule, the
    /// mode or the layer), with the preview's smaller search budget.
    pub(crate) fn update_router_preview(&mut self) {
        let Some(key) = self.router_preview_key() else {
            return;
        };
        let held = self
            .own_board_tool()
            .and_then(|session| session.router.as_ref())
            .and_then(|router| router.preview.as_ref())
            .is_some_and(|preview| preview.key == key);
        if held {
            return;
        }
        let preview = self.preview_leg(key);
        if let Some(router) = self.own_router_mut() {
            router.preview = Some(preview);
        }
    }

    /// The key of the leg on the pointer; `None` for the other routing
    /// tools, which draw theirs straight (`side_by_side.rs`, `quick.rs`).
    fn router_preview_key(&self) -> Option<PreviewKey> {
        let session = self
            .own_board_tool()
            .filter(|s| s.router.is_some() && s.tool == BoardPlaceTool::InteractiveRouting)?;
        let anchor = *session.points.last()?;
        let cursor = session.cursor?;
        let revision = self.document_state.board_engine(&session.path)?.revision();
        let interaction = &self.interaction_state;
        Some(PreviewKey {
            anchor,
            cursor,
            placed: session.points.len(),
            revision,
            corner: interaction.corner_rule,
            mode: interaction.route_mode,
            layer: self.active_copper_layer()?,
        })
    }

    /// The leg on the pointer; when there is none, why, and the straight
    /// legs it was aimed along, so the preview never just vanishes.
    fn preview_leg(&self, key: PreviewKey) -> RouterPreview {
        let mut out = RouterPreview {
            key,
            leg: None,
            refused: None,
            aimed: Vec::new(),
            end_via: None,
        };
        if key.cursor == key.anchor {
            return out;
        }
        let Some(session) = self.own_board_tool() else {
            return out;
        };
        let (Some(router), Some(board)) = (
            session.router.as_ref(),
            self.document_state.pcb_board(&session.path),
        ) else {
            return out;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let classes = self.board_net_classes(&session.path);
        let (style, via) = next_leg_style(
            board, session, defaults, key.anchor, key.cursor, key.layer, classes,
        );
        let i18n = &self.ui_state.i18n;
        let why = match leg_end(
            i18n,
            board,
            defaults,
            &router.rules,
            key.cursor,
            via.as_ref(),
            style,
        ) {
            Err(line) => line,
            Ok(end) => {
                let leg = LegRequest {
                    from: key.anchor,
                    to: key.cursor,
                    style: end.style,
                    via: via.map(|via| rule_via(via, &router.rules)),
                };
                let modes = (key.corner, key.mode, PREVIEW_BUDGET);
                match route_on(board, router, leg, modes, Uuid::nil) {
                    Ok(leg) => {
                        out.leg = Some(leg);
                        out.end_via = end.via;
                        return out;
                    }
                    Err(RouteRefusal::Blocked) => {
                        let why = t!(i18n, ids::PCB_ROUTE_AIM_BLOCKED).into_string();
                        router_line(i18n, why)
                    }
                    Err(refusal) => t!(i18n, refusal_line(refusal)).into_string(),
                }
            }
        };
        out.aimed = draw::track_run(key.anchor, key.cursor, key.corner, style, Uuid::nil);
        out.refused = Some(why);
        out
    }

    /// The route so far and the leg the next click adds (routed by
    /// [`Self::update_router_preview`]), drawn into `preview`.
    pub(super) fn router_preview(&self, session: &BoardToolSession, preview: &mut PcbBoard) {
        let Some(router) = session.router.as_ref() else {
            return;
        };
        match session.tool {
            BoardPlaceTool::QuickRoute => return self.quick_route_preview(session, preview),
            BoardPlaceTool::DiffPairRouting | BoardPlaceTool::MultiRoute => {
                return self.bus_route_preview(session, preview);
            }
            _ => {}
        }
        draw_into(&router.draft, preview);
        let key = self.router_preview_key();
        let Some(held) = router.preview.as_ref() else {
            return;
        };
        if Some(held.key) != key {
            return;
        }
        // A leg with no way is drawn by the highlight layer, in the DRC
        // error colour (`router_blocked_shapes`).
        if let Some(leg) = held.leg.clone() {
            let mut next = RouteDraft::default();
            next.add(leg);
            draw_into(&next, preview);
            preview.vias.extend(held.end_via.iter().cloned());
        }
    }

    /// The held preview of the leg on the pointer, while it is current.
    fn current_router_preview(&self) -> Option<&RouterPreview> {
        let key = self.router_preview_key()?;
        let router = self.own_board_tool()?.router.as_ref()?;
        router.preview.as_ref().filter(|held| held.key == key)
    }

    /// Why the leg on the pointer has no way, while it has none: the status
    /// bar says it.
    pub(super) fn router_preview_refusal(&self) -> Option<&str> {
        self.current_router_preview()?.refused.as_deref()
    }

    /// The leg on the pointer that has no way, as it was aimed: drawn in the
    /// DRC error colour.
    pub(crate) fn router_blocked_shapes(&self) -> Vec<pcb_geom::Shape> {
        let Some(held) = self.current_router_preview() else {
            return Vec::new();
        };
        held.aimed
            .iter()
            .filter_map(|track| pcb_geom::track_shape(track).ok())
            .collect()
    }
}

/// Route `leg` with the route's rules, in the corner rule and mode given,
/// expanding at most `budget` lattice points.
pub(super) fn route_on(
    board: &PcbBoard,
    router: &RouterState,
    leg: LegRequest,
    (corner, mode, budget): (CornerRule, RouteMode, usize),
    ids: impl FnMut() -> Uuid,
) -> Result<RoutedLeg, RouteRefusal> {
    let rules = &router.rules;
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = RouteRules {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        corner,
        mode,
        grid_mm: search_grid_mm(board),
        budget,
    };
    route_leg(board, &router.draft, leg, &route, ids)
}

/// The step a route's walkaround searches on: the board's grid, but never
/// coarser than the default, so a coarse view grid cannot close a gap the
/// route fits through. The route's ends stay where snap put them.
pub(super) fn search_grid_mm(board: &PcbBoard) -> f64 {
    grid_step_mm(board).min(PCB_GRID_MM)
}

/// The rules a pair or bus leg is routed by on `board`: `rules`'s
/// clearances, the corner rule given, walking around other nets.
pub(super) fn walk_rules<'a>(
    board: &PcbBoard,
    clearance: &'a pcb_geom::route::ClearanceRule<'a>,
    rules: &DrcRules,
    corner: CornerRule,
) -> RouteRules<'a> {
    RouteRules {
        clearance,
        reach: rules.widest_clearance(),
        corner,
        mode: RouteMode::Walkaround,
        grid_mm: search_grid_mm(board),
        budget: SEARCH_BUDGET,
    }
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
pub(super) fn rule_width(
    board: &PcbBoard,
    classes: &[NetClass],
    rules: &DrcRules,
    style: &TrackStyle,
) -> f64 {
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

/// `via` grown to the smallest copper diameter and drill the written rules
/// allow for its net, its copper never narrower than its drill.
pub(super) fn rule_via(mut via: Via, rules: &DrcRules) -> Via {
    let (diameter, drill) = rules.via_size_limits(via.net, pcb_geom::via_layers(&via));
    if let Some(min) = diameter {
        via.diameter = via.diameter.max(pcb_geom::nm_to_mm(min));
    }
    if let Some(min) = drill {
        via.drill = via.drill.max(pcb_geom::nm_to_mm(min));
    }
    via.diameter = via.diameter.max(via.drill);
    via
}

/// The Messages line for a leg that was not routed.
pub(super) fn refusal_line(refusal: RouteRefusal) -> MessageId {
    match refusal {
        RouteRefusal::Blocked => ids::PCB_ROUTE_BLOCKED,
        RouteRefusal::ViaBlocked => ids::PCB_ROUTE_VIA_BLOCKED,
        RouteRefusal::Unrepresentable => ids::PCB_ROUTE_UNREPRESENTABLE,
        RouteRefusal::BoardChanged => ids::PCB_ROUTE_BOARD_CHANGED,
        RouteRefusal::TooSharp => ids::PCB_ROUTE_TOO_SHARP,
    }
}
