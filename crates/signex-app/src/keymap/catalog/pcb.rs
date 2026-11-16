//! PCB-surface command metadata: placement, the design rule check, the
//! board view (ratsnest, layers) and board sketch mode.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
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
    // #2399: the DRC results panel and what it does. A violation is named by
    // its waiver key (`id`), the same in every run. No default keys.
    CommandMetadata {
        id: "open_drc_panel",
        category: "pcb",
        label: command_ids::CMD_OPEN_DRC_PANEL,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DRC_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_drc_violation",
        category: "pcb",
        label: command_ids::CMD_SHOW_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: operands::DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "waive_drc_violation",
        category: "pcb",
        label: command_ids::CMD_WAIVE_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::WAIVE_DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "unwaive_drc_violation",
        category: "pcb",
        label: command_ids::CMD_UNWAIVE_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "filter_drc_results",
        category: "pcb",
        label: command_ids::CMD_FILTER_DRC_RESULTS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: operands::DRC_FILTER,
        ..CommandMetadata::DEFAULT
    },
    // #2400: the board Net Classes panel and the edits of its table, each
    // one step of the project's net-class history. No default keys.
    CommandMetadata {
        id: "open_net_classes_panel",
        category: "design",
        label: command_ids::CMD_OPEN_NET_CLASSES_PANEL,
        menu_label: Some(command_ids::CMD_MENU_OPEN_NET_CLASSES_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "add_net_class",
        category: "design",
        label: command_ids::CMD_ADD_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::ADD_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rename_net_class",
        category: "design",
        label: command_ids::CMD_RENAME_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::RENAME_NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_net_class",
        category: "design",
        label: command_ids::CMD_DELETE_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::NET_CLASS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_net_class_rules",
        category: "design",
        label: command_ids::CMD_SET_NET_CLASS_RULES,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::NET_CLASS_RULES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "assign_board_net_class",
        category: "design",
        label: command_ids::CMD_ASSIGN_BOARD_NET_CLASS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::ASSIGN_BOARD_NET_CLASS,
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
    // #2388: open the Fabrication Outputs dialog on the board in view.
    // Nothing is written until the dialog's Generate. No default key.
    CommandMetadata {
        id: "fabrication_outputs",
        category: "file",
        label: command_ids::CMD_FABRICATION_OUTPUTS,
        menu_label: Some(command_ids::CMD_MENU_FABRICATION_OUTPUTS),
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
    // #2216: fill the pours again and report their copper. Read-only (fills
    // are computed, not stored). Repour Selected reads the selection, so it
    // is window only; Repour All named a board (`document`) answers with
    // each pour's fill as JSON. No default key.
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
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2389: Interactive Routing. Each route is one undo step when it ends.
    // No default key.
    CommandMetadata {
        id: "place_board_route",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_ROUTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_ROUTE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2389: walk around or push and shove. Changes no board. No default key.
    CommandMetadata {
        id: "cycle_route_mode",
        category: "pcb",
        label: command_ids::CMD_CYCLE_ROUTE_MODE,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2373: the free pad, arc, string and dimension tools. Each click (or
    // finished arc or dimension) is one undo step. No default key.
    CommandMetadata {
        id: "place_board_pad",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_PAD,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_PAD),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_arc",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_ARC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_ARC),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_string",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_STRING,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_STRING),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_dimension",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIMENSION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIMENSION),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2373: how the Arc tool's clicks make an arc. Changes no board. No
    // default key.
    CommandMetadata {
        id: "cycle_arc_mode",
        category: "pcb",
        label: command_ids::CMD_CYCLE_ARC_MODE,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2424: the Graphics tools, on the active non-copper layer. Each arms
    // its tool on the board in view; each drawing is one undo step. No
    // default keys.
    CommandMetadata {
        id: "place_board_line",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_LINE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_LINE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_graphic_arc",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_GRAPHIC_ARC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_GRAPHIC_ARC),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_circle",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_CIRCLE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_CIRCLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_rectangle",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_RECTANGLE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_RECTANGLE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_text_frame",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_TEXT_FRAME,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_TEXT_FRAME),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2424: the Graphics tools and the drawing's Properties fields as an
    // agent or the CLI runs them, on the board `document` names; each is
    // one undo step and answers with the drawing as JSON.
    CommandMetadata {
        id: "add_board_graphic",
        category: "place",
        label: command_ids::CMD_ADD_BOARD_GRAPHIC,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_GRAPHIC_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_board_graphic",
        category: "edit",
        label: command_ids::CMD_SET_BOARD_GRAPHIC,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_GRAPHIC_SET,
        ..CommandMetadata::DEFAULT
    },
    // #2394: the layer strip under the board. View state and per-user
    // settings only: no undo step, no dirty mark. The rows with operands run
    // from an agent or the CLI on the board `document` names (the per-user
    // ones name none); a person uses the tab menu or the layer set button,
    // so without operands they say so and do nothing.
    CommandMetadata {
        id: "show_layer_set",
        category: "view",
        label: command_ids::CMD_SHOW_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_SHOW_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_layer_set",
        category: "view",
        label: command_ids::CMD_SAVE_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_SAVE_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_layer_set",
        category: "view",
        label: command_ids::CMD_DELETE_LAYER_SET,
        menu_label: Some(command_ids::CMD_MENU_DELETE_LAYER_SET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_SET_NAME,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "hide_layer",
        category: "view",
        label: command_ids::CMD_HIDE_LAYER,
        menu_label: Some(command_ids::CMD_MENU_HIDE_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_layer",
        category: "view",
        label: command_ids::CMD_HIGHLIGHT_LAYER,
        menu_label: Some(command_ids::CMD_MENU_HIGHLIGHT_LAYER),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "hide_layer_group",
        category: "view",
        label: command_ids::CMD_HIDE_LAYER_GROUP,
        menu_label: Some(command_ids::CMD_MENU_HIDE_LAYER_GROUP),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_layer_group",
        category: "view",
        label: command_ids::CMD_SHOW_LAYER_GROUP,
        menu_label: Some(command_ids::CMD_MENU_SHOW_LAYER_GROUP),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_layer_tab_names",
        category: "view",
        label: command_ids::CMD_SET_LAYER_TAB_NAMES,
        menu_label: Some(command_ids::CMD_MENU_SET_LAYER_TAB_NAMES),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::LAYER_TAB_NAMES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layer_numbers",
        category: "view",
        label: command_ids::CMD_TOGGLE_LAYER_NUMBERS,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_LAYER_NUMBERS),
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
    // #2401: open the board's layer stack manager (the Layers tab menu's
    // "Layer stack..." row runs it too). No default key.
    CommandMetadata {
        id: "board_layer_stack",
        category: "design",
        label: command_ids::CMD_BOARD_LAYER_STACK,
        menu_label: Some(command_ids::CMD_MENU_BOARD_LAYER_STACK),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2401: the manager's edit for an agent or the CLI: the whole stack
    // as JSON rows, one undo step, answered with the stack it stored.
    CommandMetadata {
        id: "set_layer_stack",
        category: "design",
        label: command_ids::CMD_SET_LAYER_STACK,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_LAYER_STACK,
        ..CommandMetadata::DEFAULT
    },
    // #2407: lock or unlock the selected tracks and vias (the board menu's
    // rows), or the ones `ids` names on `document`. One undo step.
    CommandMetadata {
        id: "lock_board_items",
        category: "modify",
        label: command_ids::CMD_LOCK_BOARD_ITEMS,
        menu_label: Some(command_ids::CMD_MENU_LOCK_BOARD_ITEMS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "unlock_board_items",
        category: "modify",
        label: command_ids::CMD_UNLOCK_BOARD_ITEMS,
        menu_label: Some(command_ids::CMD_MENU_UNLOCK_BOARD_ITEMS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    // #2407: the span of vias; the Via panel's From and To rows are its
    // surface. One undo step.
    CommandMetadata {
        id: "set_via_span",
        category: "modify",
        label: command_ids::CMD_SET_VIA_SPAN,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_VIA_SPAN,
        ..CommandMetadata::DEFAULT
    },
    // #2401: per-layer copper sizes of vias or pads; the Via and Pad
    // Stack rows are its surface. One undo step.
    CommandMetadata {
        id: "set_layer_sizes",
        category: "modify",
        label: command_ids::CMD_SET_LAYER_SIZES,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_LAYER_SIZES,
        ..CommandMetadata::DEFAULT
    },
    // #2403: the board net `net` names, else that of the selection or of
    // what is under the cursor: select its tracks, vias and pads, or draw
    // it bright (again: no longer). `net` comes from a typed command line
    // (the CLI), so neither is GUI-only. View and selection only; no key.
    CommandMetadata {
        id: "select_board_net",
        category: "select",
        label: command_ids::CMD_SELECT_BOARD_NET,
        menu_label: Some(command_ids::CMD_MENU_SELECT_BOARD_NET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_board_net",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_BOARD_NET,
        menu_label: Some(command_ids::CMD_MENU_HIGHLIGHT_BOARD_NET),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::BOARD_NET,
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
        id: "query_board_nets",
        category: "search",
        label: command_ids::CMD_QUERY_BOARD_NETS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::QUERY_BOARD_NETS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "find_similar_items",
        category: "search",
        label: command_ids::CMD_FIND_SIMILAR_ITEMS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::FIND_SIMILAR_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    // #2408, #2409: the board Tools edits. Each acts on the board in view and
    // its selection from a menu, the board menu, the palette or a key, and on
    // the board an agent names with operands; each is one undo step. No
    // default keys.
    CommandMetadata {
        id: "stitch_vias",
        category: "pcb",
        label: command_ids::CMD_STITCH_VIAS,
        menu_label: Some(command_ids::CMD_MENU_STITCH_VIAS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::STITCH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "add_teardrops",
        category: "pcb",
        label: command_ids::CMD_ADD_TEARDROPS,
        menu_label: Some(command_ids::CMD_MENU_ADD_TEARDROPS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::TEARDROPS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_teardrops",
        category: "pcb",
        label: command_ids::CMD_REMOVE_TEARDROPS,
        menu_label: Some(command_ids::CMD_MENU_REMOVE_TEARDROPS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::TEARDROPS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gloss_selected_routes",
        category: "pcb",
        label: command_ids::CMD_GLOSS_SELECTED_ROUTES,
        menu_label: Some(command_ids::CMD_MENU_GLOSS_SELECTED_ROUTES),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
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
        id: "arrange_in_box",
        category: "modify",
        label: command_ids::CMD_ARRANGE_IN_BOX,
        menu_label: Some(command_ids::CMD_MENU_ARRANGE_IN_BOX),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::ARRANGE_BOX,
        ..CommandMetadata::DEFAULT
    },
    // #2410: DXF import. Without operands it asks for the file; the import
    // is one board undo step. No default key.
    CommandMetadata {
        id: "import_dxf",
        category: "file",
        label: command_ids::CMD_IMPORT_DXF,
        menu_label: Some(command_ids::CMD_MENU_IMPORT_DXF),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::IMPORT_DXF,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "import_dxf_outline",
        category: "file",
        label: command_ids::CMD_IMPORT_DXF_OUTLINE,
        menu_label: Some(command_ids::CMD_MENU_IMPORT_DXF_OUTLINE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::IMPORT_DXF_OUTLINE,
        ..CommandMetadata::DEFAULT
    },
];
