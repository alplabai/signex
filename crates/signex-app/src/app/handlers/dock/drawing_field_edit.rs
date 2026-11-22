//! Typing in a drawing's number box (#2767, #2768). A value the box can store
//! is previewed on the sheet without touching the history or the dirty mark.
//! Enter, leaving the box, another selection, closing Properties, a save or
//! any other edit commits it as one undo step, as a typed note does; Esc puts
//! the old value back. Text the box cannot store stays in it, marked invalid,
//! and the document keeps its value.

use super::super::super::*;
use crate::app::core::event::{ChangeCause, CoreEvent, DocumentChanged};
use crate::app::projector::RefreshScope;
use crate::app::state::{LiveDrawingEdit, WindowKind};
use crate::panels::{DrawingFieldId, PanelKind};
use signex_types::schematic::SelectedKind;

impl Signex {
    pub(super) fn type_drawing_field(&mut self, field: DrawingFieldId, text: &str) {
        let selected = self.document_state.panel_ctx.selected_uuid;
        let other_box = (self.document_state.live_drawing_edit.as_ref())
            .is_some_and(|live| live.field != field || Some(live.drawing) != selected);
        if other_box {
            self.commit_live_drawing_edit();
        }
        // A prior drawing's half-typed text must not bleed into this one.
        let ctx = &mut self.document_state.panel_ctx;
        if ctx.drawing_edit_buf_for != selected {
            ctx.drawing_edit_buf.clear();
            ctx.drawing_edit_buf_for = selected;
        }
        ctx.drawing_edit_buf.insert(field, text.to_owned());
        if let Some(typed) = crate::numeric_field::parse_field_f64(text)
            && field.reads(text)
        {
            // A typed Y is read from the sheet's page origin (#2034).
            let value = match self.document_state.active_engine() {
                Some(engine) if field.is_page_y() => engine.document().page_y(typed),
                _ => typed,
            };
            self.preview_drawing_field(field, value);
        } else if field.refuses(text) {
            self.cancel_live_drawing_edit();
        }
    }

    /// Enter, or the box lost the focus. A value the box reads is committed.
    /// Refused text stays to be fixed and an empty box shows the stored
    /// value again; either way the sheet goes back to its old value.
    pub(super) fn leave_drawing_field(&mut self, field: DrawingFieldId) {
        let Some(text) = self.document_state.panel_ctx.drawing_edit_buf.get(&field) else {
            return;
        };
        let (reads, refused) = (field.reads(text), field.refuses(text));
        let live_here = (self.document_state.live_drawing_edit.as_ref())
            .is_some_and(|live| live.field == field);
        if reads && live_here {
            self.commit_live_drawing_edit();
        } else if live_here {
            self.cancel_live_drawing_edit();
        }
        if !refused {
            self.drop_drawing_field_text(Some(field));
        }
    }

    fn preview_drawing_field(&mut self, field: DrawingFieldId, value: f64) {
        let ctx = &self.document_state.panel_ctx;
        let target = ctx
            .selected_uuid
            .filter(|_| matches!(ctx.selected_kind, Some(SelectedKind::Drawing)));
        let Some(target) = target else {
            return;
        };
        if self.refuse_if_channel_view().is_some() {
            return;
        }
        let Some(path) = self.document_state.edit_path().map(|p| p.to_path_buf()) else {
            return;
        };
        if self.refuse_if_read_only_at(&path).is_some() {
            return;
        }
        let Some(next) = self.drawing_with_edit(target, field.edit(value)) else {
            return;
        };
        let panel_open = self.properties_panel_is_open();
        let Some(engine) = self.document_state.engines.get_mut(&path) else {
            return;
        };
        let previewed = engine.preview_drawing(next);
        Self::report_engine_warnings(engine);
        match previewed {
            Ok(result) => {
                let Some(pair) = result.patch_pair else {
                    return;
                };
                let open = self.document_state.live_drawing_edit.as_ref();
                let panel_open = open.map_or(panel_open, |live| live.panel_open);
                self.document_state.live_drawing_edit = Some(LiveDrawingEdit {
                    path: path.clone(),
                    drawing: target,
                    field,
                    panel_open,
                });
                self.project_drawing_preview(path, pair.document);
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!("Properties: {error}"));
            }
        }
    }

    /// Repaint the sheet; the dirty mark and the history are left alone.
    fn project_drawing_preview(
        &mut self,
        path: std::path::PathBuf,
        patch: signex_engine::DocumentPatch,
    ) {
        let cause = ChangeCause::Edit;
        let event: CoreEvent = DocumentChanged { path, patch, cause }.into();
        let scope = RefreshScope::ActiveCanvas {
            clear_overlay: true,
            update_selection_info: true,
        };
        self.project_events(vec![event], scope);
    }

    /// Land the open preview as one undo step and mark the sheet.
    pub(crate) fn commit_live_drawing_edit(&mut self) {
        let Some(live) = self.document_state.live_drawing_edit.take() else {
            return;
        };
        if let Some(engine) = self.document_state.engines.get_mut(&live.path) {
            engine.commit_live_edit();
            self.document_state.mark_history_edit_dirty(&live.path);
        }
        self.drop_drawing_field_text(Some(live.field));
    }

    /// Put the sheet back as it was before the preview; the box's text is
    /// the caller's to keep or drop.
    fn cancel_live_drawing_edit(&mut self) {
        let Some(live) = self.document_state.live_drawing_edit.take() else {
            return;
        };
        let patch = (self.document_state.engines.get_mut(&live.path))
            .and_then(|engine| engine.cancel_live_edit());
        if let Some(pair) = patch
            && self.document_state.active_path.as_ref() == Some(&live.path)
        {
            self.project_drawing_preview(live.path, pair.document);
        }
    }

    /// Esc in a drawing box: the old value and no typed text. `true` when
    /// there was something to restore, so the key goes no further.
    pub(crate) fn restore_drawing_field(&mut self) -> bool {
        let typed = !self.document_state.panel_ctx.drawing_edit_buf.is_empty()
            || (self.document_state.window_panel_ctx.values())
                .any(|ctx| !ctx.drawing_edit_buf.is_empty());
        if !typed && self.document_state.live_drawing_edit.is_none() {
            return false;
        }
        self.cancel_live_drawing_edit();
        self.drop_drawing_field_text(None);
        true
    }

    /// Run after every message: the edit commits once its drawing is no
    /// longer selected, Properties closed, or the engine already landed it
    /// under another edit. Text left for another drawing goes too.
    pub(crate) fn settle_live_drawing_edit(&mut self) {
        self.drop_text_of_another_drawing();
        let Some(live) = self.document_state.live_drawing_edit.clone() else {
            return;
        };
        let engine = self.document_state.engines.get(&live.path);
        let in_engine = engine.is_some_and(|e| e.live_edit_drawing() == Some(live.drawing));
        let docs = &self.document_state;
        let selected = docs.panel_ctx.selected_uuid == Some(live.drawing)
            || (docs.window_panel_ctx.values()).any(|c| c.selected_uuid == Some(live.drawing));
        let panel_closed = live.panel_open && !self.properties_panel_is_open();
        if in_engine && selected && !panel_closed {
            return;
        }
        self.commit_live_drawing_edit();
    }

    fn drop_text_of_another_drawing(&mut self) {
        let ctx = &mut self.document_state.panel_ctx;
        if !ctx.drawing_edit_buf.is_empty() && ctx.drawing_edit_buf_for != ctx.selected_uuid {
            ctx.drawing_edit_buf.clear();
        }
    }

    /// Forget the typed text of `field` (`None`: of every box), in the main
    /// Properties and every undocked window's.
    fn drop_drawing_field_text(&mut self, field: Option<DrawingFieldId>) {
        let docs = &mut self.document_state;
        let panels = std::iter::once(&mut docs.panel_ctx).chain(docs.window_panel_ctx.values_mut());
        for ctx in panels {
            match field {
                Some(field) => {
                    ctx.drawing_edit_buf.remove(&field);
                }
                None => ctx.drawing_edit_buf.clear(),
            }
        }
    }

    fn properties_panel_is_open(&self) -> bool {
        self.document_state
            .dock
            .locate(PanelKind::Properties)
            .is_some()
            || (self.ui_state.windows.values())
                .any(|w| matches!(w, WindowKind::DetachedPanel(PanelKind::Properties)))
    }
}
