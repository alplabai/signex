//! #271: every context-menu row that runs a command reads its label and
//! shortcut from the command catalog, so the menu bar, the Active Bar and
//! the context menus cannot drift.
//!
//! The walk covers the canvas menu (every selection shape), its Place /
//! Align / Arrange flyouts and the project-tree menu for each node role. A
//! row that carries a command id must show the catalog's menu text and the
//! active keymap's hint for that id. A row with no id must be one of the
//! surface-local rows below, so a literal label reintroduced for a command
//! fails here instead of drifting quietly.
//!
//! It also compares each row's `on_press` with `bridge::resolve(id)` (#1727).
//! `SURFACE_ACTION` exists because context-menu rows still send their own
//! `ContextAction` / `ActiveBar` messages instead of dispatching their
//! `AppCommandId` (Command Registry #278 is not yet extended to row actions),
//! so its pins are a migration record, not a design endorsement.

use super::*;

use crate::keymap::AppCommandId;
use std::collections::BTreeMap;

use super::menus::{CanvasSelection, CanvasShortcuts, canvas_menu_entries};
use super::submenu::{align_entries, arrange_entries, place_entries};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::{TreeIcon, TreeNode, key_of};

const TID: ThemeId = ThemeId::CatppuccinMocha;

/// Rows that run no catalog command, with the reason each stays local.
/// A row not listed here and carrying no command id fails the walk.
const SURFACE_LOCAL: &[(&str, &str)] = &[
    // Place flyout power ports: the PlacePower* family becomes one
    // `place_power_port(style)` command (audit §4), so no per-row ids.
    (
        "GND Power Port",
        "power-port family, awaiting place_power_port(style)",
    ),
    (
        "VCC Power Port",
        "power-port family, awaiting place_power_port(style)",
    ),
    (
        "+5 Power Port",
        "power-port family, awaiting place_power_port(style)",
    ),
    (
        "+12 Power Port",
        "power-port family, awaiting place_power_port(style)",
    ),
    // Project tree: the Print row keeps its own wording (menu text per
    // surface is a later concern).
    ("Print...", "tree-addressed wording, no per-surface id"),
    // Canvas: planned stubs that run nothing yet.
    ("Clear Filter", "planned, no behaviour"),
    ("Part Actions", "planned submenu stub"),
    ("Sheet Actions", "planned submenu stub"),
    ("References", "planned submenu stub"),
    ("Unions", "planned submenu stub"),
    ("Snippets", "planned submenu stub"),
    ("Cross Probe", "planned, no behaviour"),
    ("Comment...", "planned, no behaviour"),
    ("Pin Mapping...", "planned, no behaviour"),
    ("Supplier Links...", "planned, no behaviour"),
    // Project tree: every action is addressed by a tree key or path the
    // palette cannot supply.
    (
        "Make Project Available Online...",
        "planned, tree-addressed",
    ),
    ("Validate Project", "tree-addressed"),
    ("Add Existing to Project...", "tree-addressed"),
    ("Rename...", "tree-addressed"),
    ("Close Project Documents", "tree-addressed"),
    ("Close Project", "tree-addressed"),
    ("Explore", "tree-addressed"),
    ("Variants...", "planned, tree-addressed"),
    ("Project Packager...", "planned, tree-addressed"),
    ("Project Releaser...", "planned, tree-addressed"),
    ("Share...", "planned, tree-addressed"),
    ("Enable Version Control...", "tree-addressed"),
    ("Add New ▸ Component", "library-node action"),
    ("Add New ▸ Symbol Library", "library-node action"),
    ("Add New ▸ Footprint Library", "library-node action"),
    ("Refresh All Pricing", "library-node action"),
    ("Remove from Project...", "tree-addressed"),
    ("Remove from Project", "tree-addressed"),
    ("Open", "tree-addressed"),
    ("Show Differences...", "planned, tree-addressed"),
    ("Collapse", "tree-addressed"),
    ("Expand", "tree-addressed"),
    ("Expand All", "tree-addressed"),
    ("Collapse All", "tree-addressed"),
    ("Refresh", "tree-addressed"),
];

/// `(command id, the message the row sends as Debug text, reason)` for rows
/// that deliberately send something other than `bridge::resolve(id)`. Pinning
/// the sent message means a row that changes again also fails, and an entry
/// no row produces any more fails as stale.
const SURFACE_ACTION: &[(&str, &str, &str)] = &[
    (
        "align_bottom",
        "ContextMenu(Action(ActiveBar(AlignBottom)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_horizontal_centers",
        "ContextMenu(Action(ActiveBar(AlignHorizontalCenters)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_left",
        "ContextMenu(Action(ActiveBar(AlignLeft)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_right",
        "ContextMenu(Action(ActiveBar(AlignRight)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_to_grid",
        "ContextMenu(Action(ActiveBar(AlignToGrid)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_top",
        "ContextMenu(Action(ActiveBar(AlignTop)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "align_vertical_centers",
        "ContextMenu(Action(ActiveBar(AlignVerticalCenters)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "bring_drawings_forward",
        "ContextMenu(Action(ArrangeDrawings(Forward)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "bring_drawings_to_front",
        "ContextMenu(Action(ArrangeDrawings(ToFront)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "change_component",
        "ContextMenu(Action(PlacedPart(ChangeComponent)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "copy",
        "ContextMenu(Action(Copy))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "cut",
        "ContextMenu(Action(Cut))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "delete_selection",
        "ContextMenu(Action(Delete))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "distribute_horizontally",
        "ContextMenu(Action(ActiveBar(DistributeHorizontally)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "distribute_vertically",
        "ContextMenu(Action(ActiveBar(DistributeVertically)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "draw_bus",
        "ContextMenu(Action(ActiveBar(DrawBus)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "draw_wire",
        "ContextMenu(Action(ActiveBar(DrawWire)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "edit_selected_symbol_in_symbol_editor",
        "ContextMenu(Action(PlacedPart(EditSymbol)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "find_similar_objects",
        "ContextMenu(Action(FindSimilar))",
        "opens at the click point; the palette passes at: None",
    ),
    (
        "mirror_x",
        "ContextMenu(Action(MirrorX))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "mirror_y",
        "ContextMenu(Action(MirrorY))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "open_datasheet",
        "ContextMenu(Action(PlacedPart(OpenDatasheet)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "open_project_options",
        "ContextMenu(ProjectTreeAction(OpenProjectOptions([\"proj\"])))",
        "tree-addressed: opens the options of the clicked project key, not the active document's",
    ),
    (
        "paste",
        "ContextMenu(Action(Paste))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "place_bus_entry",
        "ContextMenu(Action(ActiveBar(PlaceBusEntry)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_component",
        "ContextMenu(Action(ActiveBar(PlaceComponent)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_junction",
        "ContextMenu(Action(ActiveBar(PlaceJunction)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_net_label",
        "ContextMenu(Action(ActiveBar(PlaceNetLabel)))",
        "row clears a stale Global/Hierarchical pending_port and arms the NET ghost via handle_active_bar_action; the palette, key and Place menu (Tool(SelectTool(Label))) keep the stale port, so a click places a port (#1920)",
    ),
    (
        "place_no_erc",
        "ContextMenu(Action(ActiveBar(PlaceNoERC)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_note",
        "ContextMenu(Action(ActiveBar(PlaceNote)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_off_sheet_connector",
        "ContextMenu(Action(ActiveBar(PlaceOffSheetConnector)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_port",
        "ContextMenu(Action(ActiveBar(PlacePort)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_sheet_entry",
        "ContextMenu(Action(ActiveBar(PlaceSheetEntry)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_sheet_symbol",
        "ContextMenu(Action(ActiveBar(PlaceSheetSymbol)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_text_frame",
        "ContextMenu(Action(ActiveBar(PlaceTextFrame)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "place_text_string",
        "ContextMenu(Action(ActiveBar(PlaceTextString)))",
        "closes the menu, then runs the Active Bar handler directly",
    ),
    (
        "rotate_counterclockwise",
        "ContextMenu(Action(RotateSelected))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "save_document",
        "Menu(Save)",
        "Menu(Save) forwards to File(Save) (handlers/menu/file_commands.rs)",
    ),
    (
        "send_drawings_backward",
        "ContextMenu(Action(ArrangeDrawings(Backward)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "send_drawings_to_back",
        "ContextMenu(Action(ArrangeDrawings(ToBack)))",
        "closes the menu, then dispatches the same message",
    ),
    (
        "smart_paste",
        "ContextMenu(Action(SmartPaste))",
        "closes the menu, then dispatches the same message",
    ),
];

fn app_with_keymap(profile: &str) -> Signex {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    let mut set = crate::keymap::ShortcutProfileSet::built_ins().expect("built-ins parse");
    set.set_active_profile(profile).expect("profile exists");
    app.ui_state.active_keymap = std::sync::Arc::new(set.compile_active());
    app
}

/// A project, its source documents and a library: one node of each role.
fn project_tree() -> Vec<TreeNode> {
    vec![TreeNode::branch(
        "proj",
        TreeIcon::Folder,
        vec![
            TreeNode::branch(
                "Source Documents",
                TreeIcon::Folder,
                vec![TreeNode::leaf("a.snxsch", TreeIcon::SnxSchematic)],
            ),
            TreeNode::branch(
                "Libraries",
                TreeIcon::Folder,
                vec![TreeNode::leaf("Power.snxlib", TreeIcon::SnxLibrary)],
            ),
        ],
    )]
}

fn check_rows(
    origin: &str,
    rows: &[DropdownEntry<Message>],
    app: &Signex,
    seen: &mut usize,
    unlisted: &mut Vec<String>,
    diverging: &mut BTreeMap<(String, String), String>,
) {
    for entry in rows {
        let DropdownEntry::Item(item) = entry else {
            continue;
        };
        match item.command_id {
            Some(id) => {
                *seen += 1;
                let meta = crate::keymap::metadata_for_str(id).unwrap_or_else(|| {
                    panic!("{origin}: row {:?} names unknown id {id}", item.label)
                });
                assert_eq!(
                    item.label,
                    meta.menu_label(),
                    "{origin}: {id} shows a label other than the catalog's"
                );
                let hint = app.keymap_shortcut_label(id);
                assert_eq!(
                    item.shortcut.as_deref().unwrap_or(""),
                    hint,
                    "{origin}: {id} shows a shortcut other than the keymap's"
                );
                // A disabled stub has no on_press; only live rows are compared.
                if let Some(pressed) = &item.on_press {
                    let command = AppCommandId::new(id).expect("catalog id is well formed");
                    let expected = crate::app::command::bridge::core_to_message(&command);
                    let expected = expected.map_or_else(
                        || "<bridge has no arm>".to_string(),
                        |message| format!("{message:?}"),
                    );
                    let actual = format!("{pressed:?}");
                    if actual != expected {
                        diverging.insert(
                            (id.to_string(), actual),
                            format!(
                                "{id}: bridge::resolve gives {expected} (first seen in {origin})"
                            ),
                        );
                    }
                }
            }
            None if SURFACE_LOCAL.iter().any(|(label, _)| *label == item.label) => {}
            None => unlisted.push(format!("{origin}: {:?}", item.label)),
        }
    }
}

/// Also compares each row's `on_press` with `bridge::resolve(id)` (#1727);
/// rows that still send their own action are pinned in `SURFACE_ACTION`
/// (a migration record until #278 covers row actions).
#[test]
fn command_rows_read_label_and_shortcut_from_the_catalog() {
    let tokens = theme_tokens(TID);
    let mut seen = 0;
    let mut unlisted = Vec::new();
    let mut diverging = BTreeMap::new();
    for profile in ["altium", "classic"] {
        let app = app_with_keymap(profile);
        let hint = |id: &str| app.keymap_shortcut_label(id);
        let sc = CanvasShortcuts::resolve(hint);

        for bits in 0..16u8 {
            let selection = CanvasSelection {
                has_selection: bits & 1 != 0,
                child_sheet_selected: bits & 2 != 0,
                drawing_selected: bits & 4 != 0,
                placed_part_selected: bits & 8 != 0,
            };
            for project_key in [None, Some(vec!["proj".to_string()])] {
                let rows = canvas_menu_entries(TID, &tokens, selection, None, project_key, &sc);
                check_rows(
                    &format!("canvas {profile} {selection:?}"),
                    &rows,
                    &app,
                    &mut seen,
                    &mut unlisted,
                    &mut diverging,
                );
            }
        }
        for selected in [1, 3] {
            check_rows(
                "align flyout",
                &align_entries(TID, selected, &hint),
                &app,
                &mut seen,
                &mut unlisted,
                &mut diverging,
            );
        }
        check_rows(
            "place flyout",
            &place_entries(TID, &hint),
            &app,
            &mut seen,
            &mut unlisted,
            &mut diverging,
        );
        check_rows(
            "arrange flyout",
            &arrange_entries(&hint),
            &app,
            &mut seen,
            &mut unlisted,
            &mut diverging,
        );

        for has_schematic in [false, true] {
            let mut app = app_with_keymap(profile);
            app.document_state.panel_ctx.project_tree = project_tree();
            app.document_state.panel_ctx.has_schematic = has_schematic;
            let tree = app.document_state.panel_ctx.project_tree.as_slice();
            for path in [vec![0], vec![0, 0], vec![0, 0, 0], vec![0, 1, 0]] {
                let key = key_of(tree, &path).expect("node exists");
                let ctx = crate::app::ProjectTreeContextMenuState {
                    x: 0.0,
                    y: 0.0,
                    key: Some(key),
                };
                let rows = app
                    .project_tree_entries(&ctx)
                    .unwrap_or_else(|| panic!("no menu for {path:?}"));
                check_rows(
                    &format!("project tree {profile} {path:?} schematic={has_schematic}"),
                    &rows,
                    &app,
                    &mut seen,
                    &mut unlisted,
                    &mut diverging,
                );
            }
        }
    }
    assert!(
        unlisted.is_empty(),
        "rows with no command id that are not listed as surface-local; give \
         each its AppCommandId so the catalog owns its label and shortcut \
         (#271): {unlisted:#?}"
    );
    let unexplained: Vec<_> = diverging
        .iter()
        .filter(|((id, actual), _)| {
            !SURFACE_ACTION
                .iter()
                .any(|(listed, pinned, _)| listed == id && pinned == actual)
        })
        .map(|((_, actual), detail)| format!("{detail}; the row sends {actual}"))
        .collect();
    assert!(
        unexplained.is_empty(),
        "command rows whose on_press differs from bridge::resolve(id) and are \
         not pinned in SURFACE_ACTION; make the row send the palette's \
         message, or pin it there with a reason (#1727): {unexplained:#?}"
    );
    let stale: Vec<_> = SURFACE_ACTION
        .iter()
        .filter(|(id, pinned, _)| !diverging.contains_key(&(id.to_string(), pinned.to_string())))
        .map(|(id, pinned, _)| format!("{id} => {pinned}"))
        .collect();
    assert!(
        stale.is_empty(),
        "SURFACE_ACTION entries no walked row produces any more (the row now \
         matches the bridge, was changed, or the id is gone); fix or delete \
         them: {stale:#?}"
    );
    assert!(seen > 100, "the walk saw only {seen} command rows");
}
