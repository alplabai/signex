//! The words the sketch canvas draws, rendered in the app's language.
//!
//! A canvas `Program` has no `I18n`, so the app renders these once per
//! language into the editor state (`app/runtime/sketch_canvas_text.rs`) and
//! the draw pass reads plain strings. Fluent's bidi isolation marks are
//! stripped when they are rendered: canvas text is drawn and measured by
//! character, never shaped as mixed-script text.

/// Stands for the count in [`SketchCanvasText::conflict_badge_many`]; a
/// private-use character no translation contains.
pub const COUNT_SLOT: char = '\u{E000}';

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

    /// The conflict badge for one conflict.
    pub conflict_badge_one: String,
    /// The conflict badge for several, with [`COUNT_SLOT`] where the count
    /// goes.
    pub conflict_badge_many: String,

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
}

impl SketchCanvasText {
    /// Whether these words were rendered for `locale`.
    pub fn is_rendered_for(&self, locale: &str) -> bool {
        self.locale == Some(locale)
    }

    /// The badge naming `count` conflicts.
    pub fn conflict_badge(&self, count: usize) -> String {
        if count == 1 {
            self.conflict_badge_one.clone()
        } else {
            self.conflict_badge_many
                .replace(COUNT_SLOT, &count.to_string())
        }
    }
}
