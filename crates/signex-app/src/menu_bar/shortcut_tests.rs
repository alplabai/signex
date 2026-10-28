use super::{MenuContext, shortcut_for};
use crate::keymap::{
    AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};

fn context_with(bindings: Vec<ShortcutBinding>) -> MenuContext {
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
fn a_command_the_keymap_does_not_mention_keeps_its_fallback() {
    let ctx = context_with(vec![]);
    assert_eq!(
        shortcut_for(&ctx, "save_document", "Ctrl+S").as_deref(),
        Some("Ctrl+S")
    );
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
