//! The symbol editor's Properties panel messages (#122, #123): its tab, the
//! Pins tab filter and part chip and the illustration hover change only the
//! panel state; `‹ Pins`, `+ Pin`, the Parts stepper and the Body choice act
//! on the editor through the same paths as the canvas toolbar.

use super::*;
use crate::library::editor::symbol::{PartStep, SymbolPropsMsg};
use crate::library::messages::{SymbolEditorMsg, SymbolToolMsg};

impl Signex {
    pub(super) fn handle_symbol_props(&mut self, msg: &SymbolPropsMsg) -> Task<Message> {
        let Some(path) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.kind.as_symbol_editor())
            .cloned()
        else {
            return Task::none();
        };
        let panel_only = self
            .library
            .symbol_props
            .entry(path.clone())
            .or_default()
            .apply(msg);
        if panel_only {
            self.refresh_panel_ctx();
            return Task::none();
        }
        let editor_msg = match msg {
            SymbolPropsMsg::PinsBackPressed => {
                self.sym_editor_clear_selection();
                return Task::none();
            }
            SymbolPropsMsg::AddPinPressed => SymbolEditorMsg::SetTool(SymbolToolMsg::AddPin),
            SymbolPropsMsg::PartStepPressed(step) => match step {
                PartStep::Previous => SymbolEditorMsg::PrevPart,
                PartStep::Next => SymbolEditorMsg::NextPart,
                PartStep::Added => SymbolEditorMsg::NewPart,
                PartStep::Removed => SymbolEditorMsg::RemovePart,
            },
            SymbolPropsMsg::BodyPicked(body) => SymbolEditorMsg::BodyStyleSelected(*body),
            SymbolPropsMsg::TabPicked(_)
            | SymbolPropsMsg::PinFilterChanged(_)
            | SymbolPropsMsg::PartChipPicked(_)
            | SymbolPropsMsg::IllustrationHovered(_) => return Task::none(),
        };
        self.update(Message::Library(
            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: crate::library::messages::PrimitiveEdit::Symbol(editor_msg),
            },
        ))
    }

    /// Clear the active symbol editor's selection, so Properties shows the
    /// symbol again.
    fn sym_editor_clear_selection(&mut self) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if editor.selected.take().is_some() {
            editor.graphic_fill_picker = None;
            editor.local_color_picker = None;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }
}
