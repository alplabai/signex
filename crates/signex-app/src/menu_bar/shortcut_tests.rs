use super::{MenuContext, shortcut_for};
use crate::keymap::{
    AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};

fn context_with(bindings: Vec<ShortcutBinding>) -> MenuContext<'static> {
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings,
    };
    MenuContext {
        active_keymap: Some(std::sync::Arc::new(CompiledKeymap::compile(&profile))),
        ..MenuContext::default()
    }
}

fn binding(action: ShortcutBindingAction, trigger: &str) -> ShortcutBinding {
    ShortcutBinding {
        action,
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
    }
}

// #1378 — the fallback label is for commands the keymap is silent about,
// not for ones the user unbound.
#[test]
fn an_unbound_command_shows_no_shortcut_not_the_hardcoded_fallback() {
    let save = AppCommandId::new("save_document").unwrap();
    let ctx = context_with(vec![
        binding(ShortcutBindingAction::Command(save.clone()), "Ctrl+S"),
        binding(ShortcutBindingAction::Unbind(save), "Ctrl+S"),
    ]);
    assert_eq!(shortcut_for(&ctx, "save_document", "Ctrl+S"), None);
}

#[test]
fn a_command_the_keymap_does_not_mention_shows_no_hint() {
    let ctx = context_with(vec![]);
    assert_eq!(shortcut_for(&ctx, "save_document", "Ctrl+S"), None);
}

#[test]
fn without_any_active_keymap_the_fallback_stands_in() {
    let ctx = MenuContext::default();
    assert_eq!(
        shortcut_for(&ctx, "save_document", "Ctrl+S").as_deref(),
        Some("Ctrl+S")
    );
}

// #1378 item 6: Classic binds P, G and F1 to other commands, so the
// Altium fallback hints would advertise keys that do something else.
#[test]
fn the_classic_profile_shows_no_altium_fallback_hints() {
    let mut set = crate::keymap::ShortcutProfileSet::built_ins().unwrap();
    set.set_active_profile("classic").unwrap();
    let ctx = MenuContext {
        active_keymap: Some(std::sync::Arc::new(set.compile_active())),
        ..MenuContext::default()
    };
    for (id, fallback) in [
        ("open_components_panel", "P"),
        ("cycle_snap_grid_forward", "G"),
        ("show_current_command_hotkeys", "F1"),
    ] {
        assert_eq!(shortcut_for(&ctx, id, fallback), None, "{id}");
    }
}

#[test]
fn a_bound_command_shows_the_keymap_label() {
    let save = AppCommandId::new("save_document").unwrap();
    let ctx = context_with(vec![binding(ShortcutBindingAction::Command(save), "F2")]);
    assert_eq!(
        shortcut_for(&ctx, "save_document", "Ctrl+S").as_deref(),
        Some("F2")
    );
}
