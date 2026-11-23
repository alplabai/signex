//! The Properties panel of a Bezier, dimension or table: its edits, each one
//! undo step through `Command::UpdateSchDrawing`, and the panel state the
//! view reads (half-typed text, the sheet's parts, a gone source).

use iced::Task;
use signex_types::schematic::{SchDrawing, SelectedKind};

use super::TablesMsg;
use crate::app::{Message, Signex};
use crate::panels::{AnnotationPanelMsg, AnnotationPanelState};

impl Signex {
    /// The selected Bezier, dimension or table on the active sheet.
    fn selected_annotation(&self) -> Option<uuid::Uuid> {
        let ctx = &self.document_state.panel_ctx;
        if ctx.selected_kind != Some(SelectedKind::Drawing) {
            return None;
        }
        let id = ctx.selected_uuid?;
        let engine = self.document_state.active_engine()?;
        engine
            .document()
            .drawings
            .iter()
            .any(|d| d.uuid() == id && d.needs_format_1_3())
            .then_some(id)
    }

    pub(crate) fn handle_annotation_panel(&mut self, msg: AnnotationPanelMsg) -> Task<Message> {
        if self.is_dimension_tool_form_shown() {
            return self.handle_dimension_tool_panel(msg);
        }
        let Some(id) = self.selected_annotation() else {
            return Task::none();
        };
        match msg {
            AnnotationPanelMsg::FieldTyped(field, text) => {
                let edit = match self.document_state.active_engine() {
                    Some(engine) => {
                        let sheet = engine.document();
                        field.edit(&text, |y| sheet.page_y(y))
                    }
                    None => None,
                };
                self.document_state.panel_ctx.annotation.draft = Some((id, field, text));
                if let Some(edit) = edit {
                    self.apply_annotation_edit_to(id, &edit);
                }
            }
            AnnotationPanelMsg::FieldSubmitted(field) => {
                let ours = matches!(
                    &self.document_state.panel_ctx.annotation.draft,
                    Some((d, f, _)) if *d == id && *f == field
                );
                if ours {
                    self.document_state.panel_ctx.annotation.draft = None;
                }
            }
            AnnotationPanelMsg::Picked(edit) => {
                self.document_state.panel_ctx.annotation.draft = None;
                self.apply_annotation_edit_to(id, &edit);
            }
            // Only the armed tool's form has this box.
            AnnotationPanelMsg::OverrideEscaped => {}
            AnnotationPanelMsg::CsvImportRequested => {
                return self.dispatch_tables(TablesMsg::CsvImportRequested(id));
            }
            AnnotationPanelMsg::CsvExportRequested => {
                return self.dispatch_tables(TablesMsg::CsvExportRequested(id));
            }
        }
        Task::none()
    }

    /// The panel state for the next frame: the draft kept while the same
    /// drawing stays selected, the active sheet's parts, and whether the
    /// selected table's part is gone.
    pub(crate) fn annotation_panel_state(&self) -> AnnotationPanelState {
        let ctx = &self.document_state.panel_ctx;
        let dimension_tool = self.dimension_tool_form();
        // The armed tool's form types under the nil id.
        let draft = ctx.annotation.draft.clone().filter(|(id, _, _)| {
            ctx.selected_uuid == Some(*id) || (dimension_tool.is_some() && id.is_nil())
        });
        let is_tool_paused_elsewhere = self.is_annotation_pause_of_another(None);
        let Some(engine) = self.document_state.active_engine() else {
            return AnnotationPanelState {
                draft,
                dimension_tool,
                is_tool_paused_elsewhere,
                ..AnnotationPanelState::default()
            };
        };
        let sheet = engine.document();
        let source_missing = match &ctx.selected_drawing {
            Some(SchDrawing::Table(t)) => super::source_is_missing(sheet, t),
            _ => false,
        };
        AnnotationPanelState {
            draft,
            designators: super::designators(sheet),
            source_missing,
            dimension_tool,
            is_tool_paused_elsewhere,
        }
    }
}
