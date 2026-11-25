//! #2509: on a board no built-in key waits for a chord it cannot finish. No
//! global binding is a strict prefix of a board (`pcb`) binding, or the
//! other way round, in any shipped profile on any OS. The sheet-only `P V K`
//! and `P V N` live in the `schematic` context, so the default profile's
//! `P V` places a via at once on a board and they still work on a sheet.

use std::str::FromStr;

use super::{
    CompiledKeymap, KeyStroke, Platform, ShortcutBindingAction, ShortcutContext,
    ShortcutProfileSet, ShortcutTrigger,
};

const ALL: [Platform; 3] = [Platform::Windows, Platform::Linux, Platform::MacOs];

fn strokes(text: &str, platform: Platform) -> Vec<KeyStroke> {
    text.split(' ')
        .map(|part| {
            KeyStroke::from_str(part)
                .expect("a stroke")
                .resolved_for(platform)
        })
        .collect()
}

fn default_keymap(platform: Platform) -> CompiledKeymap {
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let profile = set.profile("signex_standard").expect("default profile");
    CompiledKeymap::compile_for(profile, platform)
}

/// `(profile, context, command, strokes)` of every key binding of every
/// built-in profile that applies on `platform`.
fn key_bindings(platform: Platform) -> Vec<(String, ShortcutContext, String, Vec<KeyStroke>)> {
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
                let ShortcutTrigger::KeySequence(sequence) = trigger else {
                    continue;
                };
                let resolved = sequence
                    .iter()
                    .map(|stroke| stroke.resolved_for(platform))
                    .collect();
                out.push((
                    profile.id.clone(),
                    binding.context,
                    command.as_str().to_string(),
                    resolved,
                ));
            }
        }
    }
    out
}

#[test]
fn p_v_on_a_board_places_a_via_at_once() {
    let on_a_board = [ShortcutContext::Global, ShortcutContext::Pcb];
    for platform in ALL {
        let lookup = default_keymap(platform).lookup(&strokes("P V", platform), &on_a_board);
        assert_eq!(
            lookup.command.as_ref().map(|command| command.as_str()),
            Some("place_board_via"),
            "{platform:?}: `P V` on a board"
        );
        assert!(
            !lookup.pending,
            "{platform:?}: `P V` on a board waits for a third key"
        );
    }
}

#[test]
fn p_v_k_and_p_v_n_still_place_their_directives_on_a_sheet() {
    let on_a_sheet = [ShortcutContext::Global, ShortcutContext::Schematic];
    for platform in ALL {
        let keymap = default_keymap(platform);
        for (keys, id) in [("P V K", "place_compile_mask"), ("P V N", "place_no_erc")] {
            let lookup = keymap.lookup(&strokes(keys, platform), &on_a_sheet);
            assert_eq!(
                lookup.command.as_ref().map(|command| command.as_str()),
                Some(id),
                "{platform:?}: `{keys}` on a sheet"
            );
            assert!(!lookup.pending, "{platform:?}: `{keys}` waits on a sheet");
        }
        let prefix = keymap.lookup(&strokes("P V", platform), &on_a_sheet);
        assert!(
            prefix.pending && prefix.command.is_none(),
            "{platform:?}: `P V` on a sheet waits for K or N: {prefix:?}"
        );
    }
}

/// A board binding with the same keys as a global one outranks it, which is
/// how a board reuses a sheet key; a strict prefix in either direction
/// makes the shorter chord wait on a board instead.
#[test]
fn no_global_binding_is_a_prefix_of_a_board_binding() {
    for platform in ALL {
        let bindings = key_bindings(platform);
        for (profile, context, command, board) in &bindings {
            if *context != ShortcutContext::Pcb {
                continue;
            }
            for (other_profile, other_context, other, global) in &bindings {
                if other_profile != profile || *other_context != ShortcutContext::Global {
                    continue;
                }
                let strict_prefix = board.len() != global.len()
                    && (board.starts_with(global) || global.starts_with(board));
                assert!(
                    !strict_prefix,
                    "{profile} on {platform:?}: the board binding of `{command}` and the \
                     global binding of `{other}` are a chord and its prefix"
                );
            }
        }
    }
}
