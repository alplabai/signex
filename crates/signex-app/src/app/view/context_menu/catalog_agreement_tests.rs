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

use super::*;

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
            }
            None if SURFACE_LOCAL.iter().any(|(label, _)| *label == item.label) => {}
            None => unlisted.push(format!("{origin}: {:?}", item.label)),
        }
    }
}

#[test]
fn command_rows_read_label_and_shortcut_from_the_catalog() {
    let tokens = theme_tokens(TID);
    let mut seen = 0;
    let mut unlisted = Vec::new();
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
            );
        }
        check_rows(
            "place flyout",
            &place_entries(TID, &hint),
            &app,
            &mut seen,
            &mut unlisted,
        );
        check_rows(
            "arrange flyout",
            &arrange_entries(&hint),
            &app,
            &mut seen,
            &mut unlisted,
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
    assert!(seen > 100, "the walk saw only {seen} command rows");
}
