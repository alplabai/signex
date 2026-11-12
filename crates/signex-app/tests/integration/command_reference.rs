//! Generates `docs/COMMANDS.md` from the command catalog and the shipped
//! keymap profiles, and fails if the checked-in copy has drifted (#277).
//!
//! There was no command reference at all before this.
//! `docs/KEYBOARD_SHORTCUTS.md` documents the TOML profile *format* and
//! contains no command table, so a user could not discover what they were
//! allowed to bind a key to except by reading `keymap/catalog/*.rs`
//! (signex#517 §8).
//!
//! Generated rather than hand-written for the reason the epic exists: the
//! catalog is the single source, and a hand-kept second copy drifts. Same
//! golden-file shape as `command_id_surface_matches_golden_snapshot`
//! (signex#276) — regenerate deliberately, never just to make a test pass:
//!
//! ```text
//! UPDATE_DOCS=1 cargo test -p signex-app --test integration command_reference
//! ```

use std::collections::BTreeMap;

use signex_app::keymap::{AppCommandId, CommandGroup, all_command_ids, metadata_for};

const ALTIUM_TOML: &str = include_str!("../../assets/keyboard-shortcuts/altium.toml");
const CLASSIC_TOML: &str = include_str!("../../assets/keyboard-shortcuts/classic.toml");

const DOC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/COMMANDS.md");

/// Command id → every trigger bound to it in one profile, in file order.
///
/// Parses the `"TRIGGER" = "command_id"` binding lines directly rather
/// than going through the TOML loader: this is a documentation artifact,
/// and reading the shipped file verbatim is what keeps it honest. Profile
/// metadata (`profile_id = "altium"`, `context = "global"`) is excluded
/// by requiring a quoted key.
///
/// Each trigger is its finished table text: `` `Mod+S` ``, and
/// `` `Mod+W` (macOS) `` when the section is tagged `platform = "macos"`.
fn bindings(profile: &str) -> BTreeMap<&str, Vec<String>> {
    let mut out: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut platform: Option<&str> = None;
    for line in profile.lines() {
        let line = line.trim();
        if line.starts_with("[[") {
            platform = None;
        }
        if let Some(tag) = line.strip_prefix("platform = ") {
            platform = match tag.trim_matches('"') {
                "macos" => Some("macOS"),
                "windows" => Some("Windows"),
                "linux" => Some("Linux"),
                _ => None,
            };
        }
        let Some(rest) = line.strip_prefix('"') else {
            continue;
        };
        let Some((trigger, rest)) = rest.split_once('"') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = rest.trim().trim_matches('"');
        if value.is_empty() || trigger.is_empty() {
            continue;
        }
        let text = match platform {
            Some(only) => format!("`{trigger}` ({only})"),
            None => format!("`{trigger}`"),
        };
        out.entry(value).or_default().push(text);
    }
    out
}

fn cell(bound: Option<&Vec<String>>) -> String {
    match bound {
        Some(triggers) => triggers.join(", "),
        None => "—".to_string(),
    }
}

fn render() -> String {
    let altium = bindings(ALTIUM_TOML);
    let classic = bindings(CLASSIC_TOML);

    let mut doc = String::new();
    doc.push_str(
        "# Signex command reference\n\
         \n\
         Every command the application exposes by a stable id, with the default\n\
         binding each shipped keyboard profile gives it. A keymap profile binds\n\
         keys to these ids — see [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md)\n\
         for the file format.\n\
         \n\
         **Generated — do not edit by hand.** Produced from\n\
         `crates/signex-app/src/keymap/catalog/` and\n\
         `crates/signex-app/assets/keyboard-shortcuts/` by\n\
         `crates/signex-app/tests/integration/command_reference.rs`, which fails if this\n\
         file drifts. Regenerate with:\n\
         \n\
         ```sh\n\
         UPDATE_DOCS=1 cargo test -p signex-app --test integration command_reference\n\
         ```\n\
         \n\
         A command listed here resolves in the keymap. Not all of them reach an\n\
         action yet — the ones that do not are pinned in\n\
         `crates/signex-app/src/app/command/bridge.rs`, and that set may only\n\
         shrink.\n\
         \n\
         ## Running a command from a script or an agent\n\
         \n\
         A command run by id can take `key=value` operands in place of a click or a\n\
         dialog answer. A command that acts on a document (`undo`, `redo`,\n\
         `save_document`, `save_document_as`, `close_active_document`, `color_net`,\n\
         `clear_all_net_colors`) names it with the required `document` operand, an\n\
         id or an absolute path, and acts on that document alone: never the active\n\
         one, and it changes no tab, focus, panel or selection. A person's palette or\n\
         keybinding call has no operands and keeps the active document.\n\
         \n\
         `list_documents`, `document_info`, `resolve`, `find_items`, `read_document` and\n\
         `query_pins` are read-only queries that answer with data\n\
         (`CommandOutcome::Answered`); `find` is still the Find dialog.\n\
         `query_pins document=<doc> designator=<ref>` lists the pins of a part on\n\
         one sheet: number, name, electrical type, hidden, unit, shared and the tip\n\
         in mm. `check_erc document=<sheet> [baseline=<run>]` is a query too: it\n\
         runs ERC on the sheet's project, answers with a check report once the run\n\
         lands and writes nothing to the ERC panel or Messages; rules in\n\
         [CLI.md](CLI.md), \"Checks on demand\".\n\
         \n\
         A point operand (`at`, `start`, `end`) also takes a pin reference\n\
         `<designator>.<pin>` (`R1.2`) or `<sheet name>.<entry name>` (`Charger.SDA`):\n\
         the pin's tip, or the sheet entry's point, on the sheet the edit targets.\n\
         `place_wire`, `draw_wire`, `place_bus` and `draw_bus` check a run before it\n\
         lands and answer with a report (optional operands `allow_merge`,\n\
         `allow_open`, `dry_run`); rules in [CLI.md](CLI.md), \"Wire intent\".\n\
         \n\
         `place_component library=.. part=.. at=x,y` places a row of an open library\n\
         and answers with its designator, unit and pin tips; `place_sheet_symbol`,\n\
         `place_sheet_entry` and `sync_sheet_pins` edit the hierarchy without writing\n\
         a file. Without operands, in a window, they open the picker, arm the tool or\n\
         act on the selection as before; rules in [CLI.md](CLI.md), \"Placing parts\n\
         and hierarchy\".\n\
         \n\
         Ids and where each file kind stores one: [DOCUMENT_IDS.md](DOCUMENT_IDS.md).\n",
    );

    for group in CommandGroup::ALL {
        let mut rows: Vec<(&'static str, String)> = all_command_ids()
            .filter_map(|id| {
                let command = AppCommandId::new(id).ok()?;
                let metadata = metadata_for(&command)?;
                if metadata.group != *group {
                    return None;
                }
                Some((
                    id,
                    format!(
                        "| `{}` | {} | {} | {} | {} |\n",
                        id,
                        metadata.label_in(signex_i18n::I18n::en_us_ref()),
                        metadata.category,
                        cell(altium.get(id)),
                        cell(classic.get(id)),
                    ),
                ))
            })
            .collect();
        rows.sort_by_key(|(id, _)| *id);

        doc.push_str(&format!("\n## {}\n\n", group.display_name()));
        if rows.is_empty() {
            doc.push_str("_No commands in this group yet._\n");
            continue;
        }
        doc.push_str("| command id | label | category | Altium | Classic |\n");
        doc.push_str("| --- | --- | --- | --- | --- |\n");
        for (_, row) in rows {
            doc.push_str(&row);
        }
    }
    doc
}

#[test]
fn command_reference_matches_the_catalog() {
    let actual = render();

    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::write(DOC_PATH, &actual)
            .unwrap_or_else(|err| panic!("failed to write {DOC_PATH}: {err}"));
        return;
    }

    let expected = std::fs::read_to_string(DOC_PATH)
        .unwrap_or_else(|err| panic!("failed to read {DOC_PATH}: {err}"));
    assert_eq!(
        actual, expected,
        "docs/COMMANDS.md is stale — regenerate with \
         `UPDATE_DOCS=1 cargo test -p signex-app --test integration command_reference`"
    );
}

/// The binding parser must actually find bindings. A silent zero would
/// render every shortcut column as `—` and still match a golden that had
/// been regenerated from the same broken parse.
#[test]
fn both_shipped_profiles_yield_bindings() {
    let altium = bindings(ALTIUM_TOML);
    let classic = bindings(CLASSIC_TOML);
    assert!(
        altium.len() >= 70,
        "altium.toml parsed to only {} bound ids — binding-line shape changed?",
        altium.len()
    );
    assert!(
        classic.len() >= 85,
        "classic.toml parsed to only {} bound ids — binding-line shape changed?",
        classic.len()
    );
    assert!(
        !altium.contains_key("altium") && !altium.contains_key("global"),
        "profile metadata leaked into the binding map"
    );
}
