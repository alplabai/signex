//! Characterization of the main menu bar's content.
//!
//! `testdata/menu_bar.golden.tsv` pins every menu, submenu, row, separator,
//! label, shortcut, message and enablement of the bar, per context variant.
//! It is the contract of the menu-model refactor: it must not change.
//!
//! The fixture's labels and shortcuts are checked against the live helpers,
//! every enablement bit against the live predicates (the `gate` column names
//! the rule the row was built with), and the model must reproduce it all.

use super::{MenuContext, cmd_label, enabled, shortcut_for};
use crate::app::command::CommandContext;
use crate::app::command::enablement::cycle_grid_menu_enabled;

const GOLDEN: &str = include_str!("testdata/menu_bar.golden.tsv");
const RECENT_GOLDEN: &str = include_str!("testdata/menu_bar_recent.golden.tsv");

/// Context variants, in the order of the `enabled[...]` column.
pub(super) fn variants() -> Vec<MenuContext<'static>> {
    let with = |commands: CommandContext| MenuContext {
        commands,
        ..MenuContext::default()
    };
    let base = CommandContext::default();
    vec![
        with(base),
        with(CommandContext {
            has_schematic: true,
            has_pcb: true,
            has_sheets: true,
            has_selection: true,
            can_undo: true,
            can_redo: true,
            project_dirty: true,
            has_dirty_documents: true,
            has_document: true,
            has_open_project: true,
            ..base
        }),
        with(CommandContext {
            has_pcb: true,
            has_document: true,
            ..base
        }),
        with(CommandContext {
            has_symbol_editor: true,
            has_document: true,
            ..base
        }),
        with(CommandContext {
            has_sheets: true,
            has_open_project: true,
            ..base
        }),
        with(CommandContext {
            undocked_has_canvas: true,
            ..base
        }),
        with(CommandContext {
            has_footprint_editor: true,
            has_document: true,
            ..base
        }),
        with(CommandContext {
            project_dirty: true,
            has_dirty_documents: true,
            has_open_project: true,
            ..base
        }),
    ]
}

fn eval_gate(gate: &str, ctx: &MenuContext) -> bool {
    match gate {
        "always" => true,
        "never" => false,
        "sch" => ctx.commands.has_schematic,
        "dirty" => ctx.commands.has_dirty_documents,
        "project" => ctx.commands.has_open_project,
        "cyclegrid" => cycle_grid_menu_enabled(&ctx.commands),
        other => {
            let id = other.strip_prefix("cmd:").expect("known gate");
            enabled(ctx, id)
        }
    }
}

fn rows() -> Vec<Vec<&'static str>> {
    GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').collect())
        .collect()
}

#[test]
fn golden_enablement_matches_live_predicates() {
    let ctxs = variants();
    for row in rows() {
        let (path, bits, gate) = (row[0], row[6], row[7]);
        if matches!(row[1], "menu" | "submenu" | "separator") {
            continue;
        }
        let got: String = ctxs
            .iter()
            .map(|c| if eval_gate(gate, c) { '1' } else { '0' })
            .collect();
        assert_eq!(got, bits, "enablement of {path} (gate {gate})");
    }
}

#[test]
fn golden_labels_and_shortcuts_match_helpers() {
    let ctx = MenuContext::default();
    for row in rows() {
        let (label, id, shortcut) = (row[2], row[3], row[4]);
        if id == "-" {
            continue;
        }
        assert_eq!(cmd_label(id, label), label, "label of {id}");
        let expected = (shortcut != "-").then(|| shortcut.to_string());
        assert_eq!(
            shortcut_for(&ctx, id, shortcut),
            expected,
            "shortcut of {id}"
        );
    }
}

/// Whitespace and trailing commas removed, so a call rustfmt wrapped over
/// several lines still matches.
fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<String>()
        .replace(",)", ")")
}

/// The model resolves labels through the command catalog, so the fixture
/// label alone would not notice a changed fallback literal: pin each
/// `cmd(id, label, shortcut)` call in the model source to the fixture.
#[test]
fn golden_fallback_literals_are_pinned_in_the_model_source() {
    let source = squash(include_str!("model/menus.rs"));
    let mut pinned = 0;
    for row in rows() {
        let (label, id, shortcut) = (row[2], row[3], row[4]);
        if id == "-" {
            continue;
        }
        let call = squash(&format!("cmd(\"{id}\", \"{label}\", \"{shortcut}\")"));
        assert!(source.contains(&call), "fallbacks of {id} differ: {call}");
        pinned += 1;
    }
    assert!(pinned >= 30, "only {pinned} command rows pinned");
}

// ─── The declarative model reproduces the golden fixture ────────────────

use super::model::{ItemState, Menu, MenuEntry, menu_bar_model};

/// One fixture row per entry, columns `path kind label command_id shortcut
/// action` plus the entry's enablement as `1` / `0` (`-` for containers).
fn flatten(menu: &Menu, parent: Option<&str>, out: &mut Vec<(String, char)>) {
    let path = parent.map_or(menu.title.clone(), |p| format!("{p}/{}", menu.title));
    let kind = if parent.is_some() { "submenu" } else { "menu" };
    out.push((format!("{path}\t{kind}\t{}\t-\t-\t-", menu.title), '-'));
    for entry in &menu.entries {
        match entry {
            MenuEntry::Separator => out.push((format!("{path}/---\tseparator\t-\t-\t-\t-"), '-')),
            MenuEntry::Submenu(sub) => flatten(sub, Some(&path), out),
            MenuEntry::Item(item) => {
                let (kind, action) = match &item.state {
                    ItemState::Placeholder | ItemState::Unavailable => ("stub", "-".to_string()),
                    ItemState::Live { action, .. } => ("item", format!("{action:?}")),
                };
                let (id, text) = item
                    .shortcut
                    .as_ref()
                    .map_or(("-", "-"), |s| (s.command_id.as_str(), s.text.as_str()));
                let cols = format!(
                    "{path}/{}\t{kind}\t{}\t{id}\t{text}\t{action}",
                    item.label, item.label
                );
                out.push((cols, if item.is_enabled() { '1' } else { '0' }));
            }
        }
    }
}

fn flat_rows(ctx: &MenuContext) -> Vec<(String, char)> {
    let mut out = Vec::new();
    for menu in &menu_bar_model(ctx).menus {
        flatten(menu, None, &mut out);
    }
    out
}

const RECENT_PREFIX: &str = "File/Open Recent/";

#[test]
fn model_reproduces_golden_menu_bar() {
    let per_variant: Vec<Vec<(String, char)>> = variants().iter().map(flat_rows).collect();
    let keep = |(cols, _): &(String, char)| !cols.starts_with(RECENT_PREFIX);
    let first: Vec<&(String, char)> = per_variant[0].iter().filter(|r| keep(r)).collect();
    let mut got = String::new();
    for (i, (cols, _)) in first.iter().enumerate() {
        let bits: String = per_variant
            .iter()
            .map(|rows| rows.iter().filter(|r| keep(r)).nth(i).expect("same shape"))
            .map(|(other, bit)| {
                assert_eq!(other, cols, "row shape must not depend on context");
                *bit
            })
            .collect();
        let bits = if bits.chars().all(|c| c == '-') {
            "-"
        } else {
            &bits
        };
        got.push_str(&format!("{cols}\t{bits}\n"));
    }
    let want: String = rows()
        .iter()
        .map(|r| format!("{}\n", r[..7].join("\t")))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn model_reproduces_golden_recent_projects() {
    let mut want: Vec<(&str, Vec<String>)> = Vec::new();
    for line in RECENT_GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        if let Some(name) = line.strip_prefix("@variant ") {
            want.push((name, Vec::new()));
        } else {
            want.last_mut()
                .expect("variant header")
                .1
                .push(line.to_string());
        }
    }
    for (name, expected) in want {
        let recent: Vec<std::path::PathBuf> = name
            .strip_prefix("paths=")
            .map(|p| p.split(',').map(Into::into).collect())
            .unwrap_or_default();
        let ctx = MenuContext {
            recent_projects: &recent,
            ..MenuContext::default()
        };
        let got: Vec<String> = flat_rows(&ctx)
            .into_iter()
            .filter(|(cols, _)| cols.starts_with(RECENT_PREFIX))
            .map(|(cols, bit)| format!("{cols}\t{bit}"))
            .collect();
        assert_eq!(got, expected, "Open Recent for {name}");
    }
}

#[test]
fn model_shortcuts_follow_the_active_keymap() {
    let keymap = crate::keymap::ShortcutProfileSet::built_ins()
        .expect("built-in profiles")
        .compile_active();
    let ctx = MenuContext {
        active_keymap: Some(std::sync::Arc::new(keymap)),
        ..MenuContext::default()
    };
    let mut checked = 0;
    for menu in &menu_bar_model(&ctx).menus {
        check_shortcuts(menu, &ctx, &mut checked);
    }
    assert_eq!(checked, 32, "the Altium profile labels 32 menu rows");
}

fn check_shortcuts(menu: &Menu, ctx: &MenuContext, checked: &mut usize) {
    for entry in &menu.entries {
        match entry {
            MenuEntry::Submenu(sub) => check_shortcuts(sub, ctx, checked),
            MenuEntry::Item(item) => {
                let Some(sc) = &item.shortcut else { continue };
                let keymap = ctx.active_keymap.as_ref().expect("keymap");
                let command = sc.command_id.clone();
                // With an active keymap a row shows the keymap's label or
                // no shortcut at all, never the catalog fallback (#1378).
                assert_eq!(
                    keymap.shortcut_label(&command).as_deref(),
                    Some(sc.text.as_str()),
                    "shortcut of {}",
                    sc.command_id
                );
                *checked += 1;
            }
            MenuEntry::Separator => {}
        }
    }
}

/// The chrome clamps its search bar with this width; it is derived from the
/// model's root titles and must stay at its pre-model value (38 label chars).
#[test]
fn approx_menu_bar_width_is_unchanged() {
    let titles: Vec<String> = menu_bar_model(&MenuContext::default())
        .menus
        .into_iter()
        .map(|menu| menu.title)
        .collect();
    assert_eq!(titles, super::model::ROOT_TITLES);
    assert_eq!(
        super::model::ROOT_TITLES,
        [
            "File", "Edit", "View", "Place", "Design", "Tools", "Window", "Help"
        ]
    );
    assert_eq!(super::approx_menu_bar_width(), 493.0);
}
