//! Characterization of the keymap catalog and the built-in shortcut profiles.
//!
//! `testdata/catalog.golden.tsv` pins every catalog command (id, category,
//! labels, group, icon, default key, enablement, flags, operands) and
//! `testdata/builtin_profiles.golden.tsv` every binding of each built-in
//! profile as the TOML spells it (`bind` rows) plus each command's menu hint
//! (`hint` rows). `builtin_profiles_resolved.golden.tsv` is the same table as
//! Windows and Linux resolve it, and `builtin_profiles_macos.golden.tsv` as
//! macOS shows it. A diff is a behaviour change, not a snapshot to refresh. Regenerate with `UPDATE_GOLDEN=1 cargo test -p signex-app golden`.

use super::catalog::{CommandMetadata, KeyBind, describe_operands};
use super::{
    AppCommandId, CompiledKeymap, Platform, ShortcutBindingAction, ShortcutProfileSet,
    ShortcutTrigger, all_command_ids, metadata_for_str,
};
use signex_i18n::{I18n, t};

const CATALOG: &str = include_str!("testdata/catalog.golden.tsv");
const PROFILES: &str = include_str!("testdata/builtin_profiles.golden.tsv");
const RESOLVED: &str = include_str!("testdata/builtin_profiles_resolved.golden.tsv");
const MACOS: &str = include_str!("testdata/builtin_profiles_macos.golden.tsv");
const CATALOG_HEADER: &str =
    "# id\tcategory\tlabel\tmenu_label\tgroup\ticon\tkeybind\tenablement\tflags\toperands";
const PROFILES_HEADER: &str = "# profile\tkind\tcontext\tsequence\taction";
const MISMATCH_NOTE: &str = "This is a behaviour change: a command or shortcut was added, removed or changed. Update the golden only with a maintainer decision that names it (see docs/DECISIONS.md).";

fn or_dash(text: Option<&str>) -> &str {
    text.unwrap_or("-")
}

/// A key sequence cell: the minus key is written `"-"`, so it never reads as
/// the column's "none" marker.
fn key_cell(sequence: &str) -> String {
    if sequence == "-" {
        "\"-\"".to_owned()
    } else {
        sequence.to_owned()
    }
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
        (f.runs_headless, "runs_headless"),
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
            meta.label_in(I18n::en_us_ref()),
            meta.menu_label
                .map_or("-".into(), |id| t!(I18n::en_us_ref(), id).to_string()),
            meta.group.display_name().to_string(),
            or_dash(meta.icon.map(|i| i.0)).to_string(),
            meta.keybind.as_ref().map_or("-".into(), keybind),
            format!("{:?}", meta.enable),
            flags(meta),
            describe_operands(meta.operands),
        ];
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

/// How a stroke is written in a profile golden.
#[derive(Clone, Copy)]
pub(super) enum Render {
    /// As the profile file spells it (`Mod+S`), every binding listed, a
    /// platform-tagged one as `Global@macos`: a Ctrl-to-Mod rewrite shows.
    Spelling,
    /// The stroke a lookup compares: resolved for the platform, config
    /// spelling, only the bindings that apply there.
    Resolved(Platform),
    /// The label the UI shows on the platform (`⇧⌘S` on macOS).
    Shown(Platform),
}

/// Every binding of every built-in profile (`bind` rows) and the menu hint
/// each command shows (`hint` rows, Windows logo key normalised to `Cmd`),
/// so a dropped extra binding is noticed too.
fn profiles_text() -> String {
    profiles_text_for(Render::Spelling)
}

/// The hint rows and, for `Resolved`/`Shown`, the bind rows are those of the
/// given platform whatever the host; `Spelling` is the file as written, with
/// Windows hints.
pub(super) fn profiles_text_for(render: Render) -> String {
    let platform = match render {
        Render::Spelling => Platform::Windows,
        Render::Resolved(platform) | Render::Shown(platform) => platform,
    };
    let set = ShortcutProfileSet::built_ins().expect("built-in profiles");
    let mut out = format!("{PROFILES_HEADER}\n");
    for profile in set.profiles() {
        for binding in &profile.bindings {
            if !matches!(render, Render::Spelling) && !binding.applies_on(platform) {
                continue;
            }
            let context = match binding.platform {
                Some(only) if matches!(render, Render::Spelling) => {
                    format!(
                        "{:?}@{}",
                        binding.context,
                        format!("{only:?}").to_lowercase()
                    )
                }
                _ => format!("{:?}", binding.context),
            };
            let action = match &binding.action {
                ShortcutBindingAction::Command(id) => format!("command:{id}"),
                ShortcutBindingAction::Unbind(id) => format!("unbind:{id}"),
                ShortcutBindingAction::NoAction => "no_action".to_string(),
            };
            for trigger in &binding.triggers {
                let sequence = match trigger {
                    ShortcutTrigger::KeySequence(strokes) => strokes
                        .iter()
                        .map(|stroke| match render {
                            Render::Spelling => stroke.to_config_string(),
                            Render::Resolved(_) => stroke.resolved_for(platform).to_config_string(),
                            Render::Shown(_) => stroke.display_for(platform),
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                    ShortcutTrigger::PointerGesture(gesture) => format!("pointer:{gesture}"),
                };
                out.push_str(&format!(
                    "{}\tbind\t{context}\t{}\t{action}\n",
                    profile.id,
                    key_cell(&sequence)
                ));
            }
        }
        let keymap = CompiledKeymap::compile_for(profile, platform);
        let logo = format!("{}+", platform.logo_key_name());
        for id in all_command_ids() {
            let command = AppCommandId::new(id).expect("catalog id is valid");
            let hint = keymap
                .shortcut_label(&command)
                .map(|label| key_cell(&label.replace(&logo, "Cmd+")));
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

/// What Windows and Linux compare on a lookup. Unlike the spelling golden,
/// this one does not change when a profile starts saying `Mod`: the strokes
/// the platform resolves to stay exactly what they were.
#[test]
fn golden_builtin_profiles_resolve_the_same_on_windows_and_linux() {
    let windows = profiles_text_for(Render::Resolved(Platform::Windows));
    assert_golden("builtin_profiles_resolved.golden.tsv", RESOLVED, &windows);
    assert_eq!(
        profiles_text_for(Render::Resolved(Platform::Linux)),
        windows
    );
}

#[test]
fn golden_builtin_profiles_as_macos_shows_them() {
    let text = profiles_text_for(Render::Shown(Platform::MacOs));
    assert_golden("builtin_profiles_macos.golden.tsv", MACOS, &text);
}
