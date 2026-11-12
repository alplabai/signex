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
//! A command id behaves the same on every surface (#1727), so a live row
//! that names an id must send that id the way the palette does: exactly
//! `bridge::resolve(id)`, or `ContextAction::Command` for that id. A live
//! row must also be one `is_enabled` allows in the context the app really
//! has in that selection state (`Signex::command_context`), or the click
//! would be dropped silently. Any other row fails here, naming the row.
//!
//! This does not run a row's handler: what each row does is pinned by
//! `tests/row_effects.rs`.

use super::*;

use crate::app::command::{CommandArgs, CommandContext, InvocationContext, bridge, is_enabled};
use crate::keymap::AppCommandId;
use std::collections::BTreeSet;

use super::menus::{CanvasSelection, CanvasShortcuts, canvas_menu_entries};
use super::submenu::{align_entries, arrange_entries, place_entries};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::{TreeIcon, TreeNode, key_of};

const TID: ThemeId = ThemeId::GraphiteDark;

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
    // #1312: runs for the clicked project, which the palette cannot name.
    ("Update PCB Document", "tree-addressed"),
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

/// A real app with a sheet and the canvas selection `bits` describe (one
/// item per set bit, or a wire for a bare selection), plus the selection
/// shape and project key the view derives from it.
fn app_in_selection(profile: &str, bits: u8) -> (Signex, CanvasSelection, Option<Vec<String>>) {
    use signex_types::schematic::{Point, SelectedItem, SelectedKind, Symbol};
    let mut app = app_with_keymap(profile);
    let part = uuid::Uuid::from_u128(1);
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::from_u128(2),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: part,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: Point::new(10.0, 10.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = std::path::PathBuf::from("/p/catalog-agreement.snxsch");
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "catalog-agreement".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    let mut selected = Vec::new();
    for (bit, kind, uuid) in [
        (2, SelectedKind::ChildSheet, 3),
        (4, SelectedKind::Drawing, 4),
    ] {
        if bits & bit != 0 {
            selected.push(SelectedItem::new(uuid::Uuid::from_u128(uuid), kind));
        }
    }
    if bits & 8 != 0 {
        selected.push(SelectedItem::new(part, SelectedKind::Symbol));
    }
    if selected.is_empty() && bits & 1 != 0 {
        selected.push(SelectedItem::new(
            uuid::Uuid::from_u128(5),
            SelectedKind::Wire,
        ));
    }
    app.interaction_state.active_canvas_mut().selected = selected;
    let (selection, project_key) = app.canvas_menu_inputs(
        app.ui_state
            .main_window_id
            .expect("the app opens its main window"),
    );
    (app, selection, project_key)
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

/// What one walk over the rows found.
#[derive(Default)]
struct Walk {
    seen: usize,
    /// Rows with no command id that are not surface-local.
    unlisted: Vec<String>,
    /// Live rows that neither send the bridge's message nor dispatch their id.
    diverging: BTreeSet<String>,
    /// Live rows whose command `is_enabled` refuses in the row's context.
    dropped: BTreeSet<String>,
}

fn check_rows(
    origin: &str,
    rows: &[DropdownEntry<Message>],
    app: &Signex,
    ctx: &CommandContext,
    walk: &mut Walk,
) {
    for entry in rows {
        let DropdownEntry::Item(item) = entry else {
            continue;
        };
        // A live submenu launcher opens a flyout and runs no command (it was
        // a `Custom` row, skipped above, until it became an item row).
        if item.submenu && item.on_hover.is_some() {
            continue;
        }
        match item.command_id {
            Some(id) => {
                walk.seen += 1;
                let meta = crate::keymap::metadata_for_str(id).unwrap_or_else(|| {
                    panic!("{origin}: row {:?} names unknown id {id}", item.label)
                });
                assert_eq!(
                    item.label,
                    meta.menu_label_in(signex_i18n::I18n::en_us_ref()),
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
                    check_live_row(origin, id, pressed, ctx, walk);
                }
            }
            None if SURFACE_LOCAL.iter().any(|(label, _)| *label == item.label) => {}
            None => walk.unlisted.push(format!("{origin}: {:?}", item.label)),
        }
    }
}

/// A live row of command `id` runs it as the palette does, and the command
/// is enabled where the row is shown.
fn check_live_row(
    origin: &str,
    id: &str,
    pressed: &Message,
    ctx: &CommandContext,
    walk: &mut Walk,
) {
    let command = AppCommandId::new(id).expect("catalog id is well formed");
    if !is_enabled(&command, ctx) {
        walk.dropped.insert(format!("{origin}: {id}"));
    }
    let dispatches_the_id = match pressed {
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: dispatched,
            project,
        })) => {
            let args = CommandArgs::in_context(InvocationContext {
                origin: None,
                project: project.clone(),
            });
            dispatched.as_str() == id && bridge::core_to_message(&command, &args).is_some()
        }
        _ => false,
    };
    if dispatches_the_id {
        return;
    }
    let expected = bridge::core_to_message(&command, &CommandArgs::none())
        .map_or_else(|| "<bridge has no arm>".to_string(), |m| format!("{m:?}"));
    let actual = format!("{pressed:?}");
    if actual != expected {
        walk.diverging.insert(format!(
            "{origin}: {id} sends {actual}, bridge::resolve gives {expected}"
        ));
    }
}

#[test]
fn command_rows_read_label_and_shortcut_from_the_catalog() {
    let mut walk = Walk::default();
    for profile in ["altium", "classic"] {
        let app = app_with_keymap(profile);
        let hint = |id: &str| app.keymap_shortcut_label(id);
        let sc = CanvasShortcuts::resolve(hint);

        for bits in 0..16u8 {
            let (app, selection, derived_key) = app_in_selection(profile, bits);
            let ctx = app.command_context(None);
            for project_key in [derived_key, Some(vec!["proj".to_string()])] {
                let rows = canvas_menu_entries(TID, selection, None, project_key, &sc);
                check_rows(
                    &format!("canvas {profile} {selection:?}"),
                    &rows,
                    &app,
                    &ctx,
                    &mut walk,
                );
            }
        }
        let flyout_ctx = CommandContext {
            has_schematic: true,
            has_selection: true,
            has_drawing_selection: true,
            ..Default::default()
        };
        for selected in [1, 3] {
            check_rows(
                "align flyout",
                &align_entries(TID, selected, &hint),
                &app,
                &flyout_ctx,
                &mut walk,
            );
        }
        check_rows(
            "place flyout",
            &place_entries(TID, &hint),
            &app,
            &flyout_ctx,
            &mut walk,
        );
        check_rows(
            "arrange flyout",
            &arrange_entries(&hint),
            &app,
            &flyout_ctx,
            &mut walk,
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
                    window: None,
                };
                let rows = app
                    .project_tree_entries(&ctx)
                    .unwrap_or_else(|| panic!("no menu for {path:?}"));
                check_rows(
                    &format!("project tree {profile} {path:?} schematic={has_schematic}"),
                    &rows,
                    &app,
                    &app.command_context(None),
                    &mut walk,
                );
            }
        }
    }
    assert!(
        walk.unlisted.is_empty(),
        "rows with no command id that are not listed as surface-local; give \
         each its AppCommandId so the catalog owns its label and shortcut \
         (#271): {:#?}",
        walk.unlisted
    );
    assert!(
        walk.diverging.is_empty(),
        "live command rows that neither send bridge::resolve(id) nor dispatch \
         their id as ContextAction::Command (#1727): {:#?}",
        walk.diverging
    );
    assert!(
        walk.dropped.is_empty(),
        "live command rows whose command is_enabled refuses where the row is \
         shown, so a click would do nothing; grey the row (#1727): {:#?}",
        walk.dropped
    );
    assert!(
        walk.seen > 100,
        "the walk saw only {} command rows",
        walk.seen
    );
}
