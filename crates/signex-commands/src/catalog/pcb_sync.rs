//! Schematic-to-board sync commands: update the board from the schematic,
//! compare the two (in the dialog, or as a query and its apply form), the
//! board's status counters, assign footprints and change a placed
//! footprint.

use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, footprint_operands,
    operands,
};
use signex_i18n_ids::command_ids;

pub(super) const PCB_SYNC: &[CommandMetadata] = &[
    // #1312: the Update PCB dialog, from a schematic or (Import Changes) a
    // board of a project. Execute is one undo step on the board. From
    // operands (#2470): `document` names the project (left out: the active
    // project), `apply` or `dry_run` the run, and the answer is the change list.
    CommandMetadata {
        id: "update_pcb_from_schematic",
        category: "pcb_sync",
        label: command_ids::CMD_UPDATE_PCB_FROM_SCHEMATIC,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::UPDATE_PCB,
        ..CommandMetadata::DEFAULT
    },
    // #2462: the report of every difference between the schematic and the
    // board, with a direction per row. Apply is one board undo step and one
    // schematic undo group. Gated like Update PCB. No default key.
    CommandMetadata {
        id: "compare_board_with_schematic",
        category: "pcb_sync",
        label: command_ids::CMD_COMPARE_BOARD_WITH_SCHEMATIC,
        menu_label: Some(command_ids::CMD_MENU_COMPARE_BOARD_WITH_SCHEMATIC),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2462: the same report as a query, answered as JSON once the dialog's
    // reads land; it changes nothing.
    CommandMetadata {
        id: "check_board_against_schematic",
        category: "pcb_sync",
        label: command_ids::CMD_CHECK_BOARD_AGAINST_SCHEMATIC,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_BOARD_AGAINST_SCHEMATIC,
        ..CommandMetadata::DEFAULT
    },
    // #2462: the dialog's Apply for the rows named by id, each to the board
    // or to the schematic: one board undo step and one schematic undo group.
    CommandMetadata {
        id: "apply_board_schematic_differences",
        category: "pcb_sync",
        label: command_ids::CMD_APPLY_BOARD_SCHEMATIC_DIFFERENCES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::APPLY_BOARD_SCHEMATIC_DIFFERENCES,
        ..CommandMetadata::DEFAULT
    },
    // #2460: the board status bar's counters, with the parts not placed,
    // the waived DRC findings, the board against the schematic and the
    // routed length, as JSON.
    CommandMetadata {
        id: "board_status",
        category: "validation",
        label: command_ids::CMD_BOARD_STATUS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::BOARD_STATUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "assign_footprints",
        category: "design",
        label: command_ids::CMD_ASSIGN_FOOTPRINTS,
        menu_label: Some(command_ids::CMD_MENU_ASSIGN_FOOTPRINTS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2937: the footprint of placed parts on the sheet `document` names, as
    // Assign Footprints' Apply sets it, from its operands only.
    CommandMetadata {
        id: "assign_footprint",
        category: "design",
        label: command_ids::CMD_ASSIGN_FOOTPRINT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: footprint_operands::FOOTPRINT_CHOICE,
        ..CommandMetadata::DEFAULT
    },
    // #2937: every part's footprint, and whether an open library has it.
    CommandMetadata {
        id: "query_footprint_assignments",
        category: "design",
        label: command_ids::CMD_QUERY_FOOTPRINT_ASSIGNMENTS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2366: pick a library footprint for the selected footprints; the swap
    // is one board undo step. No default key. #2937: with its operands it
    // swaps the footprints named on the board `document` names, headless.
    CommandMetadata {
        id: "change_footprint",
        category: "modify",
        label: command_ids::CMD_CHANGE_FOOTPRINT,
        menu_label: Some(command_ids::CMD_MENU_CHANGE_FOOTPRINT),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: footprint_operands::FOOTPRINT_SWAP,
        ..CommandMetadata::DEFAULT
    },
];
