//! The board rows of Snap ▾ (#2518, Graphite mockup #179): on a board
//! sketch the menu gains a Board features group, one switch for all of
//! them and one per kind, over [`SnapOptions::board_features`]. A
//! footprint sketch has no board and shows the menu unchanged.
//!
//! [`SnapOptions::board_features`]: super::state::SnapOptions::board_features

use std::path::PathBuf;

use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::sketch::SketchHost;
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};

use super::snap_board::BoardFeatureKind;
use super::state::FootprintEditorState;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// A click on a board row of Snap ▾.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchSnapMsg {
    /// The Snap to board features switch was flipped.
    BoardFeaturesToggled,
    /// One kind's switch was flipped.
    BoardFeatureKindToggled(BoardFeatureKind),
}

/// Flip the switch the row names. The menu stays open, like the other
/// Snap ▾ switches; nothing on the board or in the sketch changes.
pub(crate) fn apply(editor: &mut crate::app::FootprintDocument, msg: SketchSnapMsg) {
    let snaps = &mut editor.state.snap_options.board_features;
    match msg {
        SketchSnapMsg::BoardFeaturesToggled => snaps.enabled = !snaps.enabled,
        SketchSnapMsg::BoardFeatureKindToggled(kind) => snaps.toggle(kind),
    }
    editor.canvas_cache.clear();
}

/// `entries` (the Snap ▾ rows both hosts share) with the board rows
/// after them on a board sketch.
pub fn with_board_rows(
    mut entries: Vec<DropdownEntry<LibraryMessage>>,
    state: &FootprintEditorState,
    path: PathBuf,
    i18n: &I18n,
) -> Vec<DropdownEntry<LibraryMessage>> {
    if state.sketch_host() == SketchHost::Board {
        entries.push(DropdownEntry::Separator);
        entries.extend(board_rows(state, path, i18n));
    }
    entries
}

/// The label of each kind's row.
fn kind_label(kind: BoardFeatureKind) -> MessageId {
    match kind {
        BoardFeatureKind::Outline => ids::SKETCH_SNAP_BOARD_OUTLINE,
        BoardFeatureKind::Pad => ids::SKETCH_SNAP_BOARD_PADS,
        BoardFeatureKind::Hole => ids::SKETCH_SNAP_BOARD_HOLES,
        BoardFeatureKind::Via => ids::SKETCH_SNAP_BOARD_VIAS,
        BoardFeatureKind::TrackEnd => ids::SKETCH_SNAP_BOARD_TRACK_ENDS,
        BoardFeatureKind::Courtyard => ids::SKETCH_SNAP_BOARD_COURTYARDS,
    }
}

/// The Board features group: its heading, the switch for all and one row
/// per kind, greyed while the switch for all is off.
fn board_rows(
    state: &FootprintEditorState,
    path: PathBuf,
    i18n: &I18n,
) -> Vec<DropdownEntry<LibraryMessage>> {
    let snaps = state.snap_options.board_features;
    let event = |msg: SketchSnapMsg| LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSnap(msg)),
    };
    let mut rows = vec![
        DropdownEntry::Header(t!(i18n, ids::SKETCH_SNAP_BOARD_HEADER).into_string()),
        DropdownEntry::Item(
            DropdownItem::new(
                t!(i18n, ids::SKETCH_SNAP_BOARD_ALL).into_string(),
                event(SketchSnapMsg::BoardFeaturesToggled),
            )
            .checked(snaps.enabled),
        ),
    ];
    rows.extend(BoardFeatureKind::ALL.into_iter().map(|kind| {
        DropdownEntry::Item(
            DropdownItem::new(
                t!(i18n, kind_label(kind)).into_string(),
                event(SketchSnapMsg::BoardFeatureKindToggled(kind)),
            )
            .checked(snaps.kind_on(kind))
            .disabled(!snaps.enabled),
        )
    }));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::footprint::BoardBackdrop;
    use signex_library::Footprint;

    fn en_us() -> I18n {
        I18n::new(&signex_i18n::LanguagePref::Locale("en-US".into()))
    }

    fn strip(s: &str) -> String {
        s.replace(['\u{2068}', '\u{2069}'], "")
    }

    fn labels(entries: &[DropdownEntry<LibraryMessage>]) -> Vec<String> {
        entries
            .iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some(strip(&item.label)),
                DropdownEntry::Header(h) => Some(strip(h)),
                _ => None,
            })
            .collect()
    }

    fn board_state() -> FootprintEditorState {
        let mut state = FootprintEditorState::from_footprint(&Footprint::empty("t"));
        state.board_backdrop = Some(std::sync::Arc::new(BoardBackdrop::default()));
        state
    }

    #[test]
    fn a_footprint_sketch_keeps_the_snap_menu_as_it_was() {
        let state = FootprintEditorState::from_footprint(&Footprint::empty("t"));
        let shared = vec![DropdownEntry::Header("Snap layers".into())];
        let entries = with_board_rows(shared, &state, PathBuf::from("t"), &en_us());
        assert_eq!(labels(&entries), ["Snap layers"]);
    }

    #[test]
    fn a_board_sketch_adds_the_board_feature_rows() {
        let entries = with_board_rows(Vec::new(), &board_state(), PathBuf::from("t"), &en_us());
        assert_eq!(
            labels(&entries),
            [
                "Board features",
                "Snap to board features",
                "Board outline",
                "Pad centres",
                "Hole centres",
                "Via centres",
                "Track ends",
                "Courtyard corners",
            ]
        );
        let all_checked = entries.iter().all(|e| match e {
            DropdownEntry::Item(item) => item.checked && !item.disabled,
            _ => true,
        });
        assert!(all_checked, "board-feature snaps are on by default");
    }

    #[test]
    fn the_kind_rows_grey_out_while_board_features_are_off() {
        let mut state = board_state();
        state.snap_options.board_features.enabled = false;
        let entries = with_board_rows(Vec::new(), &state, PathBuf::from("t"), &en_us());
        let items: Vec<&DropdownItem<LibraryMessage>> = entries
            .iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some(item),
                _ => None,
            })
            .collect();
        assert!(
            !items[0].checked && !items[0].disabled,
            "the switch for all"
        );
        assert!(items[1..].iter().all(|item| item.disabled));
    }

    #[test]
    fn a_row_click_flips_its_switch() {
        let mut snaps = board_state().snap_options.board_features;
        snaps.toggle(BoardFeatureKind::Courtyard);
        assert!(!snaps.courtyards && snaps.pads);
        assert!(!snaps.allows(BoardFeatureKind::Courtyard));
        assert!(snaps.allows(BoardFeatureKind::Pad));
        snaps.enabled = false;
        assert!(
            !snaps.allows(BoardFeatureKind::Pad),
            "the switch for all wins"
        );
        assert!(
            snaps.kind_on(BoardFeatureKind::Pad),
            "the row keeps its own tick"
        );
    }
}
