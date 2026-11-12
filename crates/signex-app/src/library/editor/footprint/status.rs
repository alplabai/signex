//! The footprint editor's segment of the global status bar: the selected
//! pad's number, size and position, or the pad count and the key hints.
//! X / Y and zoom come from the editor through the status readout.

use signex_i18n::{I18n, ids, t};

use crate::app::FootprintDocument;
use crate::status_bar::Activity;

/// The status bar segment for `editor`.
pub(crate) fn segment(editor: &FootprintDocument, i18n: &I18n) -> Activity {
    Activity::note(summary(editor, i18n))
}

/// "Pad 1 — 1.55 × 0.60 mm @ (-2.700, +1.905)" for the primary selected
/// pad, else "8 pads · Click empty area to add, …".
fn summary(editor: &FootprintDocument, i18n: &I18n) -> String {
    let state = &editor.state;
    if let Some(pad) = state.selected_pad.and_then(|idx| state.pads.get(idx)) {
        return t!(
            i18n,
            ids::EDITORS_FOOTPRINT_STATUS_PAD_SELECTED,
            "number" => pad.number.as_str(),
            "width" => format!("{:.2}", pad.size_mm.0),
            "height" => format!("{:.2}", pad.size_mm.1),
            "x" => format!("{:+.3}", pad.position_mm.0),
            "y" => format!("{:+.3}", pad.position_mm.1)
        )
        .into_string();
    }
    t!(
        i18n,
        ids::EDITORS_FOOTPRINT_STATUS_HINT_IDLE,
        "pads" => state.pads.len()
    )
    .into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::footprint::state::EditorPad;
    use std::path::PathBuf;

    fn editor_with_pads(count: usize) -> FootprintDocument {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        let mut editor = FootprintDocument::new(PathBuf::from("f.snxfpt"), file);
        for n in 1..=count {
            let mut pad = EditorPad::new_default(n.to_string(), (-2.7, 1.905));
            pad.size_mm = (1.55, 0.6);
            editor.state.pads.push(pad);
        }
        editor
    }

    fn i18n() -> std::sync::Arc<I18n> {
        crate::app::Signex::new().0.ui_state.i18n.clone()
    }

    #[test]
    fn with_nothing_selected_the_segment_counts_the_pads_and_hints_at_the_keys() {
        let editor = editor_with_pads(8);
        let i18n = i18n();
        let shown = segment(&editor, &i18n);
        assert!(!shown.is_error);
        let expected = t!(i18n, ids::EDITORS_FOOTPRINT_STATUS_HINT_IDLE, "pads" => 8usize);
        assert_eq!(shown.text, expected.into_string());
    }

    #[test]
    fn a_selected_pad_is_named_with_its_size_and_signed_position() {
        let mut editor = editor_with_pads(2);
        editor.state.selected_pad = Some(0);
        let shown = segment(&editor, &i18n()).text;
        for part in ["1", "1.55", "0.60", "-2.700", "+1.905"] {
            assert!(shown.contains(part), "{part} missing from {shown}");
        }
    }

    #[test]
    fn a_stale_selection_index_falls_back_to_the_pad_count() {
        let mut editor = editor_with_pads(1);
        editor.state.selected_pad = Some(5);
        let i18n = i18n();
        let expected = t!(i18n, ids::EDITORS_FOOTPRINT_STATUS_HINT_IDLE, "pads" => 1usize);
        assert_eq!(segment(&editor, &i18n).text, expected.into_string());
    }
}
