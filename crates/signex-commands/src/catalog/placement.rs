//! Placement commands: components, symbols, power ports, junctions and
//! markers, blocks, directives and text on a schematic sheet.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const PLACEMENT: &[CommandMetadata] = &[
    CommandMetadata {
        id: "place_component",
        category: "place",
        label: command_ids::CMD_PLACE_COMPONENT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPONENT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PART,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_SYMBOL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_power_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_POWER_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_POWER_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::POWER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_junction",
        category: "place",
        label: command_ids::CMD_PLACE_JUNCTION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_JUNCTION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::JUNCTION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_connect",
        category: "place",
        label: command_ids::CMD_PLACE_NO_CONNECT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_erc",
        category: "place",
        label: command_ids::CMD_PLACE_NO_ERC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NO_ERC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::NO_ERC,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_compile_mask",
        category: "place",
        label: command_ids::CMD_PLACE_COMPILE_MASK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPILE_MASK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SEGMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_design_block",
        category: "place",
        label: command_ids::CMD_PLACE_DESIGN_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DESIGN_BLOCK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PLACE_FILE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_reuse_block",
        category: "place",
        label: command_ids::CMD_PLACE_REUSE_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_REUSE_BLOCK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_parameter_set",
        category: "place",
        label: command_ids::CMD_PLACE_PARAMETER_SET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_PARAMETER_SET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::NET_PARAMETERS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_diff_pair",
        category: "place",
        label: command_ids::CMD_PLACE_DIFF_PAIR,
        menu_label: Some(command_ids::CMD_MENU_PLACE_DIFF_PAIR),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_blanket",
        category: "place",
        label: command_ids::CMD_PLACE_BLANKET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BLANKET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BLANKET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_string",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_STRING,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_STRING),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_frame",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT_FRAME,
        menu_label: Some(command_ids::CMD_MENU_PLACE_TEXT_FRAME),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_note",
        category: "place",
        label: command_ids::CMD_PLACE_NOTE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NOTE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    // #1116: an image as a project asset. Without operands it asks for the
    // file; the image lands on the cursor and one click places it as one
    // undo step. No default key.
    CommandMetadata {
        id: "place_graphic",
        category: "place",
        label: command_ids::CMD_PLACE_GRAPHIC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GRAPHIC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::PLACE_IMAGE,
        ..CommandMetadata::DEFAULT
    },
    // #2505: Place ▸ Component on a board opens the part picker; the pick
    // goes on the cursor and each click places it as one undo step. No
    // default key.
    CommandMetadata {
        id: "place_board_component",
        category: "place",
        label: command_ids::CMD_PLACE_BOARD_COMPONENT,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_COMPONENT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_MUTATES,
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
    // #2410: two equivalent pins (the two selected pads) or gates of one
    // component trade nets on the board and on its schematic, one undo step
    // across both. No default key.
    CommandMetadata {
        id: "swap_pins",
        category: "modify",
        label: command_ids::CMD_SWAP_PINS,
        menu_label: Some(command_ids::CMD_MENU_SWAP_PINS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SWAP_PINS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "swap_gates",
        category: "modify",
        label: command_ids::CMD_SWAP_GATES,
        menu_label: Some(command_ids::CMD_MENU_SWAP_GATES),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::SWAP_GATES,
        ..CommandMetadata::DEFAULT
    },
    // #2410: design reuse. Saving writes the block file of the project and
    // leaves the board as it is; placing lays a block onto the selected
    // component's channel as one undo step.
    CommandMetadata {
        id: "save_board_reuse_block",
        category: "design",
        label: command_ids::CMD_SAVE_BOARD_REUSE_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_SAVE_BOARD_REUSE_BLOCK),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: super::operands::SAVE_REUSE_BLOCK,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_board_reuse_block",
        category: "design",
        label: command_ids::CMD_PLACE_BOARD_REUSE_BLOCK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BOARD_REUSE_BLOCK),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::PLACE_REUSE_BLOCK,
        ..CommandMetadata::DEFAULT
    },
];
