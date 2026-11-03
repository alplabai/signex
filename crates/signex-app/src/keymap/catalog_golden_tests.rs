//! Characterization of the keymap catalog and the built-in shortcut profiles.
//!
//! `testdata/catalog.golden.tsv` pins every catalog command (id, category,
//! labels, group, icon, default key, enablement, flags) and
//! `testdata/builtin_profiles.golden.tsv` every binding of each built-in
//! profile (`bind` rows) plus each command's menu hint (`hint` rows). A diff is a behaviour change, not a snapshot
//! to refresh. Regenerate with `UPDATE_GOLDEN=1 cargo test -p signex-app golden`.

use super::catalog::{CommandMetadata, KeyBind};
use super::{
    AppCommandId, CompiledKeymap, KeyStroke, Modifiers, ShortcutBindingAction, ShortcutProfileSet,
    ShortcutTrigger, all_command_ids, metadata_for_str,
};

const CATALOG: &str = include_str!("testdata/catalog.golden.tsv");
const PROFILES: &str = include_str!("testdata/builtin_profiles.golden.tsv");
const CATALOG_HEADER: &str =
    "# id\tcategory\tlabel\tmenu_label\tgroup\ticon\tkeybind\tenablement\tflags";
const PROFILES_HEADER: &str = "# profile\tkind\tcontext\tsequence\taction";
const MISMATCH_NOTE: &str = "This is a behaviour change: a command or shortcut was added, removed or changed. Update the golden only with a maintainer decision that names it (see docs/DECISIONS.md).";

fn or_dash(text: Option<&str>) -> &str {
    text.unwrap_or("-")
}

/// Modifiers in the config spelling, `Cmd` on every OS, so the fixture is
/// the same on every platform.
fn keybind(bind: &KeyBind) -> String {
    let m = bind.modifiers;
    let mut out = String::new();
    for (on, name) in [
        (m.ctrl, "Ctrl+"),
        (m.command, "Cmd+"),
        (m.alt, "Alt+"),
        (m.shift, "Shift+"),
    ] {
        if on {
            out.push_str(name);
        }
    }
    out + bind.key
}

fn flags(meta: &CommandMetadata) -> String {
    let f = meta.flags;
    let names: Vec<&str> = [
        (f.gui_only, "gui_only"),
        (f.mutates_doc, "mutates_doc"),
        (f.undoable, "undoable"),
        (f.hidden, "hidden"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if names.is_empty() {
        "-".into()
    } else {
        names.join(",")
    }
}

fn catalog_text() -> String {
    let mut out = format!("{CATALOG_HEADER}\n");
    for id in all_command_ids() {
        let meta = metadata_for_str(id).expect("catalog id has metadata");
        let row = [
            meta.id.to_string(),
            meta.category.to_string(),
            meta.label.to_string(),
            or_dash(meta.menu_label).to_string(),
            meta.group.display_name().to_string(),
            or_dash(meta.icon.map(|i| i.0)).to_string(),
            meta.keybind.as_ref().map_or("-".into(), keybind),
            format!("{:?}", meta.enable),
            flags(meta),
        ];
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

/// Every binding of every built-in profile (`bind` rows, strokes in the
/// config spelling) and the menu hint each command shows (`hint` rows, the
/// logo key normalised to `Cmd`), so a dropped extra binding is noticed too.
fn profiles_text() -> String {
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let mut out = format!("{PROFILES_HEADER}\n");
    for profile in set.profiles() {
        for binding in &profile.bindings {
            let action = match &binding.action {
                ShortcutBindingAction::Command(id) => format!("command:{id}"),
                ShortcutBindingAction::Unbind(id) => format!("unbind:{id}"),
                ShortcutBindingAction::NoAction => "no_action".to_string(),
            };
            for trigger in &binding.triggers {
                let sequence = match trigger {
                    ShortcutTrigger::KeySequence(strokes) => strokes
                        .iter()
                        .map(KeyStroke::to_config_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                    ShortcutTrigger::PointerGesture(gesture) => format!("pointer:{gesture}"),
                };
                out.push_str(&format!(
                    "{}\tbind\t{:?}\t{sequence}\t{action}\n",
                    profile.id, binding.context
                ));
            }
        }
        let keymap = CompiledKeymap::compile(profile);
        let logo = format!("{}+", Modifiers::command_key_name());
        for id in all_command_ids() {
            let command = AppCommandId::new(id).expect("catalog id is valid");
            let hint = keymap
                .shortcut_label(&command)
                .map(|label| label.replace(&logo, "Cmd+"));
            out.push_str(&format!(
                "{}\thint\t-\t{}\t{id}\n",
                profile.id,
                or_dash(hint.as_deref())
            ));
        }
    }
    out
}

fn assert_golden(name: &str, want: &str, got: &str) {
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let path = format!("{}/src/keymap/testdata/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, got).expect("write golden");
        return;
    }
    if want == got {
        return;
    }
    let (want_rows, got_rows): (Vec<&str>, Vec<&str>) =
        (want.lines().collect(), got.lines().collect());
    let mut diff = String::new();
    for row in want_rows.iter().filter(|r| !got_rows.contains(r)) {
        diff.push_str(&format!("- {row}\n"));
    }
    for row in got_rows.iter().filter(|r| !want_rows.contains(r)) {
        diff.push_str(&format!("+ {row}\n"));
    }
    if diff.is_empty() {
        let mut sorted = (want_rows.clone(), got_rows.clone());
        sorted.0.sort_unstable();
        sorted.1.sort_unstable();
        diff.push_str(
            if sorted.0 == sorted.1 && want_rows.len() == got_rows.len() {
                "same rows in a different order\n"
            } else {
                "a row is duplicated or its count differs\n"
            },
        );
    }
    panic!("{name} differs (- golden, + live):\n{diff}{MISMATCH_NOTE}");
}

#[test]
fn golden_catalog_matches_the_live_catalog() {
    assert_golden("catalog.golden.tsv", CATALOG, &catalog_text());
}

#[test]
fn golden_builtin_profiles_match_the_compiled_keymaps() {
    assert_golden("builtin_profiles.golden.tsv", PROFILES, &profiles_text());
}
