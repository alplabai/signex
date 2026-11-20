//! Typed pad-form text the pad does not take (CX review 2026-11-19, mockup
//! row 125): the text stays in `numeric_buffers` so the field shows it with
//! a red outline and the reason, while the pad and the illustration keep
//! the last good value. A value that is taken drops the entry; undo and
//! redo drop them all (`FootprintDocument::restore`).

use crate::app::Signex;
use crate::library::editor::footprint::{
    EditorPad, NextPadDefaults, pad_field, pad_field_buffer_key, sketch_pad_field_buffer_key,
};
use signex_sketch::attr::PadAttr;
use signex_sketch::id::SketchEntityId;

/// What an optional-mm field's text asks for: `Some(None)` clears the
/// value, `Some(Some(v))` stores `v`, and `None` is not a number, so the
/// edit is refused and the stored value kept.
pub(super) fn optional_mm_edit(value: &str) -> Option<Option<f64>> {
    let text = value.trim();
    if text.is_empty() {
        return Some(None);
    }
    crate::numeric_field::parse_field_f64(text).map(Some)
}

/// The optional-mm pad fields whose only rule is "a number, or empty to
/// clear": the hole tolerances and the copper offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::handlers::dock::sch_library) enum PadMm {
    TolerancePlus,
    ToleranceMinus,
    OffsetX,
    OffsetY,
}

impl PadMm {
    /// The `pad_field` name its refused text is kept under; also its undo
    /// key, so keystrokes into one field share one undo step.
    fn field(self) -> &'static str {
        match self {
            PadMm::TolerancePlus => pad_field::HOLE_TOLERANCE_PLUS,
            PadMm::ToleranceMinus => pad_field::HOLE_TOLERANCE_MINUS,
            PadMm::OffsetX => pad_field::COPPER_OFFSET_X,
            PadMm::OffsetY => pad_field::COPPER_OFFSET_Y,
        }
    }

    fn set_pad(self, pad: &mut EditorPad, v: Option<f64>) {
        match self {
            PadMm::TolerancePlus => pad.hole_tolerance_plus_mm = v,
            PadMm::ToleranceMinus => pad.hole_tolerance_minus_mm = v,
            PadMm::OffsetX => pad.copper_offset_x_mm = v,
            PadMm::OffsetY => pad.copper_offset_y_mm = v,
        }
    }

    fn set_next(self, d: &mut NextPadDefaults, v: Option<f64>) {
        match self {
            PadMm::TolerancePlus => d.hole_tolerance_plus_mm = v,
            PadMm::ToleranceMinus => d.hole_tolerance_minus_mm = v,
            PadMm::OffsetX => d.copper_offset_x_mm = v,
            PadMm::OffsetY => d.copper_offset_y_mm = v,
        }
    }

    fn set_sketch_pad(self, attr: &mut PadAttr, v: Option<f64>) {
        match self {
            PadMm::TolerancePlus => attr.hole_tolerance_plus_mm = v,
            PadMm::ToleranceMinus => attr.hole_tolerance_minus_mm = v,
            PadMm::OffsetX => attr.copper_offset_x_mm = v,
            PadMm::OffsetY => attr.copper_offset_y_mm = v,
        }
    }
}

impl Signex {
    /// `which` of the selected pad `idx`, or of the next pad when `None`:
    /// empty clears it, a number is stored, other text is refused and kept.
    pub(in crate::app::handlers::dock::sch_library) fn fp_pad_mm_edited(
        &mut self,
        idx: Option<usize>,
        which: PadMm,
        value: &str,
    ) -> bool {
        let field = which.field();
        match idx {
            Some(idx) => self.set_selected_pad_optional_mm(idx, (field, field), value, |pad, v| {
                which.set_pad(pad, v)
            }),
            None => self.set_next_pad_optional_mm(field, value, |d, v| which.set_next(d, v)),
        }
    }

    /// `which` of the sketch pad `id`, with the same three outcomes; the
    /// refused text is kept under the sketch pad's own key.
    pub(in crate::app::handlers::dock::sch_library) fn fp_sketch_pad_mm_edited(
        &mut self,
        id: SketchEntityId,
        which: PadMm,
        value: &str,
    ) -> bool {
        let key = sketch_pad_field_buffer_key(id, which.field());
        let edit = optional_mm_edit(value);
        if let Some(editor) = self.active_footprint_editor_mut() {
            match edit {
                Some(_) => editor.state.numeric_buffers.remove(&key),
                None => editor.state.numeric_buffers.insert(key, value.to_owned()),
            };
        }
        match edit {
            Some(v) => self.with_selected_sketch_pad_field(id, Some(which.field()), |attr| {
                which.set_sketch_pad(attr, v)
            }),
            None => {
                self.refresh_panel_ctx();
                true
            }
        }
    }

    /// Keeps `refused` as the typed text of `field` on the selected pad
    /// `idx` (the next pad when `None`), or drops the entry when `refused`
    /// is `None` because the value was taken. View state: never dirties.
    pub(super) fn pad_field_typed(
        &mut self,
        idx: Option<usize>,
        field: &str,
        refused: Option<&str>,
    ) {
        let Some(editor) = self.active_footprint_editor_mut() else {
            return;
        };
        let key = pad_field_buffer_key(idx, field);
        match refused {
            Some(text) => {
                editor.state.numeric_buffers.insert(key, text.to_owned());
            }
            None => {
                editor.state.numeric_buffers.remove(&key);
            }
        }
    }

    /// Refuses `value` for `field`: the text is kept for the panel and the
    /// stored value is left alone.
    pub(super) fn pad_field_refused(&mut self, idx: Option<usize>, field: &str, value: &str) {
        self.pad_field_typed(idx, field, Some(value));
        self.refresh_panel_ctx();
    }
}

#[cfg(test)]
mod tests {
    use super::optional_mm_edit;

    #[test]
    fn an_empty_field_clears_a_number_is_kept_and_text_is_refused() {
        assert_eq!(optional_mm_edit("  "), Some(None));
        assert_eq!(optional_mm_edit("-0.05"), Some(Some(-0.05)));
        assert_eq!(optional_mm_edit("-"), None);
        assert_eq!(optional_mm_edit("1,5"), None);
    }
}
