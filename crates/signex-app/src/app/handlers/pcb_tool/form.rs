//! The values a board placement tool places with (#2393), shown in
//! Properties while the tool is armed. A value nobody set follows the board:
//! its via size and track width (`PcbSetup`), the net of the copper under
//! the point, the active layer. Values set in the panel stay the tool's for
//! the session.
//!
//! Tab (`placement_properties`) pauses placing and focuses the panel; Enter
//! in a field or Tab again goes on placing with the values; Esc puts back
//! the values the pause began with.

use iced::Task;
use iced::widget::operation;
use signex_engine::pcb::draw::{self, TrackStyle};
use signex_i18n::{MessageId, ids, t};
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use crate::app::handlers::pcb_primitive::PrimitiveEdit;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, PcbViewMsg, Signex};
use crate::panels::pcb_primitive_properties::{
    PLACEMENT_INPUT_ID, PrimitiveField as F, PrimitiveTarget,
};

/// The values the via and track tools place with; `None` follows the board.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardToolDefaults {
    pub via_diameter_mm: Option<f64>,
    pub via_hole_mm: Option<f64>,
    /// 0 is no net.
    pub via_net: Option<u32>,
    pub track_width_mm: Option<f64>,
    /// 0 is no net.
    pub track_net: Option<u32>,
}

impl BoardToolDefaults {
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
/// Esc puts back.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementPause {
    pub defaults: BoardToolDefaults,
    pub active_layer: PcbLayer,
}

/// Whether Properties shows a form for `tool` while it is armed.
pub(crate) fn has_form(tool: BoardPlaceTool) -> bool {
    matches!(tool, BoardPlaceTool::Via | BoardPlaceTool::Track)
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
        }
    }

    fn board_tool_cursor(&self) -> Point {
        self.own_board_tool()
            .and_then(|s| s.cursor)
            .unwrap_or(Point::new(0.0, 0.0))
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

    /// An edit in the panel of the object about to be placed: it sets the
    /// armed tool's value. A refused value changes nothing and says why.
    pub(crate) fn edit_board_tool_value(&mut self, edit: &PrimitiveEdit, what: MessageId) {
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let (tool, window) = (session.tool, session.window);
        if let PrimitiveEdit::Layer(layer) = edit {
            let layer = PcbLayer::Copper(*layer);
            self.apply_pcb_view_event(window, PcbViewMsg::ActiveLayerPicked(layer));
            self.refresh_pcb_highlight();
            return;
        }
        let size = match (tool, edit) {
            (BoardPlaceTool::Via, PrimitiveEdit::Number(F::Diameter | F::HoleSize, v))
            | (BoardPlaceTool::Track, PrimitiveEdit::Number(F::Width, v)) => Some(*v),
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
            (BoardPlaceTool::Track, PrimitiveEdit::Number(F::Width, v)) => {
                defaults.track_width_mm = Some(*v);
            }
            (BoardPlaceTool::Track, PrimitiveEdit::Net(net)) => defaults.track_net = Some(*net),
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
}
