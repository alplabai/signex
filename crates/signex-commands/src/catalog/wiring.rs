//! Wiring commands: wires, buses and their entries, net labels, ports,
//! off-sheet connectors and signal harnesses.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const WIRING: &[CommandMetadata] = &[
    CommandMetadata {
        id: "draw_wire",
        category: "place",
        label: command_ids::CMD_DRAW_WIRE,
        menu_label: Some(command_ids::CMD_MENU_DRAW_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bus",
        category: "place",
        label: command_ids::CMD_DRAW_BUS,
        menu_label: Some(command_ids::CMD_MENU_DRAW_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus",
        category: "place",
        label: command_ids::CMD_PLACE_BUS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_BUS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire_to_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE_TO_BUS_ENTRY,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "break_wire",
        category: "modify",
        label: command_ids::CMD_BREAK_WIRE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_NET_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NET_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_local_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_LOCAL_NET_LABEL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_global_label",
        category: "place",
        label: command_ids::CMD_PLACE_GLOBAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GLOBAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_hierarchical_label",
        category: "place",
        label: command_ids::CMD_PLACE_HIERARCHICAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HIERARCHICAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_port",
        category: "place",
        label: command_ids::CMD_PLACE_PORT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PORT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_off_sheet_connector",
        category: "place",
        label: command_ids::CMD_PLACE_OFF_SHEET_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_OFF_SHEET_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_signal_harness",
        category: "place",
        label: command_ids::CMD_PLACE_SIGNAL_HARNESS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SIGNAL_HARNESS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_connector",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_CONNECTOR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_CONNECTOR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_entry",
        category: "place",
        label: command_ids::CMD_PLACE_HARNESS_ENTRY,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HARNESS_ENTRY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
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
    // #2719: the Interactive Routing options on the named board, answered
    // as JSON; each operand given sets its option first. Changes no board.
    // No default key.
    CommandMetadata {
        id: "route_options",
        category: "pcb",
        label: command_ids::CMD_ROUTE_OPTIONS,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: operands::ROUTE_OPTIONS,
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
    // #2389: the routing tools of the Route group. Each route is one undo
    // step when it ends. No default key.
    CommandMetadata {
        id: "place_board_diff_pair",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_DIFF_PAIR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_DIFF_PAIR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_quick_route",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_QUICK_ROUTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_QUICK_ROUTE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_multi_route",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_MULTI_ROUTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_MULTI_ROUTE),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #2389: the Tune group. Without operands they tune the selected
    // tracks' nets on the board in view; an agent names the board and the
    // net. One undo step each.
    CommandMetadata {
        id: "tune_net_length",
        category: "pcb",
        label: command_ids::CMD_TUNE_NET_LENGTH,
        menu_label: Some(command_ids::CMD_MENU_TUNE_NET_LENGTH),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::TUNE_LENGTH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "tune_diff_pair",
        category: "pcb",
        label: command_ids::CMD_TUNE_DIFF_PAIR,
        menu_label: Some(command_ids::CMD_MENU_TUNE_DIFF_PAIR),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::TUNE_PAIR,
        ..CommandMetadata::DEFAULT
    },
    // #2585: the width of a differential pair's tracks, both nets in one
    // undo step; the Track panel's Width row runs it on a pair. No default
    // key.
    CommandMetadata {
        id: "set_diff_pair_width",
        category: "modify",
        label: command_ids::CMD_SET_DIFF_PAIR_WIDTH,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SET_PAIR_WIDTH,
        ..CommandMetadata::DEFAULT
    },
    // #2797, mockup row 107: the Length Matching tab of the board in view;
    // with `document` the report of its groups. No default key.
    CommandMetadata {
        id: "length_matching",
        category: "pcb",
        label: command_ids::CMD_LENGTH_MATCHING,
        menu_label: Some(command_ids::CMD_MENU_LENGTH_MATCHING),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2797: make or change, and remove, a length-matching group; each one
    // board undo step. The Length Matching tab's fields run them.
    CommandMetadata {
        id: "set_length_group",
        category: "pcb",
        label: command_ids::CMD_SET_LENGTH_GROUP,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::LENGTH_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_length_group",
        category: "pcb",
        label: command_ids::CMD_REMOVE_LENGTH_GROUP,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::REMOVE_LENGTH_GROUP,
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
    // The board's teardrop rule and a pad's own teardrop setting (PCB
    // Routing): board data, one undo step each, that Add Teardrops reads.
    // No menu row or mockup yet; operands only. No default key.
    CommandMetadata {
        id: "set_teardrop_rule",
        category: "pcb",
        label: command_ids::CMD_SET_TEARDROP_RULE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::board_rule_operands::SET_TEARDROP_RULE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_pad_teardrop",
        category: "pcb",
        label: command_ids::CMD_SET_PAD_TEARDROP,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::board_rule_operands::SET_PAD_TEARDROP,
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
];
