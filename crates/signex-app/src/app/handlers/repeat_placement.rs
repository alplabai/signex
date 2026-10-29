//! Repeated placement of text (#1253): the text of the next label steps by
//! the "Increment step" after every click, and "Repeat last item" arms the
//! last label or note again with that stepped text.

use iced::Task;
use signex_types::schematic::LabelType;
use signex_types::text_increment::increment_trailing_number;

use crate::app::state::LastPlacement;
use crate::app::{Message, Signex, Tool};
use crate::panels::{PrePlacementData, PrePlacementKind, PrePlacementStyle};
use crate::toolbar::ToolMessage;

impl Signex {
    /// A label with `placed` as its text has just been committed: step the
    /// text the next click will use and remember it for Repeat last item.
    pub(crate) fn label_placed(&mut self, placed: &str) {
        let next = self.stepped_text(placed);
        if let Some(pre) = self.document_state.panel_ctx.pre_placement.as_mut()
            && !pre.label_text.trim().is_empty()
        {
            pre.label_text = next.clone();
        }
        self.interaction_state.last_placement = Some(LastPlacement::Label {
            port: self.interaction_state.pending_port.clone(),
            text: next,
            style: self
                .document_state
                .panel_ctx
                .pre_placement
                .as_ref()
                .map(PrePlacementData::style),
        });
    }

    /// A text note with `placed` as its text has just been committed.
    pub(crate) fn text_note_placed(&mut self, placed: &str) {
        let text = self.stepped_text(placed);
        self.interaction_state.last_placement = Some(LastPlacement::Text { text });
    }

    fn stepped_text(&self, text: &str) -> String {
        increment_trailing_number(text, i64::from(self.ui_state.increment_step))
    }

    /// Repeat last item: arm the tool that placed a label or note last, with
    /// its stepped text. Says so in the Messages panel when nothing was
    /// placed yet.
    pub(crate) fn handle_repeat_last_item_requested(&mut self) -> Task<Message> {
        let Some(last) = self.interaction_state.last_placement.clone() else {
            crate::diagnostics::log_info(
                "Repeat last item: no label or text has been placed yet, so there is nothing to repeat.",
            );
            return self.finish_update();
        };
        let (tool, port, kind, text, style) = match last {
            LastPlacement::Label { port, text, style } => {
                let kind = match port.as_ref().map(|(label_type, _)| label_type) {
                    Some(LabelType::Global) => PrePlacementKind::GlobalPort,
                    Some(LabelType::Hierarchical) => PrePlacementKind::HierPort,
                    _ => PrePlacementKind::NetLabel,
                };
                (Tool::Label, port, kind, text, style)
            }
            LastPlacement::Text { text } => {
                (Tool::Text, None, PrePlacementKind::TextNote, text, None)
            }
        };
        let armed = self.update(Message::Tool(ToolMessage::SelectTool(tool)));
        self.interaction_state.pending_port = port;
        self.set_pending_text(kind, text, style);
        armed
    }

    /// Put `text` where the next click reads it: the pre-placement data. A
    /// form that is already open keeps its own look; a new one takes `style`.
    fn set_pending_text(
        &mut self,
        kind: PrePlacementKind,
        text: String,
        style: Option<PrePlacementStyle>,
    ) {
        let tool_name = format!("{}", self.interaction_state.current_tool);
        let (cursor_x_mm, cursor_y_mm) = (self.ui_state.cursor_x, self.ui_state.cursor_y);
        let slot = &mut self.document_state.panel_ctx.pre_placement;
        match slot.as_mut() {
            Some(pre) => {
                pre.kind = kind;
                pre.tool_name = tool_name;
                pre.label_text = text;
            }
            None => {
                let mut pre =
                    PrePlacementData::new(tool_name, kind, text, cursor_x_mm, cursor_y_mm);
                if let Some(style) = style {
                    pre.set_style(style);
                }
                *slot = Some(pre);
            }
        }
    }
}
