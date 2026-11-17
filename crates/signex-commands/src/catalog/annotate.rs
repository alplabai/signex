//! Annotation commands: number schematic designators, re-annotate a board
//! and back-annotate its changes to the schematics.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const ANNOTATE: &[CommandMetadata] = &[
    CommandMetadata {
        id: "annotate_schematic",
        category: "design",
        label: command_ids::CMD_ANNOTATE_SCHEMATIC,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "annotate_schematic_quietly",
        category: "design",
        label: command_ids::CMD_ANNOTATE_SCHEMATIC_QUIETLY,
        menu_label: Some(command_ids::CMD_MENU_ANNOTATE_SCHEMATIC_QUIETLY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "force_annotate_all_schematics",
        category: "design",
        label: command_ids::CMD_FORCE_ANNOTATE_ALL_SCHEMATICS,
        menu_label: Some(command_ids::CMD_MENU_FORCE_ANNOTATE_ALL_SCHEMATICS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_schematic_designators",
        category: "design",
        label: command_ids::CMD_RESET_SCHEMATIC_DESIGNATORS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
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
    // #1312: Back Annotate, from a schematic or (Update Schematics) a board
    // of a project. Execute is one undo group across the changed sheets.
    // No default key.
    CommandMetadata {
        id: "back_annotate_schematics",
        category: "pcb_sync",
        label: command_ids::CMD_BACK_ANNOTATE_SCHEMATICS,
        menu_label: Some(command_ids::CMD_MENU_BACK_ANNOTATE_SCHEMATICS),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
