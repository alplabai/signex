//! What the built-in profiles say once they use `Mod` (#1645): macOS moves
//! exactly the approved standard set, collisions keep the Control key, and
//! Windows/Linux keep the hints and the priority they had with all-`Ctrl`
//! spellings.

use super::{CompiledKeymap, KeyStroke, Platform, ShortcutContext, ShortcutProfileSet};
use std::collections::BTreeMap;
use std::str::FromStr;

type Row = (ShortcutContext, String, String);

/// The approved platform-standard set: `(key, shift)` chords that are `Mod`.
const STANDARD: [(&str, bool); 25] = [
    ("n", false),
    ("o", false),
    ("s", false),
    ("p", false),
    ("z", false),
    ("x", false),
    ("c", false),
    ("v", false),
    ("a", false),
    ("f", false),
    (",", false),
    ("d", false),
    ("r", false),
    ("e", false),
    ("l", false),
    ("4", false),
    ("s", true),
    ("z", true),
    ("p", true),
    ("v", true),
    ("g", true),
    ("x", true),
    ("e", true),
    ("f", true),
    ("a", true),
];

fn compiled(platform: Platform, id: &str) -> CompiledKeymap {
    let set = ShortcutProfileSet::built_ins().unwrap();
    CompiledKeymap::compile_for(set.profile(id).unwrap(), platform)
}

fn rows(platform: Platform, id: &str) -> Vec<Row> {
    compiled(platform, id).resolved_table()
}

/// Windows versus macOS: every row that differs is a standard-set `Ctrl`
/// chord that became the same chord on Command, and the only rows with no
/// Windows counterpart are the macOS-only bindings. Redo is the one standard
/// chord that stays a plain `Ctrl+Shift+Z` row (the profile keeps one raw
/// owner of it on every OS) and gains `Cmd+Shift+Z` from a macOS section.
#[test]
fn macos_moves_exactly_the_approved_standard_set() {
    for (id, moved, extra) in [
        (
            "altium",
            18,
            vec!["\u{2325}\u{2318}F", "\u{21e7}\u{2318}Z", "\u{2318}W"],
        ),
        ("classic", 22, vec!["\u{21e7}\u{2318}Z"]),
    ] {
        let win = rows(Platform::Windows, id);
        let mac = rows(Platform::MacOs, id);
        let win_only: Vec<&Row> = win.iter().filter(|r| !mac.contains(r)).collect();
        let mut mac_only: Vec<&Row> = mac.iter().filter(|r| !win.contains(r)).collect();
        assert_eq!(win_only.len(), moved, "{id}: {win_only:?}");
        for (context, was, action) in win_only {
            let now = was.replacen("Ctrl+", "Cmd+", 1);
            let position = mac_only
                .iter()
                .position(|(c, s, a)| c == context && *s == now && a == action)
                .unwrap_or_else(|| panic!("{id}: {was} has no Cmd twin"));
            mac_only.remove(position);
            let chord = was.trim_start_matches("Ctrl+").to_ascii_lowercase();
            let shift = chord.starts_with("shift+");
            let key = chord.trim_start_matches("shift+");
            assert!(STANDARD.contains(&(key, shift)), "{id}: {was} moved");
        }
        let added: Vec<String> = mac_only
            .iter()
            .map(|(_, s, _)| KeyStroke::from_str(s).unwrap().display_for(Platform::MacOs))
            .collect();
        assert_eq!(added, extra, "{id}");
    }
}

/// Chords whose Command twin is a macOS system shortcut stay on Control.
#[test]
fn collisions_stay_on_physical_control_on_every_platform() {
    for (id, chords) in [
        (
            "altium",
            vec![
                "Ctrl+Q",
                "Ctrl+W",
                "Ctrl+H",
                "Ctrl+M",
                "Ctrl+SPACE",
                "Ctrl+TAB",
                "Ctrl+Shift+TAB",
                "Ctrl+Y",
                "Ctrl+Shift+Z",
                "Ctrl+F4",
                "Ctrl+PGDN",
            ],
        ),
        (
            "classic",
            vec![
                "Ctrl+Q",
                "Ctrl+G",
                "Ctrl+F5",
                "Ctrl+Y",
                "Ctrl+Shift+Z",
                "Ctrl+HOME",
                "Ctrl+K",
            ],
        ),
    ] {
        for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
            let table = rows(platform, id);
            for chord in &chords {
                assert!(
                    table.iter().any(|(_, s, _)| s == chord),
                    "{id} on {platform:?} lost {chord}"
                );
            }
        }
    }
}

/// Respelling `Ctrl+...` as `Mod+...` must not reorder the compiled
/// bindings: the last binding of a command is its menu hint. On Windows the
/// redo hint is `Ctrl+Y` and every command with several bindings shows the
/// hint it showed with all-`Ctrl` spellings (trunk c8bee4785).
#[test]
fn multi_binding_hints_keep_the_order_they_had_on_trunk() {
    let want: [(&str, &[(&str, &str)]); 2] = [
        (
            "altium",
            &[
                ("redo", "Ctrl+Y"),
                ("place_wire", "P W"),
                ("next_layer", "Shift++"),
                ("open_command_palette", "Ctrl+Shift+P"),
            ],
        ),
        (
            "classic",
            &[
                ("redo", "Ctrl+Y"),
                ("open_command_palette", "Ctrl+Shift+P"),
                ("next_layer", "Shift++"),
            ],
        ),
    ];
    for (id, hints) in want {
        for platform in [Platform::Windows, Platform::Linux] {
            let keymap = compiled(platform, id);
            for (command, hint) in hints {
                assert_eq!(
                    keymap.shortcut_label_for_id(command),
                    *hint,
                    "{id} {command}"
                );
            }
        }
        // macOS keeps trunk's redo hint too (its Mac chord sits before Ctrl+Y).
        assert_eq!(
            compiled(Platform::MacOs, id).shortcut_label_for_id("redo"),
            "\u{2303}Y",
            "{id}"
        );
        // A command gaining a second binding must be added to the table.
        let mut per_command: BTreeMap<String, usize> = BTreeMap::new();
        for (_, _, action) in rows(Platform::Windows, id) {
            *per_command.entry(action).or_default() += 1;
        }
        let multi: Vec<&str> = per_command
            .iter()
            .filter(|(_, n)| **n > 1)
            .map(|(a, _)| a.as_str())
            .collect();
        assert_eq!(multi.len(), 3, "{id}: {multi:?}");
    }
}
