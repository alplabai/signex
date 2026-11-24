//! A drawing field previewed while it is typed (#2768): every keystroke is in
//! the document, none is in the history until the edit is committed, and a
//! cancel puts the sheet back.
//!
//! Anything else that lands in the history, an undo, a redo, a transaction or
//! a save first commits the open edit as its own step, so no other entry can
//! capture a preview in its pre-image.

use crate::*;
use signex_types::schematic::SchDrawing;

/// The open preview: the sheet before it and what it touches.
#[derive(Debug)]
pub(crate) struct LiveEdit {
    before: SchematicSheet,
    drawing: uuid::Uuid,
    patch: PatchPair,
}

impl Engine {
    /// Show `drawing` in the document without a history entry. The first
    /// call opens the edit; a call for another drawing commits the open one.
    /// Refused inside a batch or a transaction.
    pub fn preview_drawing(&mut self, drawing: SchDrawing) -> Result<CommandResult, EngineError> {
        if self.history_suppressed || self.transaction.is_some() {
            return Err(EngineError::TransactionOpen);
        }
        let id = drawing.uuid();
        if self.live_edit.as_ref().is_some_and(|l| l.drawing != id) {
            self.commit_live_edit();
        }
        let before = self.live_edit.is_none().then(|| self.document.clone());
        self.field_edit_run = None;
        self.history_suppressed = true;
        let result = self.execute(Command::UpdateSchDrawing { drawing });
        self.history_suppressed = false;
        if let Some(patch) = result.as_ref().ok().and_then(|done| done.patch_pair)
            && let Some(before) = before
        {
            self.live_edit = Some(LiveEdit {
                before,
                drawing: id,
                patch,
            });
        }
        result
    }

    /// The drawing an open preview is editing.
    pub fn live_edit_drawing(&self) -> Option<uuid::Uuid> {
        self.live_edit.as_ref().map(|live| live.drawing)
    }

    /// End the preview as one history entry. `None` when none is open, or
    /// when the drawings ended as they began, which records nothing.
    pub fn commit_live_edit(&mut self) -> Option<PatchPair> {
        let live = self.live_edit.take()?;
        let ended_as_it_began = live.before.drawings == self.document.drawings;
        self.land_live_edit(live, ended_as_it_began)
    }

    /// [`Engine::commit_live_edit`] for an edit that has already changed the
    /// document: the preview ended at `pre_image`, that edit's snapshot.
    pub(crate) fn commit_live_edit_under(&mut self, pre_image: &SchematicSheet) {
        if let Some(live) = self.live_edit.take() {
            let ended_as_it_began = live.before.drawings == pre_image.drawings;
            self.land_live_edit(live, ended_as_it_began);
        }
    }

    fn land_live_edit(&mut self, live: LiveEdit, ended_as_it_began: bool) -> Option<PatchPair> {
        if ended_as_it_began {
            return None;
        }
        self.record_grouped_history(live.before, live.patch, None);
        Some(live.patch)
    }

    /// End the preview by putting the sheet back. The patch to repaint, or
    /// `None` when none was open.
    pub fn cancel_live_edit(&mut self) -> Option<PatchPair> {
        let live = self.live_edit.take()?;
        self.document = live.before;
        self.bump_revision();
        Some(live.patch)
    }

    /// [`Engine::serialize_with_extents`] of the sheet without an open
    /// preview, for a recovery copy taken while a value is being typed.
    pub fn serialize_committed_with_extents(
        &self,
        measure: impl FnMut(&signex_types::schematic::TextProbe<'_>) -> Option<f64>,
    ) -> Result<String, EngineError> {
        let committed = self.live_edit.as_ref().map(|live| &live.before);
        let mut sheet = committed.unwrap_or(&self.document).clone();
        sheet.text_extents = sheet.text_extents.refreshed(&sheet, measure);
        Self::encode(sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_sheet;
    use signex_types::schematic::{FillType, NoConnect, Point};

    fn rect(id: uuid::Uuid, right: f64) -> SchDrawing {
        SchDrawing::Rect {
            uuid: id,
            start: Point::new(0.0, 0.0),
            end: Point::new(right, 10.0),
            width: 0.2,
            fill: FillType::None,
            stroke_color: None,
            locked: false,
        }
    }

    fn engine() -> (Engine, uuid::Uuid) {
        let id = uuid::Uuid::now_v7();
        let mut sheet = test_sheet();
        sheet.drawings.push(rect(id, 20.0));
        let mut engine = Engine::new(sheet).unwrap();
        engine.mark_saved();
        (engine, id)
    }

    fn right(engine: &Engine) -> f64 {
        match &engine.document().drawings[0] {
            SchDrawing::Rect { end, .. } => end.x,
            other => panic!("not a rectangle: {other:?}"),
        }
    }

    #[test]
    fn a_preview_is_in_the_document_and_not_in_the_history() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine.preview_drawing(rect(id, 40.0)).unwrap();
        assert_eq!(right(&engine), 40.0);
        assert!(!engine.can_undo());
        assert!(engine.is_at_save_point());
    }

    #[test]
    fn a_commit_is_one_step_back_to_the_value_before_the_first_keystroke() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine.preview_drawing(rect(id, 40.0)).unwrap();
        assert!(engine.commit_live_edit().is_some());
        assert_eq!(engine.history.undo_len(), 1);
        engine.undo().unwrap();
        assert_eq!(right(&engine), 20.0);
        assert!(engine.is_at_save_point());
    }

    #[test]
    fn a_cancel_restores_the_sheet_and_records_nothing() {
        let (mut engine, id) = engine();
        let revision = engine.revision();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        assert!(engine.cancel_live_edit().is_some());
        assert_eq!(right(&engine), 20.0);
        assert!(!engine.can_undo());
        assert_ne!(engine.revision(), revision, "a cache must repaint");
    }

    #[test]
    fn typing_the_old_value_back_records_nothing() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine.preview_drawing(rect(id, 20.0)).unwrap();
        assert!(engine.commit_live_edit().is_none());
        assert!(!engine.can_undo());
    }

    /// The next edit's pre-image holds the preview, so the preview must be
    /// the step under it: two undos walk back through both.
    #[test]
    fn another_edit_commits_the_preview_as_its_own_step_first() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine
            .execute(Command::PlaceNoConnect {
                no_connect: NoConnect {
                    uuid: uuid::Uuid::now_v7(),
                    position: Point::new(0.0, 0.0),
                },
            })
            .unwrap();
        assert_eq!(engine.live_edit_drawing(), None);
        assert_eq!(engine.history.undo_len(), 2);
        engine.undo().unwrap();
        assert_eq!(right(&engine), 30.0);
        assert!(engine.document().no_connects.is_empty());
        engine.undo().unwrap();
        assert_eq!(right(&engine), 20.0);
    }

    /// The edit puts the saved value back, but its pre-image holds the
    /// preview: the preview stays a step, so undo reaches the saved value.
    #[test]
    fn an_edit_restoring_the_old_value_keeps_the_preview_as_a_step() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine
            .execute(Command::UpdateSchDrawing {
                drawing: rect(id, 20.0),
            })
            .unwrap();
        assert_eq!(engine.history.undo_len(), 2);
        engine.undo().unwrap();
        assert_eq!(right(&engine), 30.0);
        assert!(!engine.is_at_save_point());
        engine.undo().unwrap();
        assert_eq!(right(&engine), 20.0);
        assert!(engine.is_at_save_point());
    }

    #[test]
    fn undo_takes_back_the_open_preview_and_redo_returns_it() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine.undo().unwrap();
        assert_eq!(right(&engine), 20.0);
        engine.redo().unwrap();
        assert_eq!(right(&engine), 30.0);
    }

    #[test]
    fn a_save_point_taken_with_a_preview_open_includes_it() {
        let (mut engine, id) = engine();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        engine.mark_saved();
        assert_eq!(engine.live_edit_drawing(), None);
        assert!(engine.is_at_save_point());
        engine.undo().unwrap();
        assert!(!engine.is_at_save_point());
    }

    /// A recovery copy taken mid-typing holds the sheet without the preview.
    #[test]
    fn the_committed_serialization_leaves_out_an_open_preview() {
        let (mut engine, id) = engine();
        let before = engine.serialize_with_extents(|_| None).unwrap();
        engine.preview_drawing(rect(id, 30.0)).unwrap();
        assert_ne!(engine.serialize_with_extents(|_| None).unwrap(), before);
        assert_eq!(
            engine.serialize_committed_with_extents(|_| None).unwrap(),
            before
        );
        assert_eq!(right(&engine), 30.0, "the typing is undisturbed");
        assert!(engine.live_edit_drawing().is_some());
    }

    #[test]
    fn a_preview_is_refused_inside_a_transaction() {
        let (mut engine, id) = engine();
        engine.begin_transaction().unwrap();
        assert!(matches!(
            engine.preview_drawing(rect(id, 30.0)),
            Err(EngineError::TransactionOpen)
        ));
        engine.rollback_transaction();
        assert_eq!(right(&engine), 20.0);
    }
}
