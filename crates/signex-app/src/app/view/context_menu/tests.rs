//! Data-to-view tests (iced-rust skill §10): assert on the pure context-
//! menu entry builders — no GPU, no window, no widget tree. Each builder
//! turns app state into a `Vec<DropdownEntry<Message>>`; the widget that
//! renders those rows is tested in `signex-widgets`.

use super::*;

use super::items::{dd_disabled, dd_msg};
use super::menus::{CanvasSelection, CanvasShortcuts, canvas_menu_entries, tab_menu_entries};
use super::project_tree::{TreeNodeRole, tree_node_role};
use super::submenu::{align_entries, align_gate, place_entries};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};
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

fn selected() -> CanvasSelection {
    CanvasSelection {
        has_selection: true,
        ..Default::default()
    }
}

fn shortcuts() -> CanvasShortcuts {
    CanvasShortcuts::resolve(|id| {
        match id {
            "find" => "Ctrl+F",
            "find_similar_objects" => "Shift+F",
            "cut" => "Ctrl+X",
            "copy" => "Ctrl+C",
            "paste" => "Ctrl+V",
            "smart_paste" => "Shift+Ctrl+V",
            "rotate_counterclockwise" => "Space",
            "mirror_x" => "X",
            "mirror_y" => "Y",
            "delete_selection" => "Del",
            _ => "",
        }
        .to_string()
    })
}

#[test]
fn edit_rows_show_the_resolved_keymap_shortcuts() {
    let mut sc = shortcuts();
    sc.set("rotate_counterclockwise", "R");
    sc.set("delete_selection", "Delete");
    sc.set("mirror_x", "Shift+X");
    sc.set("mirror_y", "Shift+Y");
    let tokens = theme_tokens(TID);
    let entries = canvas_menu_entries(TID, &tokens, selected(), None, None, &sc);
    let shortcut_of = |label: &str| {
        entries.iter().find_map(|e| match e {
            DropdownEntry::Item(it) if it.label == label => Some(it.shortcut.clone()),
            _ => None,
        })
    };
    assert_eq!(shortcut_of("Rotate").flatten().as_deref(), Some("R"));
    assert_eq!(shortcut_of("Delete").flatten().as_deref(), Some("Delete"));
    let mirror_x = crate::menu_bar::cmd_label("mirror_x", "Mirror X");
    let mirror_y = crate::menu_bar::cmd_label("mirror_y", "Mirror Y");
    assert_eq!(shortcut_of(&mirror_x).flatten().as_deref(), Some("Shift+X"));
    assert_eq!(shortcut_of(&mirror_y).flatten().as_deref(), Some("Shift+Y"));
}

#[test]
fn classic_profile_resolves_rotate_through_the_real_lookup() {
    // The same `Signex::keymap_shortcut_label` `view_context_menu` calls,
    // fed by a keymap compiled from the bundled Classic profile.
    let (mut app, _boot) = crate::app::Signex::new();
    let mut set = crate::keymap::ShortcutProfileSet::built_ins().expect("built-ins parse");
    set.set_active_profile("classic").expect("classic exists");
    app.ui_state.active_keymap = std::sync::Arc::new(set.compile_active());
    assert_eq!(app.keymap_shortcut_label("rotate_counterclockwise"), "R");
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

/// #1257 - the row is live and routes to the Find Similar action, with or
/// without a selection.
#[test]
fn find_similar_row_is_enabled_and_routes_to_its_action() {
    let tokens = theme_tokens(TID);
    for has_selection in [false, true] {
        let rows = canvas_menu_entries(
            TID,
            &tokens,
            CanvasSelection {
                has_selection,
                ..CanvasSelection::default()
            },
            None,
            None,
            &shortcuts(),
        );
        assert!(!disabled_of(&rows, "Find Similar Objects..."));
        let item = rows
            .iter()
            .find_map(|e| match e {
                DropdownEntry::Item(it) if it.label == "Find Similar Objects..." => Some(it),
                _ => None,
            })
            .unwrap();
        assert_eq!(item.shortcut.as_deref(), Some("Shift+F"));
        assert!(matches!(
            item.on_press,
            Some(Message::ContextMenu(ContextMenuMsg::Action(
                ContextAction::FindSimilar
            )))
        ));
    }
}

#[test]
fn canvas_menu_grows_and_gates_on_selection() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let with = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: true,
            child_sheet_selected: false,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
    let without = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: false,
            child_sheet_selected: false,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );

    // Clipboard + find rows are always present.
    for row in ["Find", "Cut", "Copy", "Paste", "Paste Special"] {
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

fn project_options_row(entries: &[DropdownEntry<Message>]) -> &DropdownItem<Message> {
    entries
        .iter()
        .find_map(|e| match e {
            DropdownEntry::Item(it) if it.label == "Project Options..." => Some(it),
            _ => None,
        })
        .expect("Project Options row")
}

#[test]
fn canvas_project_options_opens_the_active_project_options() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let live = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection::default(),
        None,
        Some(vec!["7".to_string()]),
        &sc,
    );
    let row = project_options_row(&live);
    assert!(!row.disabled);
    assert!(matches!(
        &row.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            crate::app::ProjectTreeAction::OpenProjectOptions(path)
        ))) if path.as_slice() == ["7".to_string()]
    ));
}

#[test]
fn canvas_project_options_is_greyed_without_a_project_and_says_nothing_planned() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let none = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: false,
            child_sheet_selected: false,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
    let row = project_options_row(&none);
    assert!(row.disabled);
    assert!(row.on_press.is_none());
    assert!(row.tooltip.is_none());
}

#[test]
fn canvas_child_sheet_adds_open_row() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let plain = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: false,
            child_sheet_selected: false,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
    let child = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: false,
            child_sheet_selected: true,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
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
    let one = align_entries(TID, 1, &|_| String::new());
    let three = align_entries(TID, 3, &|_| String::new());
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
fn place_menu_greys_only_the_planned_tools() {
    let e = place_entries(TID, &|_| String::new());
    assert!(has_row(&e, "Wire"));
    assert!(has_row(&e, "Part"));
    // Placement actions are always available, except the planned tools
    // (#1249), which are greyed with the shared tooltip.
    for entry in &e {
        if let DropdownEntry::Item(it) = entry {
            let planned = matches!(
                it.label.as_str(),
                "Differential Pair"
                    | "Parameter Set"
                    | "Blanket"
                    | "Signal Harness"
                    | "Harness Connector"
                    | "Harness Entry"
                    | "Device Sheet Symbol"
            );
            assert_eq!(it.disabled, planned, "{}", it.label);
            assert_eq!(
                it.tooltip.as_deref(),
                planned.then_some(crate::active_bar::PLANNED_TOOLTIP),
                "{}",
                it.label
            );
        }
    }
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
        labels(&place_entries(TID, &|_| String::new())),
        [
            "Wire",
            "Bus",
            "Bus Entry",
            "Net Label",
            "Junction",
            "Port",
            "Off Sheet Connector",
            "Place GND power port",
            "Place VCC power port",
            "Place +5 power port",
            "Place +12 power port",
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
        labels(&align_entries(TID, 3, &|_| String::new())),
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
    let rows = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: true,
            child_sheet_selected: true,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &shortcuts(),
    );
    for expected in [
        "Find",
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
    let rows = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: true,
            child_sheet_selected: true,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &shortcuts(),
    );
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

// #1378 — the hardcoded fallback is for commands the keymap is silent
// about, not for ones the user unbound.
#[test]
fn a_command_the_keymap_does_not_bind_gets_no_shortcut_hint() {
    use crate::keymap::{
        AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
        ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
    };
    let copy = AppCommandId::new("copy").unwrap();
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![ShortcutBinding {
            action: ShortcutBindingAction::Unbind(copy),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::parse("Ctrl+C").unwrap()],
        }],
    };
    let (mut app, _boot) = crate::app::Signex::new();
    app.ui_state.active_keymap = std::sync::Arc::new(CompiledKeymap::compile(&profile));

    assert_eq!(app.keymap_shortcut_label("copy"), "");
    assert_eq!(
        app.keymap_shortcut_label("cut"),
        "",
        "a command the active keymap never binds has no hint either"
    );
}

/// The Arrange launcher appears only when the selection holds a drawing, and
/// its submenu rows reorder drawings (#1583).
#[test]
fn canvas_menu_offers_arrange_only_for_a_drawing_selection() {
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let with = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: true,
            child_sheet_selected: false,
            drawing_selected: true,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
    let without = canvas_menu_entries(
        TID,
        &tokens,
        CanvasSelection {
            has_selection: true,
            child_sheet_selected: false,
            drawing_selected: false,
            placed_part_selected: false,
        },
        None,
        None,
        &sc,
    );
    assert_eq!(customs(&with), customs(&without) + 1);

    let rows = super::submenu::arrange_entries(&|_| String::new());
    for label in [
        "Bring to Front",
        "Send to Back",
        "Bring Forward",
        "Send Backward",
    ] {
        assert!(has_row(&rows, label), "missing {label}");
    }
}

/// The placed-part actions (#1301, #1262, #1260) show for one selected
/// non-power symbol only, under exactly these labels, and each row carries
/// its own action.
#[test]
fn canvas_menu_offers_the_placed_part_actions_for_a_placed_part_only() {
    use crate::library::PlacedPartAction as A;
    let tokens = theme_tokens(TID);
    let sc = shortcuts();
    let part = CanvasSelection {
        placed_part_selected: true,
        ..selected()
    };
    let with = canvas_menu_entries(TID, &tokens, part, None, None, &sc);
    let without = canvas_menu_entries(TID, &tokens, selected(), None, None, &sc);

    for (label, action) in [
        ("Open Datasheet", A::OpenDatasheet),
        ("Edit Symbol", A::EditSymbol),
        ("Change Component\u{2026}", A::ChangeComponent),
    ] {
        assert!(!has_row(&without, label), "{label} without a placed part");
        let message = with
            .iter()
            .find_map(|e| match e {
                DropdownEntry::Item(it) if it.label == label => it.on_press.clone(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing or inert row {label:?}"));
        assert!(
            matches!(
                message,
                Message::ContextMenu(ContextMenuMsg::Action(ContextAction::PlacedPart(a))) if a == action
            ),
            "{label} sent the wrong message"
        );
    }
    // Nothing else moved: the rest of the menu is identical.
    assert_eq!(labels(&with).len(), labels(&without).len() + 3);
}
