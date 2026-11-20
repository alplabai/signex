//! Footprint selection-filter preset apply/capture helpers.
//!
//! Task 6 — parallel to the schematic's `CustomFilterPreset` flow
//! (`crate::app::handlers::active_bar::filter_controls`), but scoped
//! to the footprint editor's `SelectionFilterKind` categories and
//! backed by `FootprintFilterPreset` (Task 5).

use crate::active_bar::FootprintFilterPreset;

use super::state::FootprintEditorState;

/// Replace the editor's active selection filter with exactly the
/// preset's kinds (everything else is switched off).
pub fn apply_preset(state: &mut FootprintEditorState, preset: &FootprintFilterPreset) {
    state.selection_filter.apply_kinds(&preset.kinds);
}

/// Snapshot the editor's currently-enabled filter kinds into a new
/// named preset, ready to be appended to the persisted list.
pub fn capture_preset(state: &FootprintEditorState, name: String) -> FootprintFilterPreset {
    FootprintFilterPreset {
        name,
        kinds: state.selection_filter.enabled_kinds(),
    }
}

/// The saved preset `preset` would only repeat: one with the same kinds,
/// under any name when no name was `typed` (an automatic "Filter N" adds
/// nothing), or under the same name when one was.
pub fn existing_twin<'a>(
    presets: &'a [FootprintFilterPreset],
    preset: &FootprintFilterPreset,
    typed: bool,
) -> Option<&'a FootprintFilterPreset> {
    let same_kinds = |other: &FootprintFilterPreset| {
        other.kinds.len() == preset.kinds.len()
            && other.kinds.iter().all(|kind| preset.kinds.contains(kind))
    };
    presets
        .iter()
        .find(|other| same_kinds(other) && (!typed || other.name == preset.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::footprint::state::selection_filter::SelectionFilterKind as Kind;

    fn preset(name: &str, kinds: &[Kind]) -> FootprintFilterPreset {
        FootprintFilterPreset {
            name: name.to_owned(),
            kinds: kinds.to_vec(),
        }
    }

    #[test]
    fn a_repeat_of_saved_kinds_is_a_twin() {
        let saved = [preset("Pads only", &[Kind::Pads, Kind::Tracks])];
        let untyped = preset("Filter 2", &[Kind::Tracks, Kind::Pads]);
        assert_eq!(
            existing_twin(&saved, &untyped, false).map(|p| p.name.as_str()),
            Some("Pads only"),
            "kinds in another order are the same set"
        );
        let same_name = preset("Pads only", &[Kind::Pads, Kind::Tracks]);
        assert!(existing_twin(&saved, &same_name, true).is_some());
    }

    #[test]
    fn new_kinds_or_a_new_typed_name_is_no_twin() {
        let saved = [preset("Pads only", &[Kind::Pads])];
        assert!(existing_twin(&saved, &preset("Filter 2", &[Kind::Tracks]), false).is_none());
        assert!(existing_twin(&saved, &preset("Mine", &[Kind::Pads]), true).is_none());
    }
}
