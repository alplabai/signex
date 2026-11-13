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
];
