//! The Track tool (#2373): a run drawn on the active copper layer, click by
//! click. The first click starts the run on the net of the copper under it;
//! each later click adds the legs from the run's end to the point, bent by
//! the corner rule of wires (#2372: Space flips the bend, Shift+Space steps
//! the corner mode), as one undo step. Making another copper layer active
//! mid-run puts a via at the run's end in the same step, and the run goes
//! on there. A click on the run's end, Esc or a right-click ends the run.
//! A run started on copper that does not reach the active layer (a surface
//! pad on Top with an inner layer active) takes that copper's net and starts
//! with a via (`start_on_copper`).

use signex_engine::pcb::draw::{self, TrackStyle};
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::route_end::{end_via, track_end_refusal};
use super::{BoardToolDefaults, BoardToolSession};
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

impl Signex {
    /// Whether a track run is being drawn on the board this window shows:
    /// Space and Shift+Space bend it.
    pub(crate) fn is_routing_board_track(&self) -> bool {
        let path = self.active_tab_path();
        self.is_routing_on(path.as_deref(), self.interaction_state.acting_window)
    }

    /// [`Self::is_routing_board_track`] for `window` (`None`: the main
    /// window) before that window's board is swapped in, as a key from it
    /// is judged: an undocked window's own tab, else the main window's.
    pub(crate) fn is_routing_board_track_in(&self, window: Option<iced::window::Id>) -> bool {
        let (undocked, path) = self.key_window_tab(window);
        self.is_routing_on(path.as_deref(), undocked)
    }

    /// Whether `window` (`None`: the main window) shows a board, judged
    /// before its board is swapped in. There the rotate handler takes the
    /// board path and never reaches a wire run (#2553).
    pub(crate) fn window_shows_pcb(&self, window: Option<iced::window::Id>) -> bool {
        let (_, path) = self.key_window_tab(window);
        path.is_some_and(|path| {
            (self.document_state.tabs.iter())
                .find(|tab| tab.path == path)
                .and_then(|tab| tab.cached_document.as_ref())
                .is_some_and(|doc| doc.as_pcb().is_some())
        })
    }

    /// The undocked window a key came from (`None`: the main window) and the
    /// tab it shows: an undocked window's own tab, else the main window's.
    fn key_window_tab(
        &self,
        window: Option<iced::window::Id>,
    ) -> (Option<iced::window::Id>, Option<std::path::PathBuf>) {
        let undocked = window.filter(|id| self.ui_state.main_window_id != Some(*id));
        let path = match undocked {
            Some(id) => match self.ui_state.windows.get(&id) {
                Some(crate::app::state::WindowKind::UndockedTab { path, .. }) => Some(path.clone()),
                _ => None,
            },
            None => self.active_tab_path(),
        };
        (undocked, path)
    }

    /// The one rule both read: the armed tool is for the board at `path`
    /// shown in `window`, and a run is open on it.
    fn is_routing_on(
        &self,
        path: Option<&std::path::Path>,
        window: Option<iced::window::Id>,
    ) -> bool {
        (self.interaction_state.pcb_tool.as_ref())
            .is_some_and(|session| session.is_for(path, window) && session.is_routing())
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
        let classes = self.track_tool_net_classes();
        let Some(anchor) = session.points.last().copied() else {
            let defaults = &self.interaction_state.pcb_tool_defaults;
            let style = defaults.track_style(board, at, layer);
            let style = start_on_copper(board, defaults, at, style)
                .with_class_width(board, form_or_class_widths(defaults, classes));
            self.set_route(vec![at], Some(style));
            return;
        };
        if self.clicks_run_end(at, anchor) {
            // A second click on the run's end finishes the run.
            self.set_route(Vec::new(), None);
            return;
        }
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let (mut style, via) = next_leg_style(board, session, defaults, anchor, at, layer, classes);
        // A pad off the run's layer at its end is reached with a via there.
        let end = end_via(board, defaults, at, &mut style);
        // A pad of another net refuses the leg, as Interactive Routing's.
        let refused = track_end_refusal(
            &self.ui_state.i18n,
            board,
            at,
            &style,
            via.as_ref(),
            end.as_ref(),
        );
        if let Some(line) = refused {
            self.note_board_tool(line);
            return;
        }
        let rule = self.interaction_state.corner_rule;
        let tracks = draw::track_run(anchor, at, rule, style, Uuid::now_v7);
        if let Some(command) = draw::place_route(tracks, via) {
            let mut commands = vec![command];
            commands.extend(draw::place_route(Vec::new(), end));
            self.run_placement("Track", Ok(commands));
        }
        let mut points = self
            .own_board_tool()
            .map(|s| s.points.clone())
            .unwrap_or_default();
        points.push(at);
        self.set_route(points, Some(style));
    }

    /// The net classes the Track tool sizes a run by: the table of the
    /// project of the board in view (none for a loose board).
    pub(super) fn track_tool_net_classes(&self) -> &[NetClass] {
        match self.active_tab_path() {
            Some(path) => self.board_net_classes(&path),
            None => &[],
        }
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
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let classes = self.track_tool_net_classes();
        let (mut style, via) =
            next_leg_style(board, session, defaults, anchor, cursor, layer, classes);
        let end = end_via(board, defaults, cursor, &mut style);
        // The leg a click would refuse is not previewed, nor its via.
        let refused = track_end_refusal(
            &self.ui_state.i18n,
            board,
            cursor,
            &style,
            via.as_ref(),
            end.as_ref(),
        );
        if refused.is_some() {
            return;
        }
        let rule = self.interaction_state.corner_rule;
        preview
            .segments
            .extend(draw::track_run(anchor, cursor, rule, style, Uuid::nil));
        preview.vias.extend(via);
        preview.vias.extend(end);
    }
}

/// The style of the legs from `anchor` to `to` on `layer`, and the via at
/// `anchor` when `layer` is not the run's. A run with no net yet takes the
/// net of the copper under `to`, unless the tool's net was set (#2393), and
/// the track width of its net class (#2400), unless the tool's width was set.
pub(super) fn next_leg_style(
    board: &PcbBoard,
    session: &BoardToolSession,
    defaults: &BoardToolDefaults,
    anchor: Point,
    to: Point,
    layer: CopperLayer,
    classes: &[NetClass],
) -> (TrackStyle, Option<Via>) {
    let classes = form_or_class_widths(defaults, classes);
    let mut style = session.route.unwrap_or_else(|| {
        defaults
            .track_style(board, anchor, layer)
            .with_class_width(board, classes)
    });
    if style.net == 0 && defaults.track_net.is_none() {
        style.net = draw::net_under(board, to, 1 << pcb_geom::layer_bit(layer));
        style = style.with_class_width(board, classes);
    }
    let via = (style.layer != layer).then(|| Via {
        attributes: Default::default(),
        net: style.net,
        ..defaults.via(board, Uuid::now_v7(), anchor)
    });
    style.layer = layer;
    (style, via)
}

/// `style`, started at `at` (`signex_engine::pcb::route::start_on_copper`);
/// a net set in Properties keeps it on the active layer.
pub(super) fn start_on_copper(
    board: &PcbBoard,
    defaults: &BoardToolDefaults,
    at: Point,
    style: TrackStyle,
) -> TrackStyle {
    let net_set = defaults.track_net.is_some();
    signex_engine::pcb::route::start_on_copper(board, at, style, net_set)
}

/// The net classes that size a track: none once the tool's form set a width
/// (#2393), which then replaces the net class's (#2400).
fn form_or_class_widths<'a>(
    defaults: &BoardToolDefaults,
    classes: &'a [NetClass],
) -> &'a [NetClass] {
    if defaults.track_width_mm.is_some() {
        &[]
    } else {
        classes
    }
}
