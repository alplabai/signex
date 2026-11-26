//! A refused board sketch commit (mockup row 110): the error toast at the
//! canvas's bottom left, Messages, and for a profile that is not closed the
//! place it is open, as a link that frames it on the canvas. The board and
//! the copy are unchanged. See `docs/PCB_SKETCH.md`, "The way out".

use signex_bake::BoardBakeError;
use signex_i18n::{MessageId, RenderTranslatable as _, Translatable, ids};
use signex_sketch::baked_kinds::BakedKind;

use crate::app::Signex;
use crate::app::state::{BoardSketchRefusalToast, BoardSketchSession, ErrorNotice, RefusedGap};
use crate::library::editor::footprint::format_check_mm as mm;

impl Signex {
    /// The board did not take the sketch, for `reason`.
    pub(super) fn refuse_board_sketch(&mut self, reason: String) {
        let sentence = Translatable::new(ids::BOARD_SKETCH_REFUSED).arg("reason", reason);
        let toast = BoardSketchRefusalToast {
            sentence: sentence.clone(),
            gap: None,
        };
        self.refuse_board_sketch_with(sentence, toast);
    }

    /// The bake refused the sketch. A profile that is not closed is named by
    /// where it is open, as Outline checks names it, not by an edge id.
    pub(super) fn refuse_board_sketch_bake(&mut self, error: &BoardBakeError) {
        let open = self
            .interaction_state
            .board_sketch
            .as_ref()
            .and_then(|session| open_profile_refusal(session, error));
        match open {
            Some((detail, toast)) => self.refuse_board_sketch_with(detail, toast),
            None => self.refuse_board_sketch(error.to_string()),
        }
    }

    /// Say `detail` in Messages and keep it for a held save's card. The
    /// toast shows while the board is in view; a refusal of a board not in
    /// view (leaving it to sketch another) goes on the notice card instead,
    /// as no toast of it would show.
    fn refuse_board_sketch_with(&mut self, detail: Translatable, toast: BoardSketchRefusalToast) {
        // Messages stays en-US whatever the UI language.
        crate::diagnostics::log_warning(detail.render_en_us());
        let in_view = self.board_sketch_in_view().is_some();
        if !in_view {
            let notice = ErrorNotice::new(ids::BOARD_SKETCH_TITLE, detail.clone());
            self.document_state.error_notice = Some(notice);
        }
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.refusal = Some(detail);
            session.toast = in_view.then_some(toast);
        }
    }

    /// The toast's place was clicked: the sketch canvas frames the gap on
    /// its next event, as Fit does the content.
    pub(super) fn frame_board_sketch_refusal_gap(&mut self) {
        let Some(session) = self.board_sketch_in_view_mut() else {
            return;
        };
        let Some(gap) = session.toast.as_ref().and_then(|toast| toast.gap) else {
            return;
        };
        let state = &mut session.editor.state;
        state.fit_target_mm = Some(gap.frame());
        state.fit_pending = true;
        session.editor.canvas_cache.clear();
    }

    /// The toast was dismissed; the refusal stays for a held save's card.
    pub(super) fn dismiss_board_sketch_refusal(&mut self) {
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            session.toast = None;
        }
    }
}

/// The Messages line and the toast of a bake that refused an open profile,
/// placed where the copy is open through the edge the error names. `None`
/// for any other refusal, or when no open end is found on that edge.
fn open_profile_refusal(
    session: &BoardSketchSession,
    error: &BoardBakeError,
) -> Option<(Translatable, BoardSketchRefusalToast)> {
    let BoardBakeError::OpenProfile { kind, edge } = error else {
        return None;
    };
    let sketch = session.working_sketch();
    let end = session.editor.state.open_end_through(&sketch, *edge)?;
    let at = end.at?;
    let outline = matches!(kind, BakedKind::BoardOutline);
    let (line, sentence) = open_ids(outline);
    let toast = BoardSketchRefusalToast {
        sentence: Translatable::new(sentence),
        gap: Some(RefusedGap {
            at,
            gap_mm: end.gap_mm,
        }),
    };
    Some((placed(line, at.x, at.y), toast))
}

/// `(Messages line, toast sentence)` ids for the outline or another profile.
fn open_ids(outline: bool) -> (MessageId, MessageId) {
    if outline {
        (
            ids::BOARD_SKETCH_REFUSED_OUTLINE_OPEN,
            ids::BOARD_SKETCH_REFUSED_OUTLINE_NOT_CLOSED,
        )
    } else {
        (
            ids::BOARD_SKETCH_REFUSED_PROFILE_OPEN,
            ids::BOARD_SKETCH_REFUSED_PROFILE_NOT_CLOSED,
        )
    }
}

/// `id` with a place `x`, `y` mm, two decimals as Outline checks gives it.
fn placed(id: MessageId, x: f64, y: f64) -> Translatable {
    Translatable::new(id).arg("x", mm(x, 2)).arg("y", mm(y, 2))
}
