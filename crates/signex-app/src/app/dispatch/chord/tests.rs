//! #1378 — the waiting-chord lifecycle, driven through the real input path
//! (`handle_key_input` -> `route_key` -> `update`), not the resolver alone.

use super::*;
use crate::keymap::{
    AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};
use iced::keyboard;

/// `F8` toggles auto-focus and `F8 F9` opens the net colour palette, so
/// `F8` is a complete shortcut that is also the prefix of a longer one.
fn app_with_prefix_chord() -> Signex {
    let bind = |command: &str, trigger: &str| ShortcutBinding {
        action: ShortcutBindingAction::Command(AppCommandId::new(command).unwrap()),
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
    };
    let profile = ShortcutProfile {
        id: "prefix".to_string(),
        name: "Prefix".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![
            bind("toggle_auto_focus", "F8"),
            bind("open_net_color_palette", "F8 F9"),
        ],
    };
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.active_keymap = CompiledKeymap::compile(&profile);
    app
}

fn press(app: &mut Signex, key: keyboard::Key, modifiers: keyboard::Modifiers) {
    let window = app.ui_state.main_window_id.expect("main window");
    let event = keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    };
    let _ = app.handle_key_input(window, event);
}

fn press_named(app: &mut Signex, named: keyboard::key::Named) {
    press(
        app,
        keyboard::Key::Named(named),
        keyboard::Modifiers::default(),
    );
}

fn wait_generation(app: &Signex) -> u64 {
    app.ui_state
        .keymap_chord_wait
        .expect("F8 is waiting on F8 F9")
        .generation
}

fn block_with_modal(app: &mut Signex) {
    app.document_state.error_notice = Some(crate::app::state::ErrorNotice::export("disk full"));
}

#[test]
fn a_complete_chord_waits_and_the_longer_chord_stays_reachable() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;

    press_named(&mut app, keyboard::key::Named::F8);
    assert_eq!(
        app.ui_state.auto_focus, before,
        "F8 waits, it does not fire"
    );
    assert_eq!(app.ui_state.keymap_pending_sequence.len(), 1);

    press_named(&mut app, keyboard::key::Named::F9);
    assert_eq!(
        app.ui_state.auto_focus, before,
        "the longer chord ran instead"
    );
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
    assert!(app.ui_state.keymap_chord_wait.is_none());
}

#[test]
fn a_waiting_chord_fires_on_a_key_that_does_not_continue_it() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;

    press_named(&mut app, keyboard::key::Named::F8);
    press_named(&mut app, keyboard::key::Named::F2);

    assert_ne!(app.ui_state.auto_focus, before);
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
}

#[test]
fn the_timeout_fires_the_waiting_command() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);
    let generation = wait_generation(&app);

    let _ = app.update(Message::Ui(UiMsg::KeymapChordTimedOut { generation }));

    assert_ne!(app.ui_state.auto_focus, before);
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
    assert!(app.ui_state.keymap_chord_wait.is_none());
}

#[test]
fn a_stale_timeout_does_nothing() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);
    let generation = wait_generation(&app);

    let _ = app.update(Message::Ui(UiMsg::KeymapChordTimedOut {
        generation: generation + 1,
    }));

    assert_eq!(app.ui_state.auto_focus, before);
    assert_eq!(
        app.ui_state.keymap_pending_sequence.len(),
        1,
        "still waiting"
    );
}

#[test]
fn escape_through_the_real_input_path_cancels_the_waiting_chord() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);
    let generation = wait_generation(&app);

    press_named(&mut app, keyboard::key::Named::Escape);

    assert!(app.ui_state.keymap_pending_sequence.is_empty());
    assert!(app.ui_state.keymap_chord_wait.is_none());
    // The stale tick and a later unrelated key must not resurrect it.
    let _ = app.update(Message::Ui(UiMsg::KeymapChordTimedOut { generation }));
    press_named(&mut app, keyboard::key::Named::F2);
    assert_eq!(app.ui_state.auto_focus, before, "the cancelled chord fired");
}

#[test]
fn a_key_no_keymap_consumer_owns_abandons_the_waiting_chord() {
    // F1 is claimed by the shortcuts sheet, never reaching the resolver.
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);

    press_named(&mut app, keyboard::key::Named::F1);

    assert!(app.ui_state.keymap_pending_sequence.is_empty());
    assert_eq!(app.ui_state.auto_focus, before);
}

#[test]
fn a_modal_opening_drops_the_waiting_chord_instead_of_firing_it() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);
    block_with_modal(&mut app);

    // A modified stroke is not dropped by the bare-key guard (#103); it
    // must still not release the held command behind the modal.
    press(
        &mut app,
        keyboard::Key::Character("q".into()),
        keyboard::Modifiers::CTRL,
    );

    assert_eq!(app.ui_state.auto_focus, before);
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
}

#[test]
fn the_timeout_does_not_fire_behind_a_blocking_modal() {
    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);
    let generation = wait_generation(&app);
    block_with_modal(&mut app);

    let _ = app.update(Message::Ui(UiMsg::KeymapChordTimedOut { generation }));

    assert_eq!(app.ui_state.auto_focus, before);
    assert!(app.ui_state.keymap_pending_sequence.is_empty());
}

#[test]
fn a_pointer_press_or_focus_change_cancels_the_waiting_chord() {
    let press = iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left));
    assert!(chord_cancelling_event(&press));
    assert!(chord_cancelling_event(&iced::Event::Window(
        iced::window::Event::Unfocused
    )));
    assert!(!chord_cancelling_event(&iced::Event::Mouse(
        iced::mouse::Event::CursorLeft
    )));

    let mut app = app_with_prefix_chord();
    let before = app.ui_state.auto_focus;
    press_named(&mut app, keyboard::key::Named::F8);

    let _ = app.update(Message::Ui(UiMsg::KeymapChordCancelled));
    press_named(&mut app, keyboard::key::Named::F2);

    assert!(app.ui_state.keymap_chord_wait.is_none());
    assert_eq!(app.ui_state.auto_focus, before, "the cancelled chord fired");
}
