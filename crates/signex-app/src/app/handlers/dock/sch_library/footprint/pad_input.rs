//! Typed pad-form text the pad does not take (CX review 2026-11-19): the
//! text stays in `numeric_buffers` so the field shows it with a red outline
//! and the reason, while the pad and the illustration keep the last good
//! value. A value that is taken drops the entry.

use crate::app::Signex;
use crate::library::editor::footprint::state::pad_field_buffer_key;

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

impl Signex {
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
