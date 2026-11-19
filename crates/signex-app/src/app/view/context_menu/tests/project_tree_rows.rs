//! #20 — the Projects tree's right-click menus: the exact rows per node
//! role, the Planned flyout, the library's Add New flyout, the Add New to
//! Project flyout, and the strong / danger / planned row looks.

use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};
use signex_widgets::tree_view::{TreeIcon, TreeNode, key_of};

use super::TID;
use crate::app::view::context_menu::project_tree::library::library_add_new_entries;
use crate::app::view::context_menu::project_tree::planned::planned_entries;
use crate::app::view::context_menu::submenu::add_new_entries;
use crate::app::{
    ContextMenuMsg, ContextSubmenu, Message, ProjectTreeAction, ProjectTreeContextMenuState, Signex,
};
use crate::menu_bar::MenuMessage;

const SEP: &str = "---";

/// The text of a row's badge pill, if it has one.
fn badge_of(item: &DropdownItem<Message>) -> Option<&str> {
    item.badge.as_ref().map(signex_widgets::Label::as_str)
}

/// A project with a source document and a library, and the project dir's
/// `.git` probed absent so Enable Version Control is offered.
fn app_with_tree() -> Signex {
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/p", &[]);
    app.document_state.panel_ctx.project_tree = vec![TreeNode::branch(
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
    )];
    let dir = std::path::Path::new("/p");
    app.document_state
        .project_fs
        .facts
        .set_exists(crate::app::state::project_fs::git_marker(dir), false);
    app
}

/// The menu of the tree row at `path`.
fn rows_at(app: &Signex, path: &[usize]) -> Vec<DropdownEntry<Message>> {
    let key = key_of(&app.document_state.panel_ctx.project_tree, path).expect("the row exists");
    let ctx = ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(key),
        window: None,
    };
    app.project_tree_entries(&ctx).expect("the menu has rows")
}

/// Labels in order, a separator as [`SEP`].
fn layout(rows: &[DropdownEntry<Message>]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row {
            DropdownEntry::Item(item) => Some(item.label.clone()),
            DropdownEntry::Separator => Some(SEP.to_string()),
            _ => None,
        })
        .collect()
}

fn item<'r>(rows: &'r [DropdownEntry<Message>], label: &str) -> &'r DropdownItem<Message> {
    rows.iter()
        .find_map(|row| match row {
            DropdownEntry::Item(item) if item.label == label => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing row {label:?}"))
}

/// The project root is grouped add / check and sync / file / settings /
/// close, then the Planned launcher; no planned row sits at the top level.
#[test]
fn the_project_root_menu_has_the_m20_rows_in_order() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0]);
    assert_eq!(
        layout(&rows),
        [
            "Add New to Project",
            "Add Existing to Project...",
            SEP,
            "Validate Project",
            "Update PCB Document",
            SEP,
            "Save",
            "Rename...",
            "Explore",
            SEP,
            "Project Options...",
            "Enable Version Control...",
            SEP,
            "Close Project Documents",
            "Close Project",
            SEP,
            "Planned",
        ]
    );
    // Every row but Close Project Documents carries a glyph.
    for row in &rows {
        if let DropdownEntry::Item(it) = row {
            assert_eq!(
                it.icon.is_some(),
                it.label != "Close Project Documents",
                "{:?}",
                it.label
            );
            assert!(!it.danger && !it.strong, "{:?}", it.label);
        }
    }
    let add_new = item(&rows, "Add New to Project");
    assert!(add_new.submenu && add_new.on_hover.is_some());
}

/// With `.git` present the root menu has no Enable Version Control row.
#[test]
fn the_project_root_offers_version_control_only_without_git() {
    let mut app = app_with_tree();
    app.document_state.project_fs.facts.set_exists(
        crate::app::state::project_fs::git_marker(std::path::Path::new("/p")),
        true,
    );
    let rows = rows_at(&app, &[0]);
    assert!(
        !layout(&rows)
            .iter()
            .any(|l| l == "Enable Version Control...")
    );
}

/// The Planned launcher reads muted and says why, yet opens its flyout on
/// hover and on click.
#[test]
fn the_planned_launcher_is_muted_but_opens_its_flyout() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0]);
    let planned = item(&rows, "Planned");
    assert!(planned.submenu);
    assert!(planned.disabled, "the launcher reads muted");
    assert_eq!(
        planned.tooltip.as_deref(),
        Some("Planned — not available yet")
    );
    assert!(matches!(
        planned.on_hover,
        Some((
            Message::ContextMenu(ContextMenuMsg::SubmenuHover(ContextSubmenu::ProjectPlanned)),
            Message::ContextMenu(ContextMenuMsg::SubmenuLeave),
        ))
    ));
    assert!(matches!(
        planned.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
            ContextSubmenu::ProjectPlanned
        )))
    ));
}

/// The Planned flyout lists the five planned project features, each greyed
/// with the "planned" pill and publishing nothing.
#[test]
fn the_planned_flyout_lists_the_planned_rows_greyed_with_the_pill() {
    let rows = planned_entries(signex_i18n::I18n::en_us_ref(), TID);
    assert_eq!(
        layout(&rows),
        [
            "Make Project Available Online...",
            "Variants...",
            "Project Packager...",
            "Project Releaser...",
            "Share...",
        ]
    );
    for row in &rows {
        let DropdownEntry::Item(it) = row else {
            panic!("only item rows");
        };
        assert!(it.disabled && it.on_press.is_none(), "{:?}", it.label);
        assert_eq!(badge_of(it), Some("planned"), "{:?}", it.label);
        assert!(it.icon.is_some(), "{:?}", it.label);
    }
}

/// A document leaf: Open (strong) first, Remove from Project... last in the
/// danger ink, live, opening the Remove dialog.
#[test]
fn the_document_leaf_menu_ends_with_a_live_danger_remove_row() {
    let app = app_with_tree();
    let key = key_of(&app.document_state.panel_ctx.project_tree, &[0, 0, 0]).expect("the leaf");
    let rows = rows_at(&app, &[0, 0, 0]);
    assert_eq!(
        layout(&rows),
        [
            "Open",
            "Explore",
            "Rename...",
            SEP,
            "Print...",
            "Show Differences...",
            SEP,
            "Remove from Project...",
        ]
    );
    let open = item(&rows, "Open");
    assert!(open.strong && !open.danger);
    assert!(matches!(
        &open.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::OpenNode(k)
        ))) if *k == key
    ));
    let remove = item(&rows, "Remove from Project...");
    assert!(remove.danger && !remove.strong && !remove.disabled);
    assert!(matches!(
        &remove.on_press,
        Some(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::OpenRemoveDialog(k)
        ))) if *k == key
    ));
    let diff = item(&rows, "Show Differences...");
    assert!(diff.disabled && diff.on_press.is_none());
    assert_eq!(badge_of(diff), Some("planned"));
    // Only one strong row and one danger row.
    let flagged = |f: fn(&DropdownItem<Message>) -> bool| {
        rows.iter()
            .filter(|row| matches!(row, DropdownEntry::Item(it) if f(it)))
            .count()
    };
    assert_eq!(flagged(|it| it.strong), 1);
    assert_eq!(flagged(|it| it.danger), 1);
}

/// A library: one Add New launcher instead of three flat rows, then
/// pricing, the tree toggles, and Remove from Project... in the danger ink.
#[test]
fn the_library_menu_has_one_add_new_launcher() {
    let app = app_with_tree();
    let rows = rows_at(&app, &[0, 1, 0]);
    // Power.snxlib is a collapsed leaf.
    assert_eq!(
        layout(&rows),
        [
            "Add New",
            SEP,
            "Refresh All Pricing",
            SEP,
            "Expand",
            "Refresh",
            SEP,
            "Remove from Project...",
        ]
    );
    let add_new = item(&rows, "Add New");
    assert!(add_new.submenu && !add_new.disabled);
    assert!(matches!(
        add_new.on_hover,
        Some((
            Message::ContextMenu(ContextMenuMsg::SubmenuHover(
                ContextSubmenu::AddNewToLibrary
            )),
            _,
        ))
    ));
    assert!(item(&rows, "Remove from Project...").danger);
}

/// The library's Add New flyout runs the same menu messages the three flat
/// rows ran.
#[test]
fn the_library_add_new_flyout_creates_component_symbol_and_footprint() {
    let rows = library_add_new_entries(signex_i18n::I18n::en_us_ref(), TID);
    let mut expected = vec!["Component", "Symbol Library"];
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        expected.push("Footprint Library");
    }
    assert_eq!(layout(&rows), expected);
    assert!(matches!(
        item(&rows, "Component").on_press,
        Some(Message::Menu(MenuMessage::AddLibraryComponent))
    ));
    assert!(matches!(
        item(&rows, "Symbol Library").on_press,
        Some(Message::Menu(MenuMessage::AddLibrarySymbol))
    ));
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        assert!(matches!(
            item(&rows, "Footprint Library").on_press,
            Some(Message::Menu(MenuMessage::AddLibraryFootprint))
        ));
    }
}

/// Add New to Project: the documents, the libraries, then the planned
/// project files, with Constraint File and VHDL File kept as planned rows.
#[test]
fn the_add_new_to_project_flyout_is_grouped_documents_libraries_planned() {
    let rows = add_new_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        vec!["proj".to_string()],
    );
    let mut expected = vec![
        "Schematic",
        "PCB",
        SEP,
        "Component Library",
        "Symbol Library",
    ];
    if crate::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        expected.push("PCB Library");
    }
    expected.extend([
        SEP,
        "Design Notebook",
        "Output Job",
        SEP,
        "Constraint File",
        "VHDL File",
    ]);
    assert_eq!(layout(&rows), expected);
    for planned in [
        "Design Notebook",
        "Output Job",
        "Constraint File",
        "VHDL File",
    ] {
        let it = item(&rows, planned);
        assert!(it.disabled && it.on_press.is_none(), "{planned}");
        assert_eq!(badge_of(it), Some("planned"), "{planned}");
    }
}

/// One icon column: a row without a glyph starts its label where a row
/// with one does.
#[test]
fn a_row_without_an_icon_lines_its_label_up_with_the_others() {
    let app = app_with_tree();
    let key = key_of(&app.document_state.panel_ctx.project_tree, &[0]).expect("the root");
    let ctx = ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(key),
        window: None,
    };
    let card = app.view_project_tree_context_menu(&ctx);
    let texts = crate::layout_probe::lay_out_texts(card.element, iced::Size::new(1000.0, 900.0));
    let bare = crate::layout_probe::find(&texts, "Close Project Documents").bounds;
    let with_icon = crate::layout_probe::find(&texts, "Close Project").bounds;
    let planned = crate::layout_probe::find(&texts, "Planned").bounds;
    assert!((bare.x - with_icon.x).abs() < 0.5, "{bare:?} {with_icon:?}");
    assert!(
        (planned.x - with_icon.x).abs() < 0.5,
        "{planned:?} {with_icon:?}"
    );
}
