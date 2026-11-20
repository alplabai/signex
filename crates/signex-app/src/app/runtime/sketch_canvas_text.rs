//! Render the sketch canvas's words into every footprint editor and the
//! board sketch session, in the app's language. A canvas cannot read the
//! `I18n`, so the words travel as plain strings in the editor state
//! (`state/canvas_text.rs`). Each editor is re-rendered only when its words
//! are missing or in another language, and its conflict badge only when the
//! conflict count changes, so the per-update refresh is a comparison.

use signex_i18n::{I18n, MessageId, ids};

use super::super::*;
use crate::library::editor::footprint::sketch_mode::value_edit::conflicting_constraints;
use crate::library::editor::footprint::state::canvas_text::SketchCanvasText;

/// `id` as plain canvas text: no bidi isolation marks.
fn word(i18n: &I18n, id: MessageId) -> String {
    i18n.message_plain(id, &[])
}

/// Every canvas word in `i18n`'s language.
pub(crate) fn render_sketch_canvas_text(i18n: &I18n) -> SketchCanvasText {
    let w = |id| word(i18n, id);
    SketchCanvasText {
        locale: Some(i18n.locale()),
        snap_coincident: w(ids::SKETCH_CANVAS_SNAP_COINCIDENT),
        snap_horizontal: w(ids::SKETCH_CANVAS_SNAP_HORIZONTAL),
        snap_vertical: w(ids::SKETCH_CANVAS_SNAP_VERTICAL),
        snap_angle: w(ids::SKETCH_CANVAS_SNAP_ANGLE),
        snap_intersection: w(ids::SKETCH_CANVAS_SNAP_INTERSECTION),
        snap_guide: w(ids::SKETCH_CANVAS_SNAP_GUIDE),
        snap_point: w(ids::SKETCH_CANVAS_SNAP_POINT),
        snap_pad: w(ids::SKETCH_CANVAS_SNAP_PAD),
        snap_board_outline: w(ids::SKETCH_CANVAS_SNAP_BOARD_OUTLINE),
        snap_board_pad: w(ids::SKETCH_CANVAS_SNAP_BOARD_PAD),
        snap_board_hole: w(ids::SKETCH_CANVAS_SNAP_BOARD_HOLE),
        snap_board_via: w(ids::SKETCH_CANVAS_SNAP_BOARD_VIA),
        snap_board_track: w(ids::SKETCH_CANVAS_SNAP_BOARD_TRACK),
        snap_board_courtyard: w(ids::SKETCH_CANVAS_SNAP_BOARD_COURTYARD),
        snap_midpoint: w(ids::SKETCH_CANVAS_SNAP_MIDPOINT),
        snap_centre: w(ids::SKETCH_CANVAS_SNAP_CENTRE),
        snap_line: w(ids::SKETCH_CANVAS_SNAP_LINE),
        snap_circle: w(ids::SKETCH_CANVAS_SNAP_CIRCLE),
        snap_arc: w(ids::SKETCH_CANVAS_SNAP_ARC),
        conflict_badge: String::new(),
        conflict_count: 0,
        unit_mm: w(ids::SKETCH_CANVAS_UNIT_MM),
        unit_deg: w(ids::SKETCH_CANVAS_UNIT_DEG),
        value_unresolved: w(ids::SKETCH_CANVAS_VALUE_UNRESOLVED),
        pattern_linear: w(ids::SKETCH_CANVAS_PATTERN_LINEAR),
        pattern_grid: w(ids::SKETCH_CANVAS_PATTERN_GRID),
        pattern_polar: w(ids::SKETCH_CANVAS_PATTERN_POLAR),
        pattern_count: w(ids::SKETCH_CANVAS_PATTERN_COUNT),
        pattern_columns: w(ids::SKETCH_CANVAS_PATTERN_COLUMNS),
        pattern_rows: w(ids::SKETCH_CANVAS_PATTERN_ROWS),
        pattern_spacing_x: w(ids::SKETCH_CANVAS_PATTERN_SPACING_X),
        pattern_spacing_y: w(ids::SKETCH_CANVAS_PATTERN_SPACING_Y),
        pattern_sweep: w(ids::SKETCH_CANVAS_PATTERN_SWEEP),
    }
}

/// Put the words in `i18n`'s language into `editor`, unless they already
/// are, and the conflict badge for its current conflict count. Returns
/// whether they changed.
pub(crate) fn refresh_canvas_text(editor: &mut FootprintDocument, i18n: &I18n) -> bool {
    let relanguaged = !editor.state.canvas_text.is_rendered_for(i18n.locale());
    if relanguaged {
        editor.state.canvas_text = render_sketch_canvas_text(i18n);
    }
    let count = conflicting_constraints(&editor.state).len();
    let recounted = editor.state.canvas_text.set_conflict_count(i18n, count);
    let changed = relanguaged || recounted;
    if changed {
        editor.canvas_cache.clear();
    }
    changed
}

impl Signex {
    /// Bring the canvas words of every footprint editor and of the board
    /// sketch session into the app's language. Runs on every update (from
    /// `sync_panel_ctx_mirrors`), so an editor opened or a language picked
    /// since the last update draws in the right words.
    pub(in crate::app) fn sync_sketch_canvas_text(&mut self) {
        let i18n = std::sync::Arc::clone(&self.ui_state.i18n);
        for editor in self.document_state.footprint_editors.values_mut() {
            refresh_canvas_text(editor, &i18n);
        }
        if let Some(session) = self.interaction_state.board_sketch.as_mut() {
            refresh_canvas_text(&mut session.editor, &i18n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::LanguagePref;

    fn en_us() -> I18n {
        I18n::new(&LanguagePref::Locale("en-US".into()))
    }

    fn has_isolation_marks(s: &str) -> bool {
        s.contains(['\u{2068}', '\u{2069}'])
    }

    #[test]
    fn canvas_text_strips_isolation_marks() {
        let text = render_sketch_canvas_text(&en_us());
        for word in [
            &text.snap_coincident,
            &text.snap_board_courtyard,
            &text.unit_mm,
        ] {
            assert!(!has_isolation_marks(word), "{word:?}");
        }
        let mut text = text;
        for (count, badge) in [
            (1, "1 conflict · see Properties"),
            (3, "3 conflicts · see Properties"),
        ] {
            assert!(text.set_conflict_count(&en_us(), count));
            assert_eq!(text.conflict_badge, badge);
            assert!(!has_isolation_marks(&text.conflict_badge));
        }
    }

    #[test]
    fn the_snap_words_the_snap_badge_reads_are_rendered() {
        let text = render_sketch_canvas_text(&en_us());
        assert_eq!(text.snap_coincident, "Coincident");
        assert_eq!(text.snap_horizontal, "Horizontal");
        assert_eq!(text.snap_board_outline, "Board outline");
        assert_eq!(text.locale, Some("en-US"));
    }

    #[test]
    fn an_editor_is_rendered_once_per_language() {
        let mut editor = FootprintDocument::new(
            std::path::PathBuf::from("words.snxfpt"),
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty(
                "words",
            )),
        );
        let i18n = en_us();
        assert!(refresh_canvas_text(&mut editor, &i18n), "first render");
        assert!(!refresh_canvas_text(&mut editor, &i18n), "same language");
        assert_eq!(editor.state.canvas_text.unit_mm, "mm");
    }
}
