//! Keeps `docs/USER_GUIDE.md` honest against the shipped UI (#322).
//!
//! The guide names menu items and panel labels. A rename in the code that
//! leaves the guide behind sends a first-time user looking for a control that
//! is no longer there, so every label the walkthrough relies on must still
//! exist in the menu source, and the guide must not repeat caveats that have
//! been resolved. The keys it names must be the default profile's bindings
//! and its `Menu ▸ Item` paths must exist in the menu bar.

use signex_app::keymap::{
    KeyStroke, Platform, ShortcutBindingAction, ShortcutProfileSet, ShortcutTrigger,
};

const GUIDE: &str = include_str!("../../../../docs/USER_GUIDE.md");
const PANELS_FTL: &str = include_str!("../../../signex-i18n/locales/en-US/panels.ftl");
const PANEL_BODY_FTL: &str = include_str!("../../../signex-i18n/locales/en-US/panel_body.ftl");

/// Menu labels the guide's walkthrough tells the user to click, as spelled in
/// the menu model (ASCII `...`; the guide writes the ellipsis character).
const WALKTHROUGH_MENU_LABELS: &[&str] = &[
    "New Project",
    "Open Library...",
    "Place Component...",
    "Net Label",
    "Annotate Schematics Quietly",
    "Electrical Rules Check",
    "Save",
];

#[test]
fn guide_has_end_to_end_walkthrough() {
    assert!(
        GUIDE.contains("## Walkthrough: a saved schematic with a wired symbol"),
        "the guide needs the end-to-end walkthrough section (#322)"
    );
}

#[test]
fn guide_drops_resolved_stock_library_caveat() {
    // #328 shipped: the stock library mounts on every launch.
    assert!(
        !GUIDE.contains("is also not shipped yet"),
        "the stock library caveat is stale; #328 is closed"
    );
    assert!(
        GUIDE.contains("**Stock**"),
        "the guide must describe the Stock section of the Components panel"
    );
}

/// Every row label of the menu bar as the en-US model shows it: the labels
/// are catalog text, no longer literals in the menu source.
fn menu_labels() -> Vec<String> {
    use signex_app::menu_bar::MenuContext;
    use signex_app::menu_bar::model::{Menu, MenuEntry, menu_bar_model};
    fn collect(menu: &Menu, out: &mut Vec<String>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) => out.push(item.label.clone()),
                MenuEntry::Submenu(sub) => collect(sub, out),
                MenuEntry::Separator => {}
            }
        }
    }
    let mut labels = Vec::new();
    for menu in &menu_bar_model(&MenuContext::default()).menus {
        collect(menu, &mut labels);
    }
    labels
}

#[test]
fn walkthrough_menu_labels_exist_in_the_menu_source() {
    let menu_labels = menu_labels();
    for label in WALKTHROUGH_MENU_LABELS {
        assert!(
            GUIDE.contains(&label.replace("...", "\u{2026}")) || GUIDE.contains(label),
            "guide does not mention {label:?}"
        );
        assert!(
            menu_labels.iter().any(|shown| shown == label),
            "menu source no longer has {label:?}; update docs/USER_GUIDE.md"
        );
    }
}

#[test]
fn guide_stock_section_matches_the_panel() {
    // The panel's text lives in its en-US catalog since #2225.
    assert!(PANELS_FTL.contains("Stock library not found"));
    assert!(GUIDE.contains("Stock library not found"));
}

#[test]
fn guide_describes_placing_and_has_no_merge_leftovers() {
    // The trunk merge for #1160 once produced a guide that said placement
    // does not work yet next to the steps that make it work.
    assert!(
        !GUIDE.contains("No menu item or button reaches it yet"),
        "the guide still says placement is not wired"
    );
    // #2785 moved the row's Place from the row's end to its first cell.
    assert!(GUIDE.contains(
        "point at a part's row in the Components panel and press **Place** in its"
    ));
    for marker in ["<<<<<<< ", "||||||| ", ">>>>>>> "] {
        assert!(
            !GUIDE.contains(marker),
            "merge marker {marker:?} in the guide"
        );
    }
}

const NEW_PROJECT_FTL: &str = include_str!("../../../signex-i18n/locales/en-US/new-project.ftl");
const UPDATE_PCB_FTL: &str = include_str!("../../../signex-i18n/locales/en-US/update_pcb.ftl");
/// Place on a board lists the board's rows, which the default menu context
/// cannot reach from outside the crate; the golden test pins this file to
/// the model.
const PLACE_BOARD_TSV: &str =
    include_str!("../../src/menu_bar/testdata/menu_bar_place_board.golden.tsv");

/// The guide with every run of whitespace made one space, so a quoted phrase
/// can be found across the guide's line breaks.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The guide's paragraphs, each one line: blank-line separated, the
/// blockquote marks and indentation dropped.
fn paragraphs() -> Vec<String> {
    GUIDE
        .split("\n\n")
        .map(|para| {
            para.lines()
                .map(|line| line.trim_start().trim_start_matches('>').trim())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|para| !para.is_empty())
        .collect()
}

/// Whether the built-in profile `profile_id` binds `command` to `keys`, as
/// the guide spells them (`Ctrl+S`, `P W`), on `platform`.
fn binds(profile_id: &str, keys: &str, command: &str, platform: Platform) -> bool {
    let set = ShortcutProfileSet::built_ins().expect("the shipped profiles load");
    let profile = set
        .profile(profile_id)
        .unwrap_or_else(|| panic!("no built-in profile {profile_id:?}"));
    let resolve = |strokes: &[KeyStroke]| -> Vec<KeyStroke> {
        strokes.iter().map(|s| s.resolved_for(platform)).collect()
    };
    let Ok(ShortcutTrigger::KeySequence(wanted)) = ShortcutTrigger::parse(keys) else {
        panic!("the guide's key {keys:?} does not parse as a key sequence");
    };
    let wanted = resolve(&wanted);
    profile
        .bindings
        .iter()
        .filter(|binding| binding.applies_on(platform))
        .any(|binding| {
            matches!(&binding.action, ShortcutBindingAction::Command(id) if id.as_str() == command)
                && binding.triggers.iter().any(|trigger| {
                    matches!(trigger, ShortcutTrigger::KeySequence(seq) if resolve(seq) == wanted)
                })
        })
}

/// Each row of the §13 shortcut table: its Action cell, and the command of
/// each backticked key in its Shortcut cell, in order.
const SHORTCUT_TABLE: &[(&str, &[&str])] = &[
    ("New project", &["new_document"]),
    ("Open", &["open_document"]),
    ("Save", &["save_document"]),
    ("Print / export PDF", &["print"]),
    ("Undo / redo", &["undo", "redo", "redo"]),
    ("Place wire", &["place_wire", "place_wire"]),
    ("Place net label", &["place_net_label"]),
    ("Cancel the current tool", &["cancel_current_tool"]),
    ("Rotate selection", &["rotate_counterclockwise"]),
    ("Mirror selection", &["mirror_x", "mirror_y"]),
    ("Delete selection", &["delete_selection"]),
    ("Run ERC", &["run_erc"]),
    ("Annotate", &["annotate_schematic"]),
    ("Annotate quietly", &["annotate_schematic_quietly"]),
    ("Find / Find and Replace", &["find", "find_and_replace"]),
    (
        "Next / previous Find result",
        &["find_next", "find_previous"],
    ),
    ("Toggle visible grid", &["toggle_visible_grid"]),
    ("Board sketch (on a board)", &["board_sketch"]),
    (
        "Next / previous layer (on a board)",
        &["next_layer", "prev_layer"],
    ),
    ("Toggle ratsnest (on a board)", &["toggle_ratsnest"]),
    ("Flip to the other side (on a board)", &["flip_selection"]),
    (
        "Place track / via / polygon pour",
        &[
            "place_board_track",
            "place_board_via",
            "place_board_polygon_pour",
        ],
    ),
    ("Run DRC (on a board)", &["run_drc"]),
    ("3D view (on a board)", &["view_board_3d"]),
    (
        "Keyboard shortcut reference",
        &["show_current_command_hotkeys"],
    ),
    ("Preferences", &["open_preferences"]),
];

/// The text between backticks in `cell`, in order.
fn backticked(cell: &str) -> Vec<&str> {
    cell.split('`').skip(1).step_by(2).collect()
}

#[test]
fn shortcut_table_keys_are_the_default_profile_bindings() {
    let set = ShortcutProfileSet::built_ins().expect("the shipped profiles load");
    assert_eq!(
        set.active_profile_id(),
        "signex_standard",
        "§13 documents Signex Standard as the default profile"
    );
    let start = GUIDE
        .find("## 13. Keyboard shortcuts")
        .expect("the guide has the §13 shortcut section");
    let end = start + GUIDE[start..].find("## 14.").expect("§14 follows §13");
    let rows: Vec<(&str, &str)> = GUIDE[start..end]
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| Action "))
        .filter_map(|line| {
            let mut cells = line.trim_matches('|').split('|').map(str::trim);
            Some((cells.next()?, cells.next()?))
        })
        .collect();
    assert_eq!(
        rows.len(),
        SHORTCUT_TABLE.len(),
        "the §13 table has rows this test does not know: {rows:?}"
    );
    for (action, keys) in rows {
        let commands = SHORTCUT_TABLE
            .iter()
            .find(|(known, _)| *known == action)
            .map(|(_, commands)| *commands)
            .unwrap_or_else(|| panic!("§13 row {action:?} is not in SHORTCUT_TABLE"));
        let keys = backticked(keys);
        assert_eq!(
            keys.len(),
            commands.len(),
            "§13 row {action:?} lists {keys:?}"
        );
        for (key, command) in keys.iter().zip(commands) {
            for platform in [Platform::Linux, Platform::Windows] {
                assert!(
                    binds("signex_standard", key, command, platform),
                    "§13 row {action:?}: `{key}` is not bound to {command} in Signex Standard on {platform:?}"
                );
            }
        }
    }
}

/// Keys the guide names in its running text: a phrase of the guide, the key
/// as it spells it, the command, and the OS the key is for.
const IN_TEXT_KEYS: &[(&str, &str, &str, Platform)] = &[
    (
        "the command palette key (`Ctrl+Shift+P` in the default profile)",
        "Ctrl+Shift+P",
        "open_command_palette",
        Platform::Linux,
    ),
    (
        "The command palette (`Ctrl+Shift+P`)",
        "Ctrl+Shift+P",
        "open_command_palette",
        Platform::Linux,
    ),
    (
        "press `P` then `W`, one after the other, or press `Ctrl+W`",
        "P W",
        "place_wire",
        Platform::Linux,
    ),
    (
        "press `P` then `W`, one after the other, or press `Ctrl+W`",
        "Ctrl+W",
        "place_wire",
        Platform::Linux,
    ),
    (
        "(`Insert`, `Alt+I` on macOS)",
        "Insert",
        "copy_attributes_or_add_vertex",
        Platform::Linux,
    ),
    (
        "(`Insert`, `Alt+I` on macOS)",
        "Alt+I",
        "copy_attributes_or_add_vertex",
        Platform::MacOs,
    ),
    (
        "press `Insert` to add a corner and keep drawing (section 6)",
        "Insert",
        "copy_attributes_or_add_vertex",
        Platform::Linux,
    ),
    (
        "1. Press `F8`. This runs the check",
        "F8",
        "run_erc",
        Platform::Linux,
    ),
    ("**Run ERC.** Press `F8`", "F8", "run_erc", Platform::Linux),
];

#[test]
fn keys_in_the_guide_text_are_the_default_profile_bindings() {
    let guide = flat(GUIDE);
    for (phrase, key, command, platform) in IN_TEXT_KEYS {
        assert!(guide.contains(phrase), "guide no longer says {phrase:?}");
        assert!(
            binds("signex_standard", key, command, *platform),
            "{phrase:?}: `{key}` is not bound to {command} in Signex Standard on {platform:?}"
        );
    }
    // The tour card and the guide once named `Ctrl+K`; Signex Standard does
    // not bind it to the palette (Classic does).
    assert!(
        !guide.contains("`Ctrl+K`"),
        "the palette key is Ctrl+Shift+P"
    );
    // §13: the Classic profile's examples and New Project in both profiles.
    assert!(guide.contains("`W` for wire and `L` for label"));
    assert!(guide.contains("**File ▸ New Project** is `Ctrl+N` in both profiles"));
    for (key, command) in [
        ("W", "place_wire"),
        ("L", "place_local_net_label"),
        ("Ctrl+N", "new_document"),
    ] {
        assert!(
            binds("classic", key, command, Platform::Linux),
            "Classic: `{key}` is not bound to {command}"
        );
    }
}

/// Every path of the menu bar, as `▸`-free segment lists, with the menu
/// model's ASCII `...` written as the guide's ellipsis character.
fn menu_paths() -> Vec<Vec<String>> {
    use signex_app::menu_bar::MenuContext;
    use signex_app::menu_bar::model::{Menu, MenuEntry, menu_bar_model};
    fn shown(label: &str) -> String {
        label.replace("...", "\u{2026}")
    }
    fn walk(menu: &Menu, prefix: &[String], out: &mut Vec<Vec<String>>) {
        for entry in &menu.entries {
            let label = match entry {
                MenuEntry::Item(item) => &item.label,
                MenuEntry::Submenu(sub) => &sub.title,
                MenuEntry::Separator => continue,
            };
            let mut path = prefix.to_vec();
            path.push(shown(label));
            if let MenuEntry::Submenu(sub) = entry {
                walk(sub, &path, out);
            }
            out.push(path);
        }
    }
    let mut out = Vec::new();
    for menu in &menu_bar_model(&MenuContext::default()).menus {
        let root = vec![shown(&menu.title)];
        walk(menu, &root, &mut out);
        out.push(root);
    }
    for line in PLACE_BOARD_TSV.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let path = line.split('\t').next().unwrap_or_default();
        out.push(path.split('/').map(shown).collect());
    }
    out
}

/// Menu rows whose label carries a name, written in the guide with a
/// `<placeholder>`, and the catalog message that words them.
const NAMED_MENU_ROWS: &[(&str, &str)] = &[(
    "Design ▸ Import Changes From `<project>`",
    "update-pcb-menu-import = Import Changes From { $project }",
)];

/// Paths of the guide that are not in the menu bar, by their first segment:
/// the Preferences dialog's pages and the Properties panel's sections.
const NOT_MENU_BAR: &[&str] = &["Preferences", "Properties"];

#[test]
fn guide_menu_paths_exist_in_the_menu_bar() {
    let paths = menu_paths();
    let roots: Vec<&String> = paths
        .iter()
        .filter(|path| path.len() == 1)
        .map(|path| &path[0])
        .collect();
    let mut checked = 0;
    for para in paragraphs() {
        for bold in para.split("**").skip(1).step_by(2) {
            if !bold.contains('▸') {
                continue;
            }
            let mut segments: Vec<String> = bold.split('▸').map(|s| s.trim().to_string()).collect();
            while segments.last().is_some_and(|s| s.is_empty()) {
                segments.pop();
            }
            let first = segments.first().cloned().unwrap_or_default();
            if !roots.contains(&&first) {
                // A path that does not start at a menu-bar root is a dialog
                // page, a panel section or a context menu, and the guide
                // must say which.
                assert!(
                    NOT_MENU_BAR.contains(&first.as_str())
                        || para.to_lowercase().contains("right-click"),
                    "**{bold}** is not a menu-bar path, and its paragraph does not say it is a right-click menu"
                );
                continue;
            }
            if let Some((_, message)) = NAMED_MENU_ROWS.iter().find(|(row, _)| *row == bold) {
                assert!(UPDATE_PCB_FTL.contains(message), "{message:?} is gone");
                continue;
            }
            // The longest prefix that is a menu row. Segments past an item
            // that opens a dialog (its label ends in `…`) name the dialog's
            // pages.
            let found = (1..=segments.len())
                .rev()
                .find(|&k| paths.iter().any(|path| path[..] == segments[..k]));
            let resolves =
                found.is_some_and(|k| k == segments.len() || segments[k - 1].ends_with('\u{2026}'));
            assert!(
                resolves,
                "**{bold}** is not in the menu bar; update docs/USER_GUIDE.md"
            );
            checked += 1;
        }
    }
    assert!(checked > 50, "only {checked} menu paths found in the guide");
}

#[test]
fn guide_has_no_duplicated_adjacent_lines_or_paragraphs() {
    let lines: Vec<&str> = GUIDE.lines().map(str::trim).collect();
    for pair in lines.windows(2) {
        assert!(
            pair[0].is_empty() || pair[0] != pair[1],
            "line repeated in the guide: {:?}",
            pair[0]
        );
    }
    let paras = paragraphs();
    for pair in paras.windows(2) {
        assert_ne!(pair[0], pair[1], "paragraph repeated in the guide");
    }
    for para in &paras {
        let mut seen: Vec<&str> = Vec::new();
        for sentence in para.split(". ").map(str::trim).filter(|s| s.len() > 20) {
            assert!(
                !seen.contains(&sentence),
                "sentence repeated in one paragraph: {sentence:?}"
            );
            seen.push(sentence);
        }
    }
    // The merge leftover in §11 said both exports were under File ▸ Export
    // after the line that puts the BOM under Tools ▸ Reports.
    assert!(!GUIDE.contains("Both exports are under **File ▸ Export**"));
}

#[test]
fn walkthrough_steps_match_the_new_project_dialog_and_the_wire_tool() {
    for message in [
        "dialog-new-project-title = New Project",
        "dialog-new-project-name-label = Project name",
        "dialog-new-project-location-label = Location",
        "dialog-new-project-create = Create project",
    ] {
        assert!(NEW_PROJECT_FTL.contains(message), "{message:?} is gone");
    }
    let guide = flat(GUIDE);
    assert!(guide.contains(
        "name it in the **New Project** dialog (**Project name**, **Location**) and press **Create project** (section 3)"
    ));
    assert!(guide.contains("## 3. Create a project"));
    assert!(!guide.contains("name it in the save dialog"));
    assert!(
        guide.contains("click to start, click again to end it (the second click places the wire)")
    );
    assert!(guide.contains("## 6. Wire"));
    assert!(!guide.contains("double-click to stop"));
}

#[test]
fn guide_erc_panel_matches_the_catalog() {
    // Only section 9 counts: the same words appear in other sections.
    let start = GUIDE.find("\n## 9.").expect("section 9");
    let end = GUIDE[start + 1..].find("\n## 10.").expect("section 10") + start + 1;
    let erc_section = &GUIDE[start..end];
    for (message, quoted) in [
        (
            "panels-erc-empty-hint = Run ERC to populate project-wide diagnostics",
            "\"Run ERC to populate project-wide diagnostics\"",
        ),
        ("panels-erc-show = Show", "**Show**"),
        (
            "panels-erc-add-no-connect = Add No-Connect",
            "**Add No-Connect**",
        ),
        ("panels-erc-waive-net = Waive net", "**Waive net**"),
        ("panels-erc-restore = Restore", "**Restore**"),
        ("panels-erc-run-button = Run ERC", "**Run ERC**"),
        (
            "panels-erc-clear-tooltip = Clear the ERC results",
            "\"Clear the ERC results\"",
        ),
        ("panels-erc-waive = Waive", "**Waive**"),
    ] {
        assert!(
            PANELS_FTL.contains(message) || PANEL_BODY_FTL.contains(message),
            "{message:?} is gone"
        );
        assert!(
            erc_section.contains(quoted),
            "section 9 does not quote {quoted:?}"
        );
    }
    for stale in [
        "Show on Canvas",
        "No ERC diagnostics yet",
        "`<` and `>` step",
    ] {
        assert!(
            !flat(GUIDE).contains(stale),
            "the guide still says {stale:?}"
        );
    }
}
