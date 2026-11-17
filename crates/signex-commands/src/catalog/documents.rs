//! Document commands: create, open, save, revert, close and print a
//! document or project, plus the project options and report dialogs.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const DOCUMENTS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "new_document",
        category: "file",
        label: command_ids::CMD_NEW_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_NEW_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "new_project_with_profile",
        category: "file",
        label: command_ids::CMD_NEW_PROJECT_WITH_PROFILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NEW_PROJECT,
        ..CommandMetadata::DEFAULT
    },
    // #2232: a new sheet in the project (File > New > Schematic, Projects >
    // Add New > Schematic). The GUI opens the New Schematic setup dialog,
    // whose Create asks for the file; a headless run names its `profile`.
    CommandMetadata {
        id: "new_schematic_sheet",
        category: "file",
        label: command_ids::CMD_NEW_SCHEMATIC_SHEET,
        menu_label: Some(command_ids::CMD_MENU_NEW_SCHEMATIC_SHEET),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::NEW_SCHEMATIC,
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
    CommandMetadata {
        id: "open_document",
        category: "file",
        label: command_ids::CMD_OPEN_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::OPEN_PATH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "save_document_as",
        category: "file",
        label: command_ids::CMD_SAVE_DOCUMENT_AS,
        menu_label: Some(command_ids::CMD_MENU_SAVE_DOCUMENT_AS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::SAVE_DOCUMENT_AS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "revert",
        category: "edit",
        label: command_ids::CMD_REVERT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: operands::REVERT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_active_document",
        category: "file",
        label: command_ids::CMD_CLOSE_ACTIVE_DOCUMENT,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ACTIVE_DOCUMENT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "close_all_documents",
        category: "window",
        label: command_ids::CMD_CLOSE_ALL_DOCUMENTS,
        menu_label: Some(command_ids::CMD_MENU_CLOSE_ALL_DOCUMENTS),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "print",
        category: "file",
        label: command_ids::CMD_PRINT,
        menu_label: Some(command_ids::CMD_MENU_PRINT),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_project_options",
        category: "file",
        label: command_ids::CMD_OPEN_PROJECT_OPTIONS,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PROJECT_OPTIONS),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "report_manager_bom",
        category: "reports",
        label: command_ids::CMD_REPORT_MANAGER_BOM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
