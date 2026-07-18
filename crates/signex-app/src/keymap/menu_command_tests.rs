//! Drift guard (#272): every `AppCommandId` a menu surface names must
//! resolve in the command catalog (`CommandMetadata`). The menu surfaces
//! reference commands by string id — `menu_bar` via `shortcut_for` /
//! `cmd_label`, the context menu via `keymap_shortcut_label` — and derive
//! the row's shortcut (and, since #282, its label) from that id. If a menu
//! adds a row keyed on an id with no catalog entry, the shortcut/label
//! lookup silently falls back to the literal and the command never reaches
//! the Keyboard Shortcuts pane / palette. This test scans the menu source
//! files and fails on any such orphan id, so the catalog stays the single
//! source of truth as menus evolve.
//!
//! Source-scanning keeps the guard automatic: a new menu row written with
//! any of the tracked call shapes is checked without editing this test.
//! `active_bar` is intentionally out of scope — its dropdown rows dispatch
//! `ActiveBarAction` enum variants, not `AppCommandId` strings, so there is
//! no id to drift.

use super::{AppCommandId, metadata_for};

/// Menu source files scanned for command-id references, relative to the
/// crate root (`CARGO_MANIFEST_DIR`).
const MENU_SOURCES: &[&str] = &[
    "src/menu_bar/view.rs",
    "src/app/view/context_menu/menus.rs",
    "src/app/view/context_menu/project_tree.rs",
    "src/app/view/context_menu/submenu.rs",
    "src/app/view/context_menu/items.rs",
];

/// Call shapes that take a command id as their first string argument.
/// Each menu row keyed on a command names it through one of these.
const ID_CALLS: &[&str] = &["shortcut_for(&ctx,", "cmd_label(", "keymap_shortcut_label("];

/// Pull the first `"..."` string-literal argument that follows `call` in
/// `src`, collecting one id per call site. Deliberately tiny (no regex
/// dependency): it walks to each occurrence of `call`, skips to the next
/// double-quote, and reads to the closing quote.
fn ids_from_call(src: &str, call: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = src;
    while let Some(pos) = rest.find(call) {
        let after = &rest[pos + call.len()..];
        if let Some(open) = after.find('"') {
            let tail = &after[open + 1..];
            if let Some(close) = tail.find('"') {
                ids.push(tail[..close].to_string());
            }
            rest = tail;
        } else {
            rest = after;
        }
    }
    ids
}

#[test]
fn every_menu_command_id_resolves_in_the_catalog() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut orphans: Vec<(String, String)> = Vec::new();
    let mut checked = 0usize;

    for rel in MENU_SOURCES {
        let path = format!("{manifest}/{rel}");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read menu source {rel}: {e}"));
        for call in ID_CALLS {
            for id in ids_from_call(&src, call) {
                checked += 1;
                // The literal must be a well-formed id AND have catalog
                // metadata; either failure is an orphan menu row.
                let resolved = AppCommandId::new(&id)
                    .ok()
                    .and_then(|command| metadata_for(&command))
                    .is_some();
                if !resolved {
                    orphans.push((rel.to_string(), id));
                }
            }
        }
    }

    // Guard against the scan silently matching nothing (e.g. a call-shape
    // rename): the menus reference dozens of ids today.
    assert!(
        checked >= 30,
        "menu-command scan found only {checked} id references — the call \
         shapes in ID_CALLS may have drifted from the menu source"
    );
    assert!(
        orphans.is_empty(),
        "menu rows reference command ids with no CommandMetadata entry \
         (add them to keymap/catalog): {orphans:?}"
    );
}
