//! PCB-surface command metadata: placement, the design rule check, the
//! board view (ratsnest, layers) and board sketch mode.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n::command_ids;

pub(super) const PCB: &[CommandMetadata] = &[
    // Read-only: the report goes to the Messages panel, the board is not touched.
    CommandMetadata {
        id: "run_drc",
        category: "pcb",
        label: command_ids::CMD_RUN_DRC,
        menu_label: Some(command_ids::CMD_MENU_RUN_DRC),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    // #1312: Positional Re-Annotate. Opens the dialog; its Execute is one
    // board undo step. No default key.
    CommandMetadata {
        id: "reannotate_board",
        category: "design",
        label: command_ids::CMD_REANNOTATE_BOARD,
        menu_label: Some(command_ids::CMD_MENU_REANNOTATE_BOARD),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // Flip the selected footprints to the other side of the board (#2240).
    // No default key: a profile may bind one.
    CommandMetadata {
        id: "flip_selection",
        category: "modify",
        label: command_ids::CMD_FLIP_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_FLIP_SELECTION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2366: pick a library footprint for the selected footprints; the swap
    // is one board undo step. No default key.
    CommandMetadata {
        id: "change_footprint",
        category: "modify",
        label: command_ids::CMD_CHANGE_FOOTPRINT,
        menu_label: Some(command_ids::CMD_MENU_CHANGE_FOOTPRINT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_ratsnest",
        category: "view",
        label: command_ids::CMD_TOGGLE_RATSNEST,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_RATSNEST),
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
    CommandMetadata {
        id: "next_layer",
        category: "view",
        label: command_ids::CMD_NEXT_LAYER,
        menu_label: Some(command_ids::CMD_MENU_NEXT_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "prev_layer",
        category: "view",
        label: command_ids::CMD_PREV_LAYER,
        menu_label: Some(command_ids::CMD_MENU_PREV_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2357: `*` steps through the signal layers only, `+` through every
    // layer the board lists.
    CommandMetadata {
        id: "next_signal_layer",
        category: "view",
        label: command_ids::CMD_NEXT_SIGNAL_LAYER,
        menu_label: Some(command_ids::CMD_MENU_NEXT_SIGNAL_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_single_layer_mode",
        category: "view",
        label: command_ids::CMD_TOGGLE_SINGLE_LAYER_MODE,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_SINGLE_LAYER_MODE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layer_visibility",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYER_VISIBILITY,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYER_VISIBILITY),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_all_layers",
        category: "view",
        label: command_ids::CMD_SHOW_ALL_LAYERS,
        menu_label: Some(command_ids::CMD_MENU_SHOW_ALL_LAYERS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
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
        id: "toggle_layers_panel",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYERS_PANEL,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYERS_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #1312: the project's one board, opened unsaved (File > New > PCB,
    // Projects > Add New > PCB). #2232: the GUI opens the New PCB setup
    // dialog first; a headless run names its `profile`.
    CommandMetadata {
        id: "new_pcb_document",
        category: "file",
        label: command_ids::CMD_NEW_PCB_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_NEW_PCB_DOCUMENT),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: super::operands::NEW_PCB,
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
    // #2404: the board reports. In a window each opens Board Reports on its
    // table; named a board (`document`), each answers with the table.
    // Read-only, no default key.
    CommandMetadata {
        id: "board_report_bom",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_BOM,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_BOM),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_netlist",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_NETLIST,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_NETLIST),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_net_length",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_NET_LENGTH,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_NET_LENGTH),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "board_report_components",
        category: "pcb",
        label: command_ids::CMD_BOARD_REPORT_COMPONENTS,
        menu_label: Some(command_ids::CMD_MENU_BOARD_REPORT_COMPONENTS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2404: the report in view (the BOM with none) as a CSV file; headless
    // it writes `kind` of `document` to `path`.
    CommandMetadata {
        id: "export_board_report",
        category: "reports",
        label: command_ids::CMD_EXPORT_BOARD_REPORT,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_BOARD_REPORT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_REPORT_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // #2402: the Board Print / PDF dialog (File > Print and Export > PDF on
    // a board open it too); headless it writes `document` to `path`.
    CommandMetadata {
        id: "export_board_pdf",
        category: "reports",
        label: command_ids::CMD_EXPORT_BOARD_PDF,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_BOARD_PDF),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_PDF,
        ..CommandMetadata::DEFAULT
    },
];
