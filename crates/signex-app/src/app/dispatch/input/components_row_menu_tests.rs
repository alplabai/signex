//! The key that places from an open Components row menu is the key the menu
//! shows beside Place: the active profile's binding of `placement_accept`
//! (docs/DECISIONS.md, 2026-11-27, #2631).

use super::*;
use crate::keymap::{
    AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet, ShortcutTrigger,
};
use crate::library::LibraryMessage;
use crate::library::state::{ComponentsPanelRowKey, ComponentsRowAction, ComponentsRowMenu};
use crate::panels::components_panel::ROW_PLACE_COMMAND;
use keyboard::key::Named;

fn app_with_open_menu() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.library.components_panel.row_menu = Some(ComponentsRowMenu {
        key: ComponentsPanelRowKey {
            library_path: "lib.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(1)),
        },
        x: 0.0,
        y: 0.0,
        window: None,
    });
    app
}

fn press(key: keyboard::Key) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    }
}

fn places(app: &Signex, key: keyboard::Key) -> bool {
    let main = app.ui_state.main_window_id.expect("main window");
    matches!(
        app.route_key(main, &press(key)),
        Some(Message::Library(
            LibraryMessage::ComponentsPanelRowMenuPicked(ComponentsRowAction::Place)
        ))
    )
}

fn hint(app: &Signex) -> String {
    app.ui_state
        .active_keymap
        .shortcut_label_for_id(ROW_PLACE_COMMAND)
}

fn use_built_in(app: &mut Signex, id: &str) {
    let mut set = ShortcutProfileSet::built_ins().expect("built-ins parse");
    set.set_active_profile(id).expect("built-in exists");
    app.ui_state.active_keymap = std::sync::Arc::new(set.compile_active());
}

/// The standard profile binds Enter: the menu shows ENTER (named keys
/// print in capitals, as on key caps: keys.ftl) and Enter places.
#[test]
fn the_standard_profile_shows_and_takes_enter() {
    let mut app = app_with_open_menu();
    use_built_in(&mut app, "signex_standard");
    assert_eq!(hint(&app), "ENTER");
    assert!(places(&app, keyboard::Key::Named(Named::Enter)));
}

/// The classic profile binds no key: the menu shows none and Enter is not
/// taken from the keymap (old: Enter placed while no key was shown).
#[test]
fn a_profile_with_no_place_key_shows_none_and_takes_none() {
    let mut app = app_with_open_menu();
    use_built_in(&mut app, "classic");
    assert_eq!(hint(&app), "");
    assert!(!places(&app, keyboard::Key::Named(Named::Enter)));
}

/// A rebound key is both the hint and the key that places; Enter, no
/// longer bound, does not place (old: Enter placed, the hint said F7).
#[test]
fn a_rebound_place_key_is_the_one_shown_and_taken() {
    let mut app = app_with_open_menu();
    let profile = ShortcutProfile {
        id: "rebound".to_string(),
        name: "Rebound".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![ShortcutBinding {
            action: ShortcutBindingAction::Command(AppCommandId::new(ROW_PLACE_COMMAND).unwrap()),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::parse("F7").unwrap()],
            platform: None,
        }],
    };
    app.ui_state.active_keymap = std::sync::Arc::new(CompiledKeymap::compile(&profile));
    assert_eq!(hint(&app), "F7");
    assert!(places(&app, keyboard::Key::Named(Named::F7)));
    assert!(!places(&app, keyboard::Key::Named(Named::Enter)));
}
