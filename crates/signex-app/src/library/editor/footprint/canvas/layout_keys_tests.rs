//! #2095: the footprint canvas clipboard keys match on any layout through
//! `keymap::shortcut_char`; Russian Ctrl+`с` (the C key) copies the pad.

use super::FootprintCanvas;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;
use iced::Color;
use iced::keyboard::{self, Key, key::Code, key::Physical};
use iced::widget::canvas;

/// The footprint editor message the canvas publishes for `logical` typed
/// on physical `code` with the platform command key held.
fn published(logical: &str, code: Code) -> Option<FootprintEditorMsg> {
    let state = FootprintEditorState::empty();
    let cache = canvas::Cache::new();
    let fpc = FootprintCanvas {
        state: &state,
        address: EditorAddress::new(
            std::path::PathBuf::from("/tmp/lib.snxlib"),
            "footprints".to_string(),
            signex_library::RowId::new(),
        ),
        bg_color: Color::WHITE,
        grid_color: Color::BLACK,
        cache: &cache,
        sketch: None,
        silk_f: &[],
        silk_b: &[],
        generated_silk: &[],
        zoom_step: 1.1,
        trackpad_pan: false,
        wheel_mode: Default::default(),
        mirror_card: None,
    };
    let action = fpc.on_key_pressed(
        &Key::Character(logical.into()),
        Physical::Code(code),
        &keyboard::Modifiers::COMMAND,
        None,
    )?;
    match action.into_inner().0? {
        LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(msg),
            ..
        } => Some(msg),
        _ => None,
    }
}

#[test]
fn russian_clipboard_keys_cut_copy_and_paste() {
    assert!(matches!(
        published("\u{447}", Code::KeyX),
        Some(FootprintEditorMsg::CutPad)
    ));
    assert!(matches!(
        published("\u{441}", Code::KeyC),
        Some(FootprintEditorMsg::CopyPad)
    ));
    assert!(matches!(
        published("\u{43c}", Code::KeyV),
        Some(FootprintEditorMsg::PastePad)
    ));
}

#[test]
fn a_latin_letter_keeps_its_logical_meaning() {
    // Dvorak types `c` on the physical I key and `j` on the C key.
    assert!(matches!(
        published("c", Code::KeyI),
        Some(FootprintEditorMsg::CopyPad)
    ));
    assert!(published("j", Code::KeyC).is_none());
}
