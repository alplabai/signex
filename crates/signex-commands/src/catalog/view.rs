//! View commands: zoom, centre, refresh, auto focus, the board view
//! rotation and origin marker, measuring and the display unit.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const VIEW: &[CommandMetadata] = &[
    CommandMetadata {
        id: "zoom_in_at_cursor",
        category: "view",
        label: command_ids::CMD_ZOOM_IN_AT_CURSOR,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_IN_AT_CURSOR),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_out_at_cursor",
        category: "view",
        label: command_ids::CMD_ZOOM_OUT_AT_CURSOR,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_OUT_AT_CURSOR),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_all_objects",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_ALL_OBJECTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_fit",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_FIT,
        menu_label: Some(command_ids::CMD_MENU_ZOOM_TO_FIT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "zoom_to_selection_area",
        category: "view",
        label: command_ids::CMD_ZOOM_TO_SELECTION_AREA,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "center_on_cursor",
        category: "view",
        label: command_ids::CMD_CENTER_ON_CURSOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "center_view_at_cursor",
        category: "view",
        label: command_ids::CMD_CENTER_VIEW_AT_CURSOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "refresh_view",
        category: "view",
        label: command_ids::CMD_REFRESH_VIEW,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_all_design_objects",
        category: "view",
        label: command_ids::CMD_SHOW_ALL_DESIGN_OBJECTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_auto_focus",
        category: "view",
        label: command_ids::CMD_TOGGLE_AUTO_FOCUS,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_AUTO_FOCUS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2316: turn the board view on screen. View state only (no undo step,
    // no dirty mark). No default keys: no free conventional chord exists.
    // `view_rotate` takes `degrees` (90 when left out) from an agent or the
    // CLI, so it is not GUI-only; it acts on the board canvas in view.
    CommandMetadata {
        id: "view_rotate",
        category: "view",
        label: command_ids::CMD_VIEW_ROTATE,
        menu_label: Some(command_ids::CMD_MENU_VIEW_ROTATE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::TURN,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "view_rotate_ccw",
        category: "view",
        label: command_ids::CMD_VIEW_ROTATE_CCW,
        menu_label: Some(command_ids::CMD_MENU_VIEW_ROTATE_CCW),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "view_rotate_180",
        category: "view",
        label: command_ids::CMD_VIEW_ROTATE_180,
        menu_label: Some(command_ids::CMD_MENU_VIEW_ROTATE_180),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "view_rotate_reset",
        category: "view",
        label: command_ids::CMD_VIEW_ROTATE_RESET,
        menu_label: Some(command_ids::CMD_MENU_VIEW_ROTATE_RESET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2351: the board origin marker, shown by default. No default key.
    CommandMetadata {
        id: "toggle_origin_marker",
        category: "view",
        label: command_ids::CMD_TOGGLE_ORIGIN_MARKER,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_ORIGIN_MARKER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // 2026-11-18: Edit ▸ Origin, where the board counts from. Each is one
    // undo step on the board's options; greyed on a read-only board.
    CommandMetadata {
        id: "set_board_origin",
        category: "edit",
        label: command_ids::CMD_SET_BOARD_ORIGIN,
        menu_label: Some(command_ids::CMD_MENU_SET_BOARD_ORIGIN),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_board_origin",
        category: "edit",
        label: command_ids::CMD_RESET_BOARD_ORIGIN,
        menu_label: Some(command_ids::CMD_MENU_RESET_BOARD_ORIGIN),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "snap_board_origin_to_corner",
        category: "edit",
        label: command_ids::CMD_SNAP_BOARD_ORIGIN_TO_CORNER,
        menu_label: Some(command_ids::CMD_MENU_SNAP_BOARD_ORIGIN_TO_CORNER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "measure_distance",
        category: "view",
        label: command_ids::CMD_MEASURE_DISTANCE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2403: the board queries an agent or the CLI asks; each answers with
    // data about the named board and changes nothing. The person's surfaces
    // are Measure Distance, the Net Inspector and Find Similar Objects.
    CommandMetadata {
        id: "measure_board",
        category: "search",
        label: command_ids::CMD_MEASURE_BOARD,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::MEASURE_BOARD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_local_coordinates",
        category: "view",
        label: command_ids::CMD_RESET_LOCAL_COORDINATES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_unit",
        category: "view",
        label: command_ids::CMD_CYCLE_UNIT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2390: the board in 3D, or flat again. View state only (no undo
    // step, no dirty mark); `3` on a board in both built-in profiles.
    CommandMetadata {
        id: "view_board_3d",
        category: "view",
        label: command_ids::CMD_VIEW_BOARD_3D,
        menu_label: Some(command_ids::CMD_MENU_VIEW_BOARD_3D),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_board_view",
        category: "view",
        label: command_ids::CMD_FLIP_BOARD_VIEW,
        menu_label: Some(command_ids::CMD_MENU_FLIP_BOARD_VIEW),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
