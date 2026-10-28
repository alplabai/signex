//! Characterization of the main menu bar's content.
//!
//! `testdata/menu_bar.golden.tsv` pins every menu, submenu, row, separator,
//! label, shortcut, message and enablement of the bar, per context variant.
//! It is the contract of the menu-model refactor: it must not change.
//!
//! These checks run against `view.rs` as it was hand-built from iced widgets:
//! the structure is cross-checked against the source text, the label and
//! shortcut against the live helpers, and every enablement bit against the
//! live predicates (the `gate` column names the rule the row was built with).

use super::{MenuContext, cmd_label, enabled, shortcut_for};
use crate::app::command::CommandContext;
use crate::app::command::enablement::cycle_grid_menu_enabled;

const GOLDEN: &str = include_str!("testdata/menu_bar.golden.tsv");
const RECENT_GOLDEN: &str = include_str!("testdata/menu_bar_recent.golden.tsv");
const VIEW_SOURCE: &str = include_str!("view.rs");

/// Context variants, in the order of the `enabled[...]` column.
pub(super) fn variants() -> Vec<MenuContext> {
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
            ..base
        }),
        with(CommandContext {
            has_pcb: true,
            ..base
        }),
        with(CommandContext {
            has_symbol_editor: true,
            ..base
        }),
        with(CommandContext {
            has_sheets: true,
            ..base
        }),
        with(CommandContext {
            undocked_has_canvas: true,
            ..base
        }),
        with(CommandContext {
            has_footprint_editor: true,
            ..base
        }),
        with(CommandContext {
            project_dirty: true,
            ..base
        }),
    ]
}

fn eval_gate(gate: &str, ctx: &MenuContext) -> bool {
    match gate {
        "always" => true,
        "never" => false,
        "sch" => ctx.commands.has_schematic,
        "cyclegrid" => cycle_grid_menu_enabled(&ctx.commands),
        "-" => true,
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

#[test]
fn golden_structure_is_present_in_view_source() {
    for row in rows() {
        let (kind, label, id, shortcut, action) = (row[1], row[2], row[3], row[4], row[5]);
        if kind != "separator" {
            assert!(
                VIEW_SOURCE.contains(&format!("\"{label}\"")),
                "label {label:?} not built by view.rs"
            );
        }
        if action != "-" {
            assert!(
                VIEW_SOURCE.contains(&format!("MenuMessage::{action}")),
                "message {action} not sent by view.rs"
            );
        }
        if id != "-" && shortcut != "-" {
            assert!(
                VIEW_SOURCE.contains(&format!("shortcut_for(&ctx, \"{id}\", \"{shortcut}\")")),
                "shortcut fallback of {id} differs in view.rs"
            );
        }
    }
    let mut count = std::collections::BTreeMap::new();
    for row in rows() {
        *count.entry(row[1]).or_insert(0) += 1;
    }
    assert_eq!(count["menu"], 8, "root menus");
}

#[test]
fn recent_golden_is_present_in_view_source() {
    let rows = RECENT_GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with('@') && !l.is_empty());
    for line in rows {
        let cols: Vec<&str> = line.split('\t').collect();
        let (kind, label, action) = (cols[1], cols[2], cols[5]);
        if action.starts_with("OpenRecentProject") {
            assert!(VIEW_SOURCE.contains("MenuMessage::OpenRecentProject(path.clone())"));
            continue;
        }
        if kind != "separator" {
            assert!(VIEW_SOURCE.contains(&format!("\"{label}\"")), "{line}");
        }
        if action != "-" {
            assert!(
                VIEW_SOURCE.contains(&format!("MenuMessage::{action}")),
                "{line}"
            );
        }
    }
}
