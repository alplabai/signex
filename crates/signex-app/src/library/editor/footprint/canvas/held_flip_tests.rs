//! #2545: a held X flips the selected pad's layer once
//! (`docs/DECISIONS.md`, "Held keys"); a held Space keeps rotating it.

use super::FootprintCanvas;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;
use iced::Color;
use iced::keyboard::{self, Key, key::Code, key::Named, key::Physical};
use iced::widget::canvas;

/// What a press of `key` on `code` does with a pad selected: whether the
/// canvas captured it, and the footprint message it published.
fn pressed(key: Key, code: Code, repeat: bool) -> (bool, Option<FootprintEditorMsg>) {
    let mut state = FootprintEditorState::empty();
    state.selected_pad = Some(0);
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
    };
    let Some(action) = fpc.on_key_pressed(
        &key,
        Physical::Code(code),
        &keyboard::Modifiers::empty(),
        None,
        repeat,
    ) else {
        return (false, None);
    };
    let (message, _redraw, status) = action.into_inner();
    let msg = match message {
        Some(LibraryMessage::EditorEvent {
            msg: EditorMsg::Footprint(msg),
            ..
        }) => Some(msg),
        _ => None,
    };
    (status == iced::event::Status::Captured, msg)
}

#[test]
fn a_held_x_flips_the_pad_once_and_swallows_its_repeats() {
    let x = || Key::Character("x".into());
    assert!(matches!(
        pressed(x(), Code::KeyX, false),
        (true, Some(FootprintEditorMsg::ActiveBarFlipSelection))
    ));
    // Captured, so the app keymap does not act on the repeat either.
    assert!(matches!(pressed(x(), Code::KeyX, true), (true, None)));
}

#[test]
fn a_held_space_keeps_rotating_the_pad() {
    let space = || Key::Named(Named::Space);
    for repeat in [false, true] {
        assert!(matches!(
            pressed(space(), Code::Space, repeat),
            (true, Some(FootprintEditorMsg::ActiveBarRotateSelection))
        ));
    }
}
