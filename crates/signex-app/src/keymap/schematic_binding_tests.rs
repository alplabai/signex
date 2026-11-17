//! #2416: the built-in profiles' sheet-only bindings. Every binding of the
//! `schematic` context runs a command that resolves and is enabled on a
//! sheet, and none of them clashes with a global binding: no global chord is
//! the same as, or a prefix of, a sheet chord, or the other way round, so a
//! sheet key never shadows a global one and never makes it wait.

use super::{
    AppCommandId, CatalogTextExt, KeyStroke, Platform, ShortcutBindingAction, ShortcutContext,
    ShortcutProfileSet, ShortcutTrigger,
};
use crate::app::command::CommandContext;

/// The facts of a window showing a sheet.
fn on_a_sheet() -> CommandContext {
    CommandContext {
        has_schematic: true,
        document_is_schematic: true,
        has_document: true,
        has_active_path: true,
        ..CommandContext::default()
    }
}

/// `(context, command, strokes)` of every key binding of every built-in
/// profile that applies on `platform`, the strokes resolved for it.
fn key_bindings(
    platform: Platform,
) -> Vec<(String, ShortcutContext, AppCommandId, Vec<KeyStroke>)> {
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let mut out = Vec::new();
    for profile in set.profiles() {
        for binding in &profile.bindings {
            if !binding.applies_on(platform) {
                continue;
            }
            let ShortcutBindingAction::Command(command) = &binding.action else {
                continue;
            };
            for trigger in &binding.triggers {
                let ShortcutTrigger::KeySequence(strokes) = trigger else {
                    continue;
                };
                let resolved = strokes
                    .iter()
                    .map(|stroke| stroke.resolved_for(platform))
                    .collect();
                out.push((
                    profile.id.clone(),
                    binding.context,
                    command.clone(),
                    resolved,
                ));
            }
        }
    }
    out
}

#[test]
fn every_schematic_binding_runs_a_command_enabled_on_a_sheet() {
    let sheet = on_a_sheet();
    let mut checked = 0;
    for (profile, context, command, _) in key_bindings(Platform::Windows) {
        if context != ShortcutContext::Schematic {
            continue;
        }
        assert!(
            crate::app::command::bridge::is_dispatchable(&command),
            "{profile}: `{}` does not resolve",
            command.as_str()
        );
        assert!(
            crate::app::command::is_enabled(&command, &sheet),
            "{profile}: `{}` is not enabled on a sheet",
            command.as_str()
        );
        checked += 1;
    }
    // The default profile's five sheet tools (#2416).
    assert!(checked >= 5, "only {checked} schematic bindings checked");
}

#[test]
fn no_schematic_binding_clashes_with_a_global_one() {
    for platform in [Platform::Windows, Platform::Linux, Platform::MacOs] {
        let bindings = key_bindings(platform);
        for (profile, context, command, strokes) in &bindings {
            if *context != ShortcutContext::Schematic {
                continue;
            }
            for (other_profile, other_context, other, other_strokes) in &bindings {
                if other_profile != profile || *other_context != ShortcutContext::Global {
                    continue;
                }
                let clashes =
                    strokes.starts_with(other_strokes) || other_strokes.starts_with(strokes);
                assert!(
                    !clashes,
                    "{profile} on {platform:?}: the sheet binding of `{}` clashes with the \
                     global binding of `{}`",
                    command.as_str(),
                    other.as_str()
                );
            }
        }
    }
}

/// The tools #2416 gave a key in both shipped profiles.
const COMMON_TOOLS: [&str; 8] = [
    "place_port",
    "place_off_sheet_connector",
    "place_sheet_symbol",
    "place_bus_entry",
    "open_net_inspector",
    "cross_probe",
    "find_similar_objects",
    "place_no_connect",
];

/// The Keyboard Shortcuts sheet lists each of them in both profiles.
#[test]
fn the_shortcuts_sheet_lists_the_common_tools_in_both_profiles() {
    let i18n = signex_i18n::I18n::en_us_ref();
    let mut set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let ids: Vec<String> = set.profiles().map(|profile| profile.id.clone()).collect();
    for profile in ids {
        set.set_active_profile(profile.clone())
            .expect("a built-in profile");
        let labels: Vec<String> = crate::keyboard_shortcuts_modal::build_sections(i18n, &set)
            .into_iter()
            .flat_map(|section| section.rows)
            .map(|(label, _)| label)
            .collect();
        for id in COMMON_TOOLS {
            let label = super::metadata_for_str(id)
                .expect("a catalog command")
                .label_in(i18n);
            assert!(
                labels.contains(&label),
                "{profile}: the shortcuts sheet does not list {label}"
            );
        }
    }
}

/// The tools #2416 gave keys are bound in both shipped profiles.
#[test]
fn both_profiles_bind_the_common_sheet_tools() {
    let bindings = key_bindings(Platform::Windows);
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let profiles: Vec<String> = set.profiles().map(|profile| profile.id.clone()).collect();
    assert_eq!(profiles.len(), 2, "both shipped profiles: {profiles:?}");
    for profile in &profiles {
        for id in COMMON_TOOLS {
            assert!(
                bindings
                    .iter()
                    .any(|(p, _, command, _)| p == profile && command.as_str() == id),
                "{profile} binds no key to `{id}`"
            );
        }
    }
}
