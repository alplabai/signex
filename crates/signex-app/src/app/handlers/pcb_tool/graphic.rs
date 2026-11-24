//! The Graphics tools (#2424), on the active non-copper layer (not a
//! keep-out), each drawing one undo step:
//! - Line: a run; each click adds a line from the last point, a click on
//!   the last point ends the run;
//! - Arc: the centre, the start (its distance is the radius), the end;
//! - Full Circle: the centre, a point on the circle;
//! - Rectangle and Text Frame: two opposite corners.
//!
//! What the next click adds follows the pointer. The line width, a text
//! frame's text and its height are the tool's values (#2393 form).

use signex_engine::pcb::graphic::{self as gfx, GraphicStyle};
use signex_engine::pcb::primitive_edit::GraphicEdit;
use signex_engine::pcb::primitive_edit::apply_graphic_edit;
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::{
    BoardGraphic, PCB_DEFAULT_GRAPHIC_WIDTH_MM, PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard, Point,
};
use signex_types::pcb_graphic::{BoardGraphicKind, takes_board_graphics};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::tool_layer::DRAWING_LAYER;
use super::{BoardToolDefaults, BoardToolSession};
use crate::app::handlers::pcb_primitive::{PrimitiveEdit, graphic_edit};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{PcbViewMsg, Signex};

impl BoardToolDefaults {
    /// The layer and width a Graphics tool draws with on `layer`.
    pub fn graphic_style(&self, layer: PcbLayer) -> GraphicStyle {
        GraphicStyle {
            layer,
            width_mm: self
                .graphic_width_mm
                .unwrap_or(PCB_DEFAULT_GRAPHIC_WIDTH_MM),
        }
    }

    /// A new text frame's text height in mm.
    pub fn frame_text_height(&self) -> f64 {
        self.frame_text_height_mm
            .unwrap_or(PCB_DEFAULT_TEXT_SIZE_MM)
    }
}

/// How many points a drawing of `kind` takes.
fn points_needed(kind: BoardGraphicKind) -> usize {
    match kind {
        BoardGraphicKind::Arc => 3,
        _ => 2,
    }
}

/// What a Graphics tool draws from its points.
struct Recipe<'a> {
    kind: BoardGraphicKind,
    style: GraphicStyle,
    text: &'a str,
    text_height: f64,
}

impl Recipe<'_> {
    /// The drawing `points` make, named `id`; `None` while there are too few
    /// points or they make nothing. An arc with its centre and start only
    /// is drawn as its full circle, for the preview.
    fn drawing(&self, id: Uuid, points: &[Point]) -> Option<BoardGraphic> {
        let style = self.style;
        match (self.kind, points) {
            (BoardGraphicKind::Line, [.., a, b]) => gfx::new_line(id, *a, *b, style),
            (BoardGraphicKind::Arc, [c, s, e, ..]) => gfx::new_arc(id, *c, *s, *e, style),
            (BoardGraphicKind::Arc, [c, s]) => gfx::new_arc(id, *c, *s, *s, style),
            (BoardGraphicKind::Circle, [c, r, ..]) => gfx::new_circle(id, *c, *r, style),
            (BoardGraphicKind::Rectangle, [a, b, ..]) => gfx::new_rectangle(id, *a, *b, style),
            (BoardGraphicKind::TextFrame, [a, b, ..]) => {
                gfx::new_text_frame(id, *a, *b, style, self.text.to_string(), self.text_height)
            }
            _ => None,
        }
    }
}

impl Signex {
    /// The text a new text frame reads: the tool's, else the catalog's.
    fn frame_text(&self) -> String {
        self.interaction_state
            .pcb_tool_defaults
            .frame_text
            .clone()
            .unwrap_or_else(|| t!(self.ui_state.i18n, ids::PCB_TOOL_TEXT_FRAME_TEXT).into_string())
    }

    /// The active layer of the board in view when a drawing may go on it.
    fn active_graphic_layer(&self) -> Option<PcbLayer> {
        let path = self.active_tab_path()?;
        Some(self.pcb_layer_view(&path).active()).filter(|layer| takes_board_graphics(*layer))
    }

    /// What `kind` draws on `layer` with the tool's values, with `text`.
    fn recipe<'a>(&self, kind: BoardGraphicKind, layer: PcbLayer, text: &'a str) -> Recipe<'a> {
        let defaults = &self.interaction_state.pcb_tool_defaults;
        Recipe {
            kind,
            style: defaults.graphic_style(layer),
            text,
            text_height: defaults.frame_text_height(),
        }
    }

    /// A Graphics tool click at `at`: a point of the drawing, or the drawing
    /// itself once it has its points. Off a layer a drawing may go on, the
    /// Messages panel and the status bar say so and nothing changes.
    pub(super) fn graphic_clicked(&mut self, tool: BoardPlaceTool, at: Point) {
        let Some(kind) = tool.graphic_kind() else {
            return;
        };
        let name = t!(self.ui_state.i18n, tool.name()).into_string();
        let Some(layer) = self.active_graphic_layer() else {
            let note = t!(
                self.ui_state.i18n,
                ids::PCB_TOOL_NEEDS_GRAPHIC_LAYER,
                "tool" => name.as_str()
            );
            self.note_board_tool(note.into_string());
            return;
        };
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let mut points = session.points.clone();
        if kind == BoardGraphicKind::Line && points.last() == Some(&at) {
            // A click on the run's end ends the run.
            self.set_tool_points(Vec::new());
            return;
        }
        if points.last() == Some(&at) {
            return;
        }
        points.push(at);
        if points.len() < points_needed(kind) {
            self.set_tool_points(points);
            return;
        }
        let text = self.frame_text();
        let drawing = self
            .recipe(kind, layer, &text)
            .drawing(Uuid::now_v7(), &points);
        let Some(drawing) = drawing else {
            if matches!(
                kind,
                BoardGraphicKind::Rectangle | BoardGraphicKind::TextFrame
            ) {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_AREA, "tool" => name.as_str());
                crate::diagnostics::log_warning(note.into_string());
                self.set_tool_points(Vec::new());
            }
            // A point on the centre draws nothing; the click waits for another.
            return;
        };
        self.run_placement(&name, Ok(vec![gfx::place_graphic(drawing)]));
        // A line run goes on from its end; every other drawing starts anew.
        let next = match kind {
            BoardGraphicKind::Line => vec![at],
            _ => Vec::new(),
        };
        self.set_tool_points(next);
    }

    fn set_tool_points(&mut self, points: Vec<Point>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points = points;
        }
    }

    /// The drawing the next click finishes, drawn into `preview`.
    pub(super) fn graphic_preview(
        &self,
        session: &BoardToolSession,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let (Some(kind), Some(layer)) = (session.tool.graphic_kind(), self.active_graphic_layer())
        else {
            return;
        };
        let text = self.frame_text();
        let points: Vec<Point> = session.points.iter().copied().chain([cursor]).collect();
        preview.graphics.extend(
            self.recipe(kind, layer, &text)
                .drawing(Uuid::nil(), &points),
        );
    }

    /// The drawing the Graphics tool armed on the board in view places next,
    /// for its Properties form: its values on a stand-in shape at the
    /// pointer. The layer is the active one, or the top overlay while the
    /// active one cannot take a drawing.
    pub(crate) fn next_board_graphic(&self) -> Option<BoardGraphic> {
        let session = self.own_board_tool()?;
        let kind = session.tool.graphic_kind()?;
        let at = session.cursor.unwrap_or(Point::new(0.0, 0.0));
        let layer = self.active_graphic_layer().unwrap_or(DRAWING_LAYER);
        let text = self.frame_text();
        let far = Point::new(at.x + 2.0, at.y + 1.0);
        let points = match kind {
            BoardGraphicKind::Arc => vec![at, far, Point::new(at.x, at.y + 2.0)],
            _ => vec![at, far],
        };
        self.recipe(kind, layer, &text)
            .drawing(Uuid::nil(), &points)
    }

    /// An edit of the drawing a Graphics tool places next: its layer (the
    /// active layer), line width, text or text height. Its points are the
    /// pointer's, so a point edit sets nothing. A refused value changes
    /// nothing and says why.
    pub(super) fn edit_graphic_tool_value(&mut self, edit: &PrimitiveEdit, what: MessageId) {
        let Some(change) = graphic_edit(edit) else {
            return;
        };
        let Some(mut next) = self.next_board_graphic() else {
            return;
        };
        if let Err(refusal) = apply_graphic_edit(&mut next, &change) {
            self.refuse_primitive_edit(what, refusal.to_string());
            return;
        }
        if let GraphicEdit::Layer(layer) = change {
            // What the tool draws goes on the active layer.
            let window = self.own_board_tool().and_then(|s| s.window);
            self.apply_pcb_view_event(window, PcbViewMsg::ActiveLayerPicked(layer));
            self.refresh_pcb_highlight();
            return;
        }
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        match change {
            GraphicEdit::Width(w) => defaults.graphic_width_mm = Some(w),
            GraphicEdit::Text(text) => defaults.frame_text = Some(text),
            GraphicEdit::TextHeight(h) => defaults.frame_text_height_mm = Some(h),
            _ => return,
        }
        self.refresh_pcb_highlight();
    }
}
