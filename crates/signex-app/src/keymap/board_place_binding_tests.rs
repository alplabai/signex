//! #2509: the Classic profile's board Place keys. Each one runs its tool on
//! a board on every OS, and none equals or prefixes another Classic binding
//! that is live on a board (global or board), so it never shadows one and
//! never makes one wait.

use std::str::FromStr;

use super::{
    CompiledKeymap, KeyStroke, Platform, ShortcutBindingAction, ShortcutContext,
    ShortcutProfileSet, ShortcutTrigger,
};

const ALL: [Platform; 3] = [Platform::Windows, Platform::Linux, Platform::MacOs];

/// The keys #2509 added, with the command each one runs.
const PLACE_KEYS: [(&str, &str); 5] = [
    ("Shift+T", "place_board_track"),
    ("Shift+V", "place_board_via"),
    ("Shift+G", "place_board_polygon_pour"),
    ("Shift+Alt+F", "place_board_fill"),
    ("Shift+Alt+R", "place_board_region"),
];

fn classic_set() -> ShortcutProfileSet {
    let mut set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    set.set_active_profile("classic").expect("classic profile");
    set
}

fn stroke(text: &str, platform: Platform) -> KeyStroke {
    KeyStroke::from_str(text)
        .expect("a stroke")
        .resolved_for(platform)
}

#[test]
fn classic_board_place_keys_run_their_tools_on_a_board() {
    let set = classic_set();
    let profile = set.profile("classic").expect("classic profile");
    let on_a_board = [ShortcutContext::Global, ShortcutContext::Pcb];
    for platform in ALL {
        let keymap = CompiledKeymap::compile_for(profile, platform);
        for (keys, id) in PLACE_KEYS {
            let lookup = keymap.lookup(&[stroke(keys, platform)], &on_a_board);
            assert_eq!(
                lookup.command.as_ref().map(|command| command.as_str()),
                Some(id),
                "{platform:?}: `{keys}` on a board"
            );
            assert!(!lookup.pending, "{platform:?}: `{keys}` waits for a chord");
            let command = lookup.command.expect("a command");
            assert!(
                crate::app::command::bridge::is_dispatchable(&command),
                "`{id}` does not resolve"
            );
        }
    }
}

#[test]
fn classic_board_place_keys_do_nothing_on_a_sheet() {
    let set = classic_set();
    let profile = set.profile("classic").expect("classic profile");
    let on_a_sheet = [ShortcutContext::Global, ShortcutContext::Schematic];
    for platform in ALL {
        let keymap = CompiledKeymap::compile_for(profile, platform);
        for (keys, _) in PLACE_KEYS {
            let lookup = keymap.lookup(&[stroke(keys, platform)], &on_a_sheet);
            assert!(
                !lookup.matched && !lookup.pending,
                "{platform:?}: `{keys}` is bound on a sheet: {:?}",
                lookup.command
            );
        }
    }
}

#[test]
fn no_classic_board_place_key_clashes_with_another_board_binding() {
    let set = classic_set();
    let profile = set.profile("classic").expect("classic profile");
    for platform in ALL {
        let mut live: Vec<(String, Vec<KeyStroke>)> = Vec::new();
        for binding in &profile.bindings {
            if !binding.applies_on(platform)
                || !matches!(
                    binding.context,
                    ShortcutContext::Global | ShortcutContext::Pcb
                )
            {
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
                live.push((command.as_str().to_string(), resolved));
            }
        }
        for (keys, id) in PLACE_KEYS {
            let mine = vec![stroke(keys, platform)];
            let own = live
                .iter()
                .filter(|(command, strokes)| command == id && *strokes == mine)
                .count();
            assert_eq!(own, 1, "{platform:?}: `{keys}` binds `{id}` once");
            for (other, strokes) in &live {
                if other == id && *strokes == mine {
                    continue;
                }
                let clashes = strokes.starts_with(&mine) || mine.starts_with(strokes);
                assert!(
                    !clashes,
                    "{platform:?}: `{keys}` (`{id}`) clashes with the binding of `{other}`"
                );
            }
        }
    }
}
