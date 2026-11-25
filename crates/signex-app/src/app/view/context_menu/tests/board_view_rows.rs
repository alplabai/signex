//! #2240 / #2223: the board canvas menu ends with a View group whose
//! Toggle Ratsnest row runs `toggle_ratsnest`, with or without a selection.
//! #2405: it also has the Place and Align flyouts, Undo and Redo, Find
//! Similar, Select Net, Highlight Net and Mirror, each live exactly when its
//! command is, and a read-only board greys every editing row.

use signex_widgets::active_bar_dropdown::DropdownEntry;

use super::{TID, board_ctx, disabled_of, row_message, shortcuts};
use crate::app::ReuseBlockMsg;
use crate::app::handlers::pcb_placement::PcbTargets;
use crate::app::view::context_menu::menus::pcb_canvas_menu_entries;
use crate::app::view::context_menu::menus::pcb_canvas_menu_with_blocks;
use crate::app::view::context_menu::submenu::board_place_entries;
use crate::app::{ContextAction, ContextMenuMsg, ContextSubmenu, Message};

/// #2410: Swap Pins and Swap Gates run their commands with two pads of one
/// unlocked component selected, Save as Reuse Block with board objects
/// selected, and each project block gets a Place row live with one unlocked
/// component selected; all greyed otherwise, and the View group still ends
/// the menu.
#[test]
fn the_board_menu_swaps_pins_and_saves_and_places_reuse_blocks() {
    let sc = shortcuts();
    let i18n = signex_i18n::I18n::en_us_ref();
    let picked = PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        copyable: true,
        pad_pair: true,
        one_footprint: true,
        footprints: 1,
        ..PcbTargets::default()
    };
    let blocks = vec!["amp".to_string()];
    let rows = pcb_canvas_menu_with_blocks(i18n, TID, &board_ctx(picked), None, &sc, &blocks);
    assert!(runs(row_message(&rows, "Swap Pins"), "swap_pins"));
    assert!(runs(row_message(&rows, "Swap Gates"), "swap_gates"));
    assert!(runs(
        row_message(&rows, "Save as Reuse Block"),
        "save_board_reuse_block"
    ));
    let place = rows.iter().find_map(|row| match row {
        DropdownEntry::Item(it) if it.label.contains("amp") => Some(it.on_press.clone()),
        _ => None,
    });
    assert!(
        matches!(
            &place,
            Some(Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::ReuseBlock(
                ReuseBlockMsg::PlaceRequested { name }
            )))))
                if name == "amp"
        ),
        "{place:?}"
    );
    assert!(
        matches!(&rows[rows.len() - 1], DropdownEntry::Item(it) if it.label == "Toggle Ratsnest")
    );

    let empty = PcbTargets {
        shown: true,
        ..PcbTargets::default()
    };
    let rows = pcb_canvas_menu_with_blocks(i18n, TID, &board_ctx(empty), None, &sc, &[]);
    // 2026-11-18: the empty-area menu has no selection rows.
    assert!(absent(&rows, "Swap Pins"));
    assert!(absent(&rows, "Save as Reuse Block"));
    assert!(
        row_message(&rows, "Place Reuse Block").is_none(),
        "greyed with no block"
    );
}

/// Whether the menu has no row `label` at all.
fn absent(rows: &[DropdownEntry<Message>], label: &str) -> bool {
    !rows
        .iter()
        .any(|row| matches!(row, DropdownEntry::Item(it) if it.label == label))
}

/// The labels of the menu's rows, in order.
fn labels(rows: &[DropdownEntry<Message>]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row {
            DropdownEntry::Item(it) => Some(it.label.clone()),
            _ => None,
        })
        .collect()
}

/// 2026-11-18 (maintainer request): a right-click on an empty area with
/// nothing selected shows the short menu, no selection or object row.
#[test]
fn the_empty_area_menu_is_short() {
    let empty = rows(PcbTargets {
        shown: true,
        ..PcbTargets::default()
    });
    assert_eq!(
        labels(&empty),
        [
            "Find",
            "Place",
            "Undo",
            "Redo",
            "Paste",
            "Paste Special",
            "Select All",
            "Board Sketch",
            "Place Reuse Block",
            "Toggle Ratsnest",
        ]
    );
}

/// 2026-11-18: with a footprint selected the footprint rows show and the
/// route rows do not; with a track the route rows show and the footprint
/// rows do not; the net rows need an object with a net.
#[test]
fn the_object_menu_shows_the_rows_of_the_selected_kind() {
    let footprint = rows(PcbTargets {
        net: true,
        ..footprint_picked()
    });
    for label in [
        "Find Similar Objects...",
        "Change Footprint...",
        "Swap Pins",
        "Swap Gates",
        "Arrange in Box",
        "Select Net",
        "Delete",
    ] {
        assert!(!absent(&footprint, label), "{label} on a footprint");
    }
    for label in ["Gloss Selected Routes", "Add Teardrops", "Lock", "Unlock"] {
        assert!(absent(&footprint, label), "{label} is a route row");
    }
    let track = rows(PcbTargets {
        shown: true,
        any: true,
        routes: true,
        lockable: true,
        copyable: true,
        ..PcbTargets::default()
    });
    for label in ["Gloss Selected Routes", "Add Teardrops", "Lock", "Unlock"] {
        assert!(!absent(&track, label), "{label} on a track");
    }
    for label in ["Change Footprint...", "Swap Pins", "Select Net"] {
        assert!(absent(&track, label), "{label} on a track without a net");
    }
    // Only the pointer over an object: the object rows, no selection rows.
    let hovered = rows(PcbTargets {
        shown: true,
        hovered: true,
        net: true,
        ..PcbTargets::default()
    });
    assert!(!absent(&hovered, "Find Similar Objects..."));
    assert!(!absent(&hovered, "Select Net"));
    assert!(absent(&hovered, "Delete"));
}

fn rows(board: PcbTargets) -> Vec<DropdownEntry<Message>> {
    pcb_canvas_menu_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        &board_ctx(board),
        None,
        &shortcuts(),
    )
}

fn runs(message: Option<Message>, command: &str) -> bool {
    matches!(
        message,
        Some(Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, project: None })))
            if id.as_str() == command
    )
}

/// An unlocked footprint selected on an editable board.
fn footprint_picked() -> PcbTargets {
    PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        deletable: true,
        copyable: true,
        footprints: 1,
        ..PcbTargets::default()
    }
}

#[test]
fn the_board_menu_ends_with_a_live_toggle_ratsnest_row() {
    for any in [false, true] {
        let board = PcbTargets {
            shown: true,
            any,
            ..PcbTargets::default()
        };
        let rows = rows(board);
        assert!(runs(
            row_message(&rows, "Toggle Ratsnest"),
            "toggle_ratsnest"
        ));
        // Its own group: a separator above it, and it is the last row.
        let n = rows.len();
        assert!(matches!(rows[n - 2], DropdownEntry::Separator));
        assert!(matches!(&rows[n - 1], DropdownEntry::Item(it) if it.label == "Toggle Ratsnest"));
    }
}

/// #1312: the board menu's Properties row shows the Properties panel, live
/// with anything selected.
#[test]
fn the_board_menu_properties_row_is_live_with_a_selection() {
    let picked = PcbTargets {
        shown: true,
        any: true,
        ..PcbTargets::default()
    };
    let rows_picked = rows(picked);
    assert!(runs(
        row_message(&rows_picked, "Properties..."),
        "edit_selected_object_properties"
    ));
    let empty = PcbTargets {
        shown: true,
        ..PcbTargets::default()
    };
    assert!(absent(&rows(empty), "Properties..."));
}

/// #2366: Change Footprint... runs `change_footprint` with an unlocked
/// footprint selected, and is greyed otherwise.
#[test]
fn the_board_menu_change_footprint_row_needs_an_unlocked_footprint() {
    let placeable = PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        footprints: 1,
        ..PcbTargets::default()
    };
    assert!(runs(
        row_message(&rows(placeable), "Change Footprint..."),
        "change_footprint"
    ));
    let locked = PcbTargets {
        shown: true,
        any: true,
        footprints: 1,
        ..PcbTargets::default()
    };
    let rows = rows(locked);
    assert!(row_message(&rows, "Change Footprint...").is_none());
    assert!(
        rows.iter()
            .any(|row| matches!(row, DropdownEntry::Item(it) if it.label == "Change Footprint...")),
        "the row is there, greyed"
    );
}

/// #2407: Lock runs `lock_board_items` with an unlocked track or via
/// selected, Unlock `unlock_board_items` with a locked one; each is greyed
/// otherwise.
#[test]
fn the_board_menu_lock_rows_follow_the_selected_tracks_and_vias() {
    let rows_unlocked = rows(PcbTargets {
        shown: true,
        any: true,
        lockable: true,
        routes: true,
        ..PcbTargets::default()
    });
    assert!(runs(
        row_message(&rows_unlocked, "Lock"),
        "lock_board_items"
    ));
    assert!(row_message(&rows_unlocked, "Unlock").is_none());
    let rows_locked = rows(PcbTargets {
        shown: true,
        any: true,
        unlockable: true,
        routes: true,
        ..PcbTargets::default()
    });
    assert!(runs(
        row_message(&rows_locked, "Unlock"),
        "unlock_board_items"
    ));
    assert!(row_message(&rows_locked, "Lock").is_none());
}

/// #2405: the new rows run their commands with an unlocked footprint
/// selected.
#[test]
fn the_board_menu_has_find_similar_net_and_mirror_rows_that_run_their_commands() {
    let rows = rows(PcbTargets {
        net: true,
        ..footprint_picked()
    });
    for (label, command) in [
        ("Find Similar Objects...", "find_similar_objects"),
        ("Find", "find"),
        ("Select Net", "select_board_net"),
        ("Highlight Net", "highlight_board_net"),
        ("Mirror X", "mirror_x"),
        ("Mirror Y", "mirror_y"),
        ("Flip Selection", "flip_selection"),
    ] {
        assert!(runs(row_message(&rows, label), command), "{label}");
    }
}

/// #2405: Undo and Redo follow the board's history, as Edit's rows do.
#[test]
fn the_board_menu_undo_and_redo_follow_the_history() {
    let mut ctx = board_ctx(footprint_picked());
    let quiet = pcb_canvas_menu_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        &ctx,
        None,
        &shortcuts(),
    );
    assert!(disabled_of(&quiet, "Undo"));
    assert!(disabled_of(&quiet, "Redo"));
    ctx.can_undo = true;
    let undoable = pcb_canvas_menu_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        &ctx,
        None,
        &shortcuts(),
    );
    assert!(runs(row_message(&undoable, "Undo"), "undo"));
    assert!(disabled_of(&undoable, "Redo"));
}

/// #2405: the Place launcher is always there; Align only with a selection.
#[test]
fn the_board_menu_offers_place_always_and_align_with_a_selection() {
    let launches = |rows: &[DropdownEntry<Message>], kind: ContextSubmenu| {
        rows.iter().any(|row| {
            matches!(row, DropdownEntry::Item(it)
                if matches!(&it.on_press,
                    Some(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(open))) if *open == kind))
        })
    };
    let empty = rows(PcbTargets {
        shown: true,
        ..PcbTargets::default()
    });
    assert!(launches(&empty, ContextSubmenu::Place));
    assert!(!launches(&empty, ContextSubmenu::Align));
    let picked = rows(footprint_picked());
    assert!(launches(&picked, ContextSubmenu::Place));
    assert!(launches(&picked, ContextSubmenu::Align));
}

/// #2405: on a read-only board every row that edits is greyed; the rows
/// that only read the board stay live.
#[test]
fn a_read_only_board_greys_every_editing_row() {
    // What `pcb_targets` reports for a read-only board with a footprint
    // selected and a board copy on the clipboard.
    let read_only = PcbTargets {
        shown: true,
        any: true,
        copyable: true,
        read_only: true,
        footprints: 1,
        net: true,
        ..PcbTargets::default()
    };
    let rows = rows(read_only);
    for label in [
        "Cut",
        "Paste",
        "Paste Special",
        "Move Selection",
        "Rotate",
        "Rotate Clockwise",
        "Flip Selection",
        "Mirror X",
        "Mirror Y",
        "Change Footprint...",
        "Delete",
        "Board Sketch",
    ] {
        assert!(
            disabled_of(&rows, label),
            "{label} is live on a read-only board"
        );
    }
    for label in [
        "Copy",
        "Select All",
        "Select Net",
        "Cross Probe",
        "Toggle Ratsnest",
    ] {
        assert!(!disabled_of(&rows, label), "{label} reads the board only");
    }
}

/// #2405: the Place flyout lists the board's tools; each live tool runs its
/// command, and on a read-only board every tool and Board Sketch is greyed.
#[test]
fn the_board_place_flyout_runs_the_tools_and_greys_them_when_read_only() {
    let i18n = signex_i18n::I18n::en_us_ref();
    let hint = |_: &str| String::new();
    let live = board_ctx(PcbTargets {
        shown: true,
        ..PcbTargets::default()
    });
    let place = board_place_entries(i18n, &live, &hint);
    assert!(runs(row_message(&place, "Via"), "place_board_via"));
    assert!(runs(row_message(&place, "Keepout"), "place_board_keepout"));
    assert!(runs(row_message(&place, "Board Sketch"), "board_sketch"));
    // A tool the board editor cannot run yet is greyed, with its reason.
    // Pad is live since #2373; Component is still greyed.
    assert!(runs(row_message(&place, "Pad"), "place_board_pad"));
    assert!(row_message(&place, "Component").is_none());
    let read_only = board_ctx(PcbTargets {
        shown: true,
        read_only: true,
        ..PcbTargets::default()
    });
    let place = board_place_entries(i18n, &read_only, &hint);
    for label in ["Via", "Track", "Keepout", "Board Sketch"] {
        assert!(disabled_of(&place, label), "{label}");
    }
}

/// #2408, #2409: the board menu offers the selection tools, each running
/// its command: live with a selection (Arrange in Box with unlocked
/// components); an empty-area menu has none of them (2026-11-18).
#[test]
fn the_board_menu_offers_the_selection_tools() {
    let picked = rows(PcbTargets {
        shown: true,
        any: true,
        placeable: true,
        routes: true,
        footprints: 1,
        ..PcbTargets::default()
    });
    for (label, command) in [
        ("Gloss Selected Routes", "gloss_selected_routes"),
        ("Add Teardrops", "add_teardrops"),
        ("Board Shape from Selection", "board_shape_from_selection"),
        ("Keepout from Selection", "keepout_from_selection"),
        ("Arrange in Box", "arrange_in_box"),
    ] {
        assert!(runs(row_message(&picked, label), command), "{label}");
    }
    let empty = rows(PcbTargets {
        shown: true,
        ..PcbTargets::default()
    });
    assert!(absent(&empty, "Add Teardrops"), "a route row needs a route");
    for label in [
        "Gloss Selected Routes",
        "Board Shape from Selection",
        "Keepout from Selection",
        "Arrange in Box",
    ] {
        assert!(
            absent(&empty, label),
            "{label} is not on the empty-area menu"
        );
    }
}

/// #2462: Compare with Schematic sits after Properties, above the View
/// group, and runs `compare_board_with_schematic` when the board is part
/// of an open project; it is greyed otherwise, like Update PCB. It is an
/// object-menu row: the empty-area menu stays short (2026-11-18).
#[test]
fn the_board_menu_compares_with_the_schematic_in_a_project() {
    let board = PcbTargets {
        shown: true,
        hovered: true,
        ..PcbTargets::default()
    };
    let loose = rows(board);
    assert!(row_message(&loose, "Compare with Schematic").is_none());
    let mut ctx = board_ctx(board);
    ctx.has_open_project = true;
    let in_project = pcb_canvas_menu_entries(
        signex_i18n::I18n::en_us_ref(),
        TID,
        &ctx,
        None,
        &shortcuts(),
    );
    assert!(runs(
        row_message(&in_project, "Compare with Schematic"),
        "compare_board_with_schematic"
    ));
    let at = |label: &str| {
        in_project
            .iter()
            .position(|row| matches!(row, DropdownEntry::Item(it) if it.label == label))
            .unwrap_or_else(|| panic!("missing row {label:?}"))
    };
    let compare = at("Compare with Schematic");
    assert_eq!(compare, at("Properties...") + 1);
    assert!(matches!(in_project[compare + 1], DropdownEntry::Separator));
    assert_eq!(compare + 2, at("Toggle Ratsnest"));
}

/// #2506: a right-click on a polygon pour offers Unfill Selected, which
/// runs `unfill_selected`, right before Polygon Manager; off a pour the row
/// is not there.
#[test]
fn the_pour_menu_has_a_live_unfill_row_before_polygon_manager() {
    let picked = PcbTargets {
        shown: true,
        any: true,
        ..PcbTargets::default()
    };
    let on_pour = crate::app::view::context_menu::board_menu::pcb_pour_menu(
        signex_i18n::I18n::en_us_ref(),
        TID,
        &board_ctx(picked),
        &shortcuts(),
    );
    assert!(runs(
        row_message(&on_pour, "Unfill Selected"),
        "unfill_selected"
    ));
    let names = labels(&on_pour);
    let unfill = names.iter().position(|l| l == "Unfill Selected");
    let manager = names.iter().position(|l| l == "Polygon Manager...");
    assert_eq!(unfill.map(|at| at + 1), manager, "{names:?}");
    assert!(absent(&rows(picked), "Unfill Selected"));
}
