//! The values a board placement tool places with (#2393), shown in
//! Properties while the tool is armed. A value nobody set follows the board:
//! its via size, track width and clearance (`PcbSetup`), the net of the
//! copper under the point, the active layer. Values set in the panel stay the tool's for
//! the session.
//!
//! Tab (`placement_properties`) pauses placing and focuses the panel; Enter
//! in a field or Tab again goes on placing with the values; Esc puts back
//! the values the pause began with.

use iced::Task;
use iced::widget::operation;
use signex_engine::pcb::draw::{self, ArcMode, TrackStyle};
use signex_engine::pcb::primitive_edit::{
    PrimitiveEditRefusal, ZoneEdit, apply_zone_edit, zone_layer_allowed,
};
use signex_i18n::{MessageId, ids, t};
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{PadShape, PcbBoard, Point, Segment, Via, Zone};
use signex_types::pcb_dimension_style::DimensionStyle;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::zone_keepout::KEEPOUT_FILL;
use uuid::Uuid;

use crate::app::handlers::pcb_primitive::{PrimitiveEdit, zone_edit};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbViewMsg, Signex};
use crate::ignore::IgnoreResult;
use crate::panels::pcb_primitive_properties::{
    PLACEMENT_INPUT_ID, PrimitiveField as F, PrimitiveTarget,
};

/// The values the placement tools place with; `None` follows the board (or
/// the tool's own default).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardToolDefaults {
    pub via_diameter_mm: Option<f64>,
    pub via_hole_mm: Option<f64>,
    /// 0 is no net.
    pub via_net: Option<u32>,
    pub track_width_mm: Option<f64>,
    /// 0 is no net.
    pub track_net: Option<u32>,
    /// The net of a Fill, Solid Region or Polygon Pour; 0 is no net.
    pub area_net: Option<u32>,
    /// A pour's clearance; 0 follows the board's.
    pub pour_clearance_mm: Option<f64>,
    pub pour_min_thickness_mm: Option<f64>,
    /// How the Arc tool's clicks make an arc (#2373).
    pub arc_mode: ArcMode,
    pub arc_width_mm: Option<f64>,
    /// A copper arc's net; 0 is no net.
    pub arc_net: Option<u32>,
    /// A free pad's designator, shape, size, hole, turn and net (#2373).
    pub pad_designator: Option<String>,
    pub pad_shape: Option<PadShape>,
    pub pad_size_x_mm: Option<f64>,
    pub pad_size_y_mm: Option<f64>,
    pub pad_hole_mm: Option<f64>,
    pub pad_rotation: Option<f64>,
    /// 0 is no net.
    pub pad_net: Option<u32>,
    /// A string's text, height and turn (#2373).
    pub text: Option<String>,
    pub text_height_mm: Option<f64>,
    pub text_rotation: Option<f64>,
    /// A dimension's line width, text height and decimals (#2373).
    pub dimension_line_width_mm: Option<f64>,
    pub dimension_text_height_mm: Option<f64>,
    pub dimension_precision: Option<u8>,
    /// A dimension's arrows, text, extension lines and number format, and
    /// its Manual text point (#2868); `None` follows Preferences.
    pub dimension_style: Option<DimensionStyle>,
    pub dimension_text_at: Option<Point>,
    /// A pour's priority and island rules (#2216).
    pub pour_priority: Option<f64>,
    pub pour_min_island_mm2: Option<f64>,
    pub pour_remove_dead_islands: Option<bool>,
    /// The line width of a drawing the Graphics tools place (#2424).
    pub graphic_width_mm: Option<f64>,
    /// A text frame's text; `None` reads the catalog's new-frame text.
    pub frame_text: Option<String>,
    pub frame_text_height_mm: Option<f64>,
}

impl BoardToolDefaults {
    /// `zone`, just made by an area tool, with the values set for the area
    /// tools. A value an area does not have (a keepout's net) leaves it.
    pub fn area(&self, board: &PcbBoard, mut zone: Zone) -> Zone {
        let edits = [
            self.area_net.map(ZoneEdit::Net),
            self.pour_clearance_mm.map(ZoneEdit::Clearance),
            self.pour_min_thickness_mm.map(ZoneEdit::MinThickness),
            self.pour_priority.map(ZoneEdit::Priority),
            self.pour_min_island_mm2.map(ZoneEdit::MinIslandArea),
            self.pour_remove_dead_islands
                .map(ZoneEdit::RemoveDeadIslands),
        ];
        for edit in edits.into_iter().flatten() {
            // Each value was checked by `apply_zone_edit` when it was set,
            // so none is refused here.
            apply_zone_edit(board, &mut zone, edit)
                .ignore("each value was checked when it was set");
        }
        zone
    }

    /// The via the Via tool places at `at` on `board`.
    pub fn via(&self, board: &PcbBoard, id: Uuid, at: Point) -> Via {
        let mut via = draw::new_via(board, id, at);
        if let Some(diameter) = self.via_diameter_mm {
            via.diameter = diameter;
        }
        via.drill = self.via_hole_mm.unwrap_or(via.drill).min(via.diameter);
        if let Some(net) = self.via_net {
            via.net = net;
        }
        via
    }

    /// The style of a track run starting at `at` on `layer`.
    pub fn track_style(&self, board: &PcbBoard, at: Point, layer: CopperLayer) -> TrackStyle {
        let mut style = TrackStyle::starting_at(board, at, layer);
        if let Some(width) = self.track_width_mm {
            style.width_mm = width;
        }
        if let Some(net) = self.track_net {
            style.net = net;
        }
        style
    }
}

/// A pause for editing: the values and active layer it began with, which
/// Esc puts back. The sheet and footprint twin is
/// `state::SheetPlacementPause`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementPause {
    pub defaults: BoardToolDefaults,
    pub active_layer: PcbLayer,
    /// The width and net of the run being drawn, which a width or net
    /// edit while paused changes too.
    pub route: Option<TrackStyle>,
}

/// Whether Properties shows a form for `tool` while it is armed: every tool
/// that can be armed on a board but Differential Pair Routing and
/// Multi-Route, whose width and gap come from the rules (#2389).
pub(crate) fn has_form(tool: BoardPlaceTool) -> bool {
    matches!(tool, BoardPlaceTool::Via)
        || is_one_track(tool)
        || is_area(tool)
        || is_free(tool)
        || tool.graphic_kind().is_some()
}

/// Whether `tool` draws one track run whose width and net the form sets.
fn is_one_track(tool: BoardPlaceTool) -> bool {
    matches!(
        tool,
        BoardPlaceTool::Track | BoardPlaceTool::InteractiveRouting | BoardPlaceTool::QuickRoute
    )
}

/// Whether `tool` places a free pad, arc, string or dimension (#2373),
/// whose values `free_form` sets.
pub(super) fn is_free(tool: BoardPlaceTool) -> bool {
    matches!(
        tool,
        BoardPlaceTool::Pad
            | BoardPlaceTool::Arc
            | BoardPlaceTool::TextString
            | BoardPlaceTool::Dimension(_)
    )
}

impl Signex {
    /// The via the tool armed on the board in view places next, at the
    /// pointer.
    pub(crate) fn next_board_via(&self, board: &PcbBoard) -> Via {
        let at = self.board_tool_cursor();
        self.interaction_state
            .pcb_tool_defaults
            .via(board, Uuid::nil(), at)
    }

    /// The track the tool armed on the board in view places next, as a
    /// point at the pointer: the run's style mid-run, else the tool's.
    pub(crate) fn next_board_track(&self, board: &PcbBoard) -> Segment {
        let at = self.board_tool_cursor();
        let layer = self.pcb_active_layer();
        let style = self
            .own_board_tool()
            .and_then(|s| s.route)
            .unwrap_or_else(|| {
                self.interaction_state
                    .pcb_tool_defaults
                    .track_style(board, at, layer)
            });
        Segment {
            uuid: Uuid::nil(),
            start: at,
            end: at,
            width: style.width_mm,
            layer: pcb_layer_token(PcbLayer::Copper(layer)),
            net: style.net,
            unknown_columns: Default::default(),
            attributes: Default::default(),
            locked: false,
        }
    }

    pub(super) fn board_tool_cursor(&self) -> Point {
        self.own_board_tool()
            .and_then(|s| s.cursor)
            .unwrap_or(Point::new(0.0, 0.0))
    }

    /// A tool with a form was armed: Properties comes to the front on what
    /// the next click places, as on a sheet. The canvas keeps the keyboard,
    /// so a Properties window of its own is left where it is.
    pub(crate) fn raise_board_tool_form(&mut self) -> Task<Message> {
        let armed = self.own_board_tool().is_some_and(|s| has_form(s.tool));
        let kind = crate::panels::PanelKind::Properties;
        let docked = self.document_view.dock.locate(kind).is_some();
        if !armed || (!docked && self.is_panel_open(kind)) {
            return Task::none();
        }
        self.show_panel(kind)
    }

    /// Tab while a tool is armed on the board in view: pause it for its
    /// values to be edited, or go on placing when paused. `None` when no
    /// tool with a form is armed there.
    pub(crate) fn toggle_board_tool_pause(&mut self) -> Option<Task<Message>> {
        let session = self.own_board_tool()?;
        if !has_form(session.tool) {
            return None;
        }
        if session.paused.is_some() {
            self.resume_board_tool();
            return Some(Task::none());
        }
        let pause = PlacementPause {
            defaults: self.interaction_state.pcb_tool_defaults.clone(),
            active_layer: self.pcb_layer_view(&session.path).active(),
            route: session.route,
        };
        if let Some(session) = self.own_board_tool_mut() {
            session.paused = Some(pause);
        }
        self.refresh_pcb_highlight();
        let show = self.show_panel(crate::panels::PanelKind::Properties);
        let input = PLACEMENT_INPUT_ID.clone();
        Some(Task::batch([
            show,
            operation::focus(input.clone()),
            operation::text_input::select_all(input),
        ]))
    }

    /// Go on placing with the values in the panel: text typed and not yet
    /// applied is applied first.
    pub(crate) fn resume_board_tool(&mut self) {
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let target = PrimitiveTarget::placement(session.path.clone());
        let panel = &mut self.interaction_state.pcb_primitive;
        if panel.draft_target.as_ref() == Some(&target) {
            panel.draft_target = None;
            let drafts = std::mem::take(&mut panel.drafts);
            for (field, text) in drafts {
                self.submit_primitive_text(&target, field, &text);
            }
        }
        if let Some(session) = self.own_board_tool_mut() {
            session.paused = None;
            forget_routed_leg(session);
        }
        self.refresh_pcb_highlight();
    }

    /// Esc while paused: the values and active layer the pause began with
    /// come back, and placing goes on. Whether a pause took the key.
    pub(super) fn revert_board_tool_pause(&mut self) -> bool {
        let Some(session) = self.own_board_tool_mut() else {
            return false;
        };
        let Some(pause) = session.paused.take() else {
            return false;
        };
        session.route = pause.route;
        forget_routed_leg(session);
        let window = session.window;
        let path = session.path.clone();
        self.interaction_state.pcb_tool_defaults = pause.defaults;
        self.interaction_state.pcb_primitive.drop_drafts_of(&path);
        if self.pcb_layer_view(&path).active() != pause.active_layer {
            self.apply_pcb_view_event(window, PcbViewMsg::ActiveLayerPicked(pause.active_layer));
        }
        self.refresh_pcb_highlight();
        true
    }

    /// Whether the tool armed on the board in view is paused.
    pub(crate) fn is_board_tool_paused(&self) -> bool {
        self.own_board_tool().is_some_and(|s| s.paused.is_some())
    }

    /// Whether a paused tool is armed on the board `window_id` shows: the
    /// Placement Paused banner is drawn over that canvas.
    pub(crate) fn board_tool_paused_in_window(&self, window_id: iced::window::Id) -> bool {
        let Some(session) = self.interaction_state.pcb_tool.as_ref() else {
            return false;
        };
        let window = (self.ui_state.main_window_id != Some(window_id)).then_some(window_id);
        let shown = self
            .document_state
            .tab_shown_in_window(window_id, &self.ui_state)
            .map(|tab| tab.path.as_path());
        session.paused.is_some() && session.is_for(shown, window)
    }

    /// An edit in the panel of the object about to be placed: it sets the
    /// armed tool's value. A refused value changes nothing and says why.
    pub(crate) fn edit_board_tool_value(&mut self, edit: &PrimitiveEdit, what: MessageId) {
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let (tool, window) = (session.tool, session.window);
        // A Graphics Line or Arc on copper sets the Track or Arc values.
        let tool = self.placing_tool(tool);
        if tool.graphic_kind().is_some() {
            self.edit_graphic_tool_value(edit, what);
            return;
        }
        let layer = match edit {
            PrimitiveEdit::Layer(layer) => Some(PcbLayer::Copper(*layer)),
            PrimitiveEdit::AreaLayer(layer) => Some(*layer),
            _ => None,
        };
        if let Some(layer) = layer {
            // What the tool draws goes on the active layer.
            let area = self.active_pcb().and_then(|b| self.next_board_area(b));
            // A dimension takes any layer, copper included (Caner, 2026-11-21).
            let text_tool = tool == BoardPlaceTool::TextString;
            if area.is_some_and(|zone| !zone_layer_allowed(&zone, layer))
                || (text_tool && !layer.takes_text())
            {
                self.refuse_primitive_edit(what, PrimitiveEditRefusal::LayerNotAllowed.to_string());
                return;
            }
            self.apply_pcb_view_event(window, PcbViewMsg::ActiveLayerPicked(layer));
            self.refresh_pcb_highlight();
            return;
        }
        if is_area(tool) {
            self.edit_area_tool_value(edit, what);
            return;
        }
        if is_free(tool) {
            self.edit_free_tool_value(tool, edit, what);
            return;
        }
        let size = match (tool, edit) {
            (BoardPlaceTool::Via, PrimitiveEdit::Number(F::Diameter | F::HoleSize, v)) => Some(*v),
            (tool, PrimitiveEdit::Number(F::Width, v)) if is_one_track(tool) => Some(*v),
            _ => None,
        };
        if size.is_some_and(|v| v <= 0.0) {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_POSITIVE).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        }
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        match (tool, edit) {
            (BoardPlaceTool::Via, PrimitiveEdit::Number(F::Diameter, v)) => {
                defaults.via_diameter_mm = Some(*v);
            }
            (BoardPlaceTool::Via, PrimitiveEdit::Number(F::HoleSize, v)) => {
                defaults.via_hole_mm = Some(*v);
            }
            (BoardPlaceTool::Via, PrimitiveEdit::Net(net)) => defaults.via_net = Some(*net),
            (tool, PrimitiveEdit::Number(F::Width, v)) if is_one_track(tool) => {
                defaults.track_width_mm = Some(*v);
            }
            (tool, PrimitiveEdit::Net(net)) if is_one_track(tool) => {
                defaults.track_net = Some(*net);
            }
            // The location of what is not placed yet is the pointer's.
            _ => return,
        }
        // A run being drawn goes on with the new width and net.
        let (width, net) = (defaults.track_width_mm, defaults.track_net);
        if let Some(route) = self.own_board_tool_mut().and_then(|s| s.route.as_mut()) {
            route.width_mm = width.unwrap_or(route.width_mm);
            route.net = net.unwrap_or(route.net);
        }
        self.refresh_pcb_highlight();
    }

    /// An edit of the area an area tool places next, checked on that area
    /// the way an edit of a placed area is.
    fn edit_area_tool_value(&mut self, edit: &PrimitiveEdit, what: MessageId) {
        let Some(zone_edit) = zone_edit(edit) else {
            return;
        };
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(mut zone) = self.next_board_area(board) else {
            return;
        };
        if let Err(refusal) = apply_zone_edit(board, &mut zone, zone_edit) {
            self.refuse_primitive_edit(what, refusal.to_string());
            return;
        }
        let is_copper = zone.fill_type != KEEPOUT_FILL;
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        match zone_edit {
            ZoneEdit::Net(net) if is_copper => defaults.area_net = Some(net),
            ZoneEdit::Clearance(v) if zone.is_pour() => defaults.pour_clearance_mm = Some(v),
            ZoneEdit::MinThickness(v) if zone.is_pour() => {
                defaults.pour_min_thickness_mm = Some(v);
            }
            ZoneEdit::Priority(v) if zone.is_pour() => defaults.pour_priority = Some(v),
            ZoneEdit::MinIslandArea(v) if zone.is_pour() => {
                defaults.pour_min_island_mm2 = Some(v);
            }
            ZoneEdit::RemoveDeadIslands(on) if zone.is_pour() => {
                defaults.pour_remove_dead_islands = Some(on);
            }
            _ => return,
        }
        self.refresh_pcb_highlight();
    }
}

/// Drop the leg routed for the pointer, so it is routed again with the
/// values the pause left: its key holds no width or net.
fn forget_routed_leg(session: &mut super::BoardToolSession) {
    if let Some(router) = session.router.as_mut() {
        router.preview = None;
    }
}

/// Whether `tool` draws an area.
pub(super) fn is_area(tool: BoardPlaceTool) -> bool {
    matches!(
        tool,
        BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::Keepout
    )
}
