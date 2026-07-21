//! Data-to-view tests (iced-rust skill §10): assert on the pure context-
//! menu entry builders — no GPU, no window, no widget tree. Each builder
//! turns app state into a `Vec<DropdownEntry<Message>>`; the widget that
//! renders those rows is tested in `signex-widgets`.

use super::*;

use super::items::{dd_disabled, dd_msg};
use super::menus::{CanvasShortcuts, canvas_menu_entries, tab_menu_entries};
use super::project_tree::{TreeNodeRole, tree_node_role};
use super::submenu::{align_entries, align_gate, place_entries};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::TreeIcon;

const TID: ThemeId = ThemeId::CatppuccinMocha;

fn labels(entries: &[DropdownEntry<Message>]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| match e {
            DropdownEntry::Item(it) => Some(it.label.clone()),
            _ => None,
        })
        .collect()
}

fn has_row(entries: &[DropdownEntry<Message>], label: &str) -> bool {
    labels(entries).iter().any(|l| l == label)
}

fn seps(entries: &[DropdownEntry<Message>]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e, DropdownEntry::Separator))
        .count()
}

fn customs(entries: &[DropdownEntry<Message>]) -> usize {
    entries
        .iter()
        .filter(|e| matches!(e, DropdownEntry::Custom(_)))
        .count()
}

fn disabled_of(entries: &[DropdownEntry<Message>], label: &str) -> bool {
    entries
        .iter()
        .find_map(|e| match e {
            DropdownEntry::Item(it) if it.label == label => Some(it.disabled),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing row {label:?}"))
}

fn shortcuts() -> CanvasShortcuts {
    CanvasShortcuts {
        find: "Ctrl+F".into(),
        cut: "Ctrl+X".into(),
        copy: "Ctrl+C".into(),
        paste: "Ctrl+V".into(),
        smart_paste: "Shift+Ctrl+V".into(),
    }
}

#[test]
fn dd_disabled_row_is_passive() {
    // A greyed stub drops its `on_press` and keeps its right-column badge.
    match dd_disabled(None, "PCB", Some("v2.0")) {
        DropdownEntry::Item(it) => {
            assert!(it.disabled);
            assert!(it.on_press.is_none());
            assert_eq!(it.shortcut.as_deref(), Some("v2.0"));
        }
        _ => panic!("expected Item"),
    }
}

#[test]
fn dd_msg_row_carries_message_and_optional_shortcut() {
    let with_sc = dd_msg(
        None,
        "Cut",
        "Ctrl+X",
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Cut)),
    );
    let no_sc = dd_msg(
        None,
        "Refresh",
        "",
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Cut)),
    );
    match with_sc {
        DropdownEntry::Item(it) => {
            assert!(!it.disabled);
            assert!(it.on_press.is_some());
            assert_eq!(it.shortcut.as_deref(), Some("Ctrl+X"));
        }
        _ => panic!("expected Item"),
    }
    match no_sc {
        DropdownEntry::Item(it) => assert!(it.shortcut.is_none()),
        _ => panic!("expected Item"),
    }
}

#[test]
fn canvas_menu_grows_and_gates_on_selection() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let with = canvas_menu_entries(TID, &tokens, true, false, None, &sc);
    let without = canvas_menu_entries(TID, &tokens, false, false, None, &sc);

    // Clipboard + find rows are always present.
    for row in ["Find Text...", "Cut", "Copy", "Paste", "Paste Special"] {
        assert!(has_row(&without, row), "missing {row}");
    }
    // Selection-only rows appear only with a selection.
    assert!(!has_row(&without, "Rotate"));
    assert!(has_row(&with, "Rotate"));
    assert!(has_row(&with, "Properties..."));
    // Place launcher is a Custom row; Align launcher joins it on selection.
    assert_eq!(customs(&without), 1);
    assert_eq!(customs(&with), 2);
}

#[test]
fn canvas_child_sheet_adds_open_row() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let plain = canvas_menu_entries(TID, &tokens, false, false, None, &sc);
    let child = canvas_menu_entries(TID, &tokens, false, true, None, &sc);
    assert!(!has_row(&plain, "Open Child Sheet"));
    assert!(has_row(&child, "Open Child Sheet"));
}

#[test]
fn tab_menu_gates_bulk_close_and_undock() {
    let single = tab_menu_entries("sheet1.SchDoc", 0, 1, false);
    let multi = tab_menu_entries("sheet1.SchDoc", 2, 3, false);
    // Close row carries the live tab title.
    assert!(has_row(&single, "Close sheet1.SchDoc"));
    // Single tab → "Close All Other Documents" is greyed.
    assert!(disabled_of(&single, "Close All Other Documents"));
    assert!(!disabled_of(&multi, "Close All Other Documents"));
    // Undock greys out once the tab has its own window.
    let undocked = tab_menu_entries("s", 0, 3, true);
    assert!(!disabled_of(&multi, "Open In New Window"));
    assert!(disabled_of(&undocked, "Open In New Window"));
    assert_eq!(seps(&single), 1);
}

#[test]
fn tree_role_precedence() {
    assert_eq!(
        tree_node_role(&TreeIcon::SnxProject, 1, true),
        TreeNodeRole::Root
    );
    assert_eq!(
        tree_node_role(&TreeIcon::SnxLibrary, 3, false),
        TreeNodeRole::Library
    );
    // SnxLibrary off the depth-3 slot is a normal openable leaf.
    assert_eq!(
        tree_node_role(&TreeIcon::SnxLibrary, 2, false),
        TreeNodeRole::OpenableLeaf
    );
    assert_eq!(
        tree_node_role(&TreeIcon::Schematic, 2, false),
        TreeNodeRole::OpenableLeaf
    );
    assert_eq!(
        tree_node_role(&TreeIcon::Folder, 2, true),
        TreeNodeRole::Container
    );
    assert_eq!(
        tree_node_role(&TreeIcon::Folder, 2, false),
        TreeNodeRole::Unknown
    );
}

#[test]
fn align_gate_thresholds() {
    assert_eq!(align_gate(0), (false, false));
    assert_eq!(align_gate(1), (false, false));
    assert_eq!(align_gate(2), (true, false));
    assert_eq!(align_gate(3), (true, true));
}

#[test]
fn align_menu_disables_pairwise_below_two() {
    let one = align_entries(TID, 1);
    let three = align_entries(TID, 3);
    // Pairwise + distribute greyed with one item; grid always on.
    assert!(disabled_of(&one, "Align Left"));
    assert!(disabled_of(&one, "Distribute Horizontally"));
    assert!(!disabled_of(&one, "Align To Grid"));
    // All live with three selected.
    assert!(!disabled_of(&three, "Align Left"));
    assert!(!disabled_of(&three, "Distribute Horizontally"));
    // Two separators frame the three groups.
    assert_eq!(seps(&three), 2);
}

#[test]
fn place_menu_is_all_enabled_active_bar_rows() {
    let e = place_entries(TID);
    assert!(has_row(&e, "Wire"));
    assert!(has_row(&e, "Part"));
    // No Place row is greyed — placement actions are always available.
    assert!(e.iter().all(|entry| match entry {
        DropdownEntry::Item(it) => !it.disabled,
        _ => true,
    }));
}
