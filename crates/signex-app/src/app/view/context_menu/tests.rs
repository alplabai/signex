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
    let with = canvas_menu_entries(TID, &tokens, true, false, false, None, &sc);
    let without = canvas_menu_entries(TID, &tokens, false, false, false, None, &sc);

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
    let plain = canvas_menu_entries(TID, &tokens, false, false, false, None, &sc);
    let child = canvas_menu_entries(TID, &tokens, false, true, false, None, &sc);
    assert!(!has_row(&plain, "Open Child Sheet"));
    assert!(has_row(&child, "Open Child Sheet"));
    assert!(!has_row(&plain, "Sync Sheet Pins"));
    assert!(has_row(&child, "Sync Sheet Pins"));
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

#[test]
fn refresh_all_pricing_row_is_live_for_a_library_node() {
    use super::project_tree::refresh_pricing_entry;
    let app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    let row = crate::app::LibraryRowRef {
        project_id: app.document_state.projects[0].id,
        root: std::path::PathBuf::from("/libs/Power.snxlib"),
    };
    match refresh_pricing_entry(Some(row.clone()), true) {
        DropdownEntry::Item(it) => {
            assert_eq!(it.label, "Refresh All Pricing");
            assert!(!it.disabled, "the row used to be a greyed stub");
            match it.on_press {
                Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                    crate::app::ProjectTreeAction::RefreshLibraryPricing(got),
                ))) => assert_eq!(got, row),
                other => panic!("wrong message: {other:?}"),
            }
        }
        _ => panic!("expected an Item"),
    }
    assert!(
        disabled_of(&[refresh_pricing_entry(None, true)], "Refresh All Pricing"),
        "no resolvable library, nothing to refresh"
    );
}

#[test]
fn refresh_all_pricing_row_is_greyed_with_a_reason_when_no_client_prices() {
    use super::project_tree::refresh_pricing_entry;
    let app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    let row = crate::app::LibraryRowRef {
        project_id: app.document_state.projects[0].id,
        root: std::path::PathBuf::from("/libs/Power.snxlib"),
    };
    match refresh_pricing_entry(Some(row), false) {
        DropdownEntry::Item(it) => {
            assert!(it.disabled);
            assert!(it.on_press.is_none(), "a greyed row must not publish");
            assert_eq!(
                it.shortcut.as_deref(),
                Some("No enabled distributor returns prices yet")
            );
        }
        _ => panic!("expected an Item"),
    }
}

/// #1098: the Enable Version Control row follows the Model fact alone. The
/// directory below does not exist, so any disk read would answer differently.
#[test]
fn enable_version_control_follows_the_model_fact_not_the_disk() {
    use crate::app::state::project_fs::git_marker;
    let dir = std::path::Path::new("snx-1098-no-such-dir");
    let (mut app, _task) = Signex::new();
    assert!(
        !app.offers_enable_version_control(dir),
        "unprobed must not offer the row"
    );
    let facts = &mut app.document_state.project_fs.facts;
    facts.set_exists(git_marker(dir), false);
    assert!(app.offers_enable_version_control(dir), "probed: no .git");
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(dir), true);
    assert!(!app.offers_enable_version_control(dir), "probed: has .git");
}

// ---- #271: context-menu rows read their text from the command table ----

#[test]
fn place_and_align_row_text_is_byte_identical_to_the_old_literals() {
    // Routing the rows through `action_label` must not change what the
    // user sees; this pins the wording that was hard-coded before.
    assert_eq!(
        labels(&place_entries(TID)),
        [
            "Wire",
            "Bus",
            "Bus Entry",
            "Net Label",
            "Port",
            "Off Sheet Connector",
            "GND Power Port",
            "VCC Power Port",
            "+5 Power Port",
            "+12 Power Port",
            "Parameter Set",
            "Generic No ERC",
            "Differential Pair",
            "Blanket",
            "Signal Harness",
            "Harness Connector",
            "Harness Entry",
            "Sheet Symbol",
            "Sheet Entry",
            "Device Sheet Symbol",
            "Part",
            "Text String",
            "Text Frame",
            "Note",
        ]
    );
    assert_eq!(
        labels(&align_entries(TID, 3)),
        [
            "Align Left",
            "Align Right",
            "Align Horizontal Centers",
            "Distribute Horizontally",
            "Align Top",
            "Align Bottom",
            "Align Vertical Centers",
            "Distribute Vertically",
            "Align To Grid",
        ]
    );
}

#[test]
fn canvas_row_text_is_byte_identical_to_the_old_literals() {
    let tokens = theme_tokens(TID);
    let rows = canvas_menu_entries(TID, &tokens, true, true, false, None, &shortcuts());
    for expected in [
        "Find Text...",
        "Cut",
        "Copy",
        "Paste",
        "Paste Special",
        "Rotate",
        "Mirror X",
        "Mirror Y",
        "Delete",
        "Preferences...",
        "Properties...",
    ] {
        assert!(has_row(&rows, expected), "missing {expected:?}");
    }
}

/// The same `AppCommandId` reads the same label on the menu bar and the
/// context menu wherever both carry it, and the Active Bar actions reached
/// from the Place / Align flyouts read the catalog `menu_label` the Active
/// Bar itself renders.
#[test]
fn shared_command_ids_read_the_same_label_on_every_surface() {
    use crate::active_bar::ActiveBarAction as A;
    use crate::app::command::active_bar::{action_label, id_for_action};

    // Context-menu canvas ids: label comes from the catalog, so the menu
    // bar's lookup of the same id must agree with the row.
    let tokens = theme_tokens(TID);
    let rows = canvas_menu_entries(TID, &tokens, true, true, false, None, &shortcuts());
    for (id, row_label) in [
        ("cut", "Cut"),
        ("copy", "Copy"),
        ("paste", "Paste"),
        ("smart_paste", "Paste Special"),
        ("mirror_x", "Mirror X"),
        ("mirror_y", "Mirror Y"),
        ("delete_selection", "Delete"),
        ("open_preferences", "Preferences..."),
        ("edit_selected_object_properties", "Properties..."),
    ] {
        assert_eq!(
            crate::menu_bar::cmd_label(id, "<missing>"),
            row_label,
            "{id}"
        );
        assert!(has_row(&rows, row_label), "{id} row {row_label:?}");
    }

    // Place / Align flyouts: id-backed and labelled by the catalog.
    for (action, row_label) in [
        (A::DrawWire, "Wire"),
        (A::PlacePort, "Port"),
        (A::AlignLeft, "Align Left"),
        (A::AlignToGrid, "Align To Grid"),
    ] {
        assert!(id_for_action(&action).is_some(), "{action:?} lost its id");
        assert_eq!(action_label(&action, "<missing>"), row_label);
    }
}

/// The Arrange launcher appears only when the selection holds a drawing, and
/// its submenu rows reorder drawings (#1583).
#[test]
fn canvas_menu_offers_arrange_only_for_a_drawing_selection() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let with = canvas_menu_entries(TID, &tokens, true, false, true, None, &sc);
    let without = canvas_menu_entries(TID, &tokens, true, false, false, None, &sc);
    assert_eq!(customs(&with), customs(&without) + 1);

    let rows = super::submenu::arrange_entries();
    for label in [
        "Bring to Front",
        "Send to Back",
        "Bring Forward",
        "Send Backward",
    ] {
        assert!(has_row(&rows, label), "missing {label}");
    }
}
