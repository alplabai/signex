//! The words the sketch canvas draws, rendered in the app's language.
//!
//! A canvas `Program` has no `I18n`, so the app renders these once per
//! language into the editor state (`app/runtime/sketch_canvas_text.rs`) and
//! the draw pass reads plain strings. Fluent's bidi isolation marks are
//! stripped when they are rendered: canvas text is drawn and measured by
//! character, never shaped as mixed-script text.

use signex_i18n::{Arg, I18n, ids};

/// The rendered canvas words. Empty until the app first renders them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchCanvasText {
    /// The message locale these were rendered in; `None` before the first
    /// render. The app re-renders when its locale differs.
    pub locale: Option<&'static str>,

    // Snap badges at the cursor (read by the snap glyph).
    pub snap_coincident: String,
    pub snap_horizontal: String,
    pub snap_vertical: String,
    pub snap_angle: String,
    pub snap_intersection: String,
    pub snap_guide: String,
    pub snap_point: String,
    pub snap_pad: String,
    pub snap_board_outline: String,
    pub snap_board_pad: String,
    pub snap_board_hole: String,
    pub snap_board_via: String,
    pub snap_board_track: String,
    pub snap_board_courtyard: String,

    /// The conflict badge, rendered from the real count so the catalog picks
    /// the plural form; empty when nothing is in conflict.
    pub conflict_badge: String,
    /// The conflict count `conflict_badge` was rendered for.
    pub conflict_count: usize,

    /// Unit after a length on the drawing ("mm").
    pub unit_mm: String,
    /// Sign after an angle on the drawing ("°").
    pub unit_deg: String,
    /// Shown for a value that does not evaluate.
    pub value_unresolved: String,

    /// Pattern glyph letters: linear, grid (rectangular), polar (circular).
    pub pattern_linear: String,
    pub pattern_grid: String,
    pub pattern_polar: String,
    /// Names of a selected pattern's values on the drawing.
    pub pattern_count: String,
    pub pattern_columns: String,
    pub pattern_rows: String,
    pub pattern_spacing_x: String,
    pub pattern_spacing_y: String,
    pub pattern_sweep: String,
    /// The mark before a mirrored pattern's glyph ("⇔").
    pub pattern_mirror: String,
    /// Joins a pattern's glyph to the pattern it repeats ("of").
    pub pattern_of: String,
}

impl SketchCanvasText {
    /// Whether these words were rendered for `locale`.
    pub fn is_rendered_for(&self, locale: &str) -> bool {
        self.locale == Some(locale)
    }

    /// Render the badge for `count` conflicts in `i18n`'s language, unless
    /// it already names that count. Returns whether it changed.
    pub fn set_conflict_count(&mut self, i18n: &I18n, count: usize) -> bool {
        if self.conflict_count == count && (count == 0) == self.conflict_badge.is_empty() {
            return false;
        }
        self.conflict_count = count;
        self.conflict_badge = if count == 0 {
            String::new()
        } else {
            let count = u32::try_from(count).unwrap_or(u32::MAX);
            i18n.message_plain(
                ids::SKETCH_CANVAS_CONFLICT_BADGE,
                &[("count", Arg::from(count))],
            )
        };
        true
    }
}
