//! Board editing commands: the board placement tools (via, track, fill,
//! region, keepout) and board sketch mode.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const BOARD: &[CommandMetadata] = &[
    // #2373: the board placement tools. Each arms its tool on the board in
    // view; every click (or finished shape) is one undo step.
    CommandMetadata {
        id: "place_board_via",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_VIA,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_VIA),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_track",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_TRACK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_TRACK),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_fill",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_FILL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_FILL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_region",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_REGION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_REGION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_keepout",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_KEEPOUT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_KEEPOUT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2234: enter the board sketch of the board in view, or leave it
    // (committing the changes as one undo step). No default key.
    CommandMetadata {
        id: "board_sketch",
        category: "design",
        label: command_ids::CMD_BOARD_SKETCH,
        menu_label: Some(command_ids::CMD_MENU_BOARD_SKETCH),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2216: a pour whose copper is computed clear of other nets.
    CommandMetadata {
        id: "place_board_polygon_pour",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_POLYGON_POUR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_POLYGON_POUR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2216: cut an area out of the pours on the active copper layer, or
    // slice them along a line; each edit is one undo step. No default key.
    CommandMetadata {
        id: "place_board_pour_cutout",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_POUR_CUTOUT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_POUR_CUTOUT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_slice_pour",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_SLICE_POUR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_SLICE_POUR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2216: fill the pours again and report their copper. Repour Selected
    // reads the selection, so it is window only; Repour All named a board
    // (`document`) answers with each pour's fill as JSON. #2506: Repour All
    // also clears `unfilled` on every unfilled pour as one undo step, so it
    // edits the board (`MUTATES`: the CLI's Edit capability, journaled). No
    // default key.
    CommandMetadata {
        id: "repour_selected",
        category: "pcb",
        label: command_ids::CMD_REPOUR_SELECTED,
        menu_label: Some(command_ids::CMD_MENU_REPOUR_SELECTED),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "repour_all",
        category: "pcb",
        label: command_ids::CMD_REPOUR_ALL,
        menu_label: Some(command_ids::CMD_MENU_REPOUR_ALL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2506: clear the copper of the selected pours, keeping their outline,
    // until the next Repour; one undo step. It reads the selection, so it is
    // window only. No default key.
    CommandMetadata {
        id: "unfill_selected",
        category: "pcb",
        label: command_ids::CMD_UNFILL_SELECTED,
        menu_label: Some(command_ids::CMD_MENU_UNFILL_SELECTED),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // Mockup row 178: open the board's Polygon Manager tab, or focus it
    // (Tools, a pour's right-click, the PCB panel's Polygons mode). No
    // default key.
    CommandMetadata {
        id: "polygon_manager",
        category: "pcb",
        label: command_ids::CMD_POLYGON_MANAGER,
        menu_label: Some(command_ids::CMD_MENU_POLYGON_MANAGER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_shape_from_selection",
        category: "design",
        label: command_ids::CMD_BOARD_SHAPE_FROM_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_BOARD_SHAPE_FROM_SELECTION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "keepout_from_selection",
        category: "design",
        label: command_ids::CMD_KEEPOUT_FROM_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_KEEPOUT_FROM_SELECTION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "panelize_board",
        category: "pcb",
        label: command_ids::CMD_PANELIZE_BOARD,
        menu_label: Some(command_ids::CMD_MENU_PANELIZE_BOARD),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_board_panel",
        category: "pcb",
        label: command_ids::CMD_REMOVE_BOARD_PANEL,
        menu_label: Some(command_ids::CMD_MENU_REMOVE_BOARD_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_board_panel",
        category: "pcb",
        label: command_ids::CMD_SET_BOARD_PANEL,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::SET_BOARD_PANEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_panel",
        category: "pcb",
        label: command_ids::CMD_BOARD_PANEL,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2457: Place ▸ Room draws a room by two corners; one undo step.
    CommandMetadata {
        id: "place_board_room",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_ROOM,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_ROOM),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2457: Design ▸ Rooms ▸ Create Rooms from Sheets opens its dialog;
    // Create runs as one undo step. Greyed without a project.
    CommandMetadata {
        id: "create_rooms_from_sheets",
        category: "design",
        label: command_ids::CMD_CREATE_ROOMS_FROM_SHEETS,
        menu_label: Some(command_ids::CMD_MENU_CREATE_ROOMS_FROM_SHEETS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
