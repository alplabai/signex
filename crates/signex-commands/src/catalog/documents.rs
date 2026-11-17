//! Document commands: create, open, save, revert, close and print a
//! document or project, plus the project options and report dialogs.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
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
    CommandMetadata {
        id: "project_settings",
        category: "design",
        label: command_ids::CMD_PROJECT_SETTINGS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_move_sheet",
        category: "design",
        label: command_ids::CMD_PROJECT_MOVE_SHEET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_MOVE_SHEET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_add_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_ADD_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_rename_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_RENAME_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_RENAME_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_remove_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_REMOVE_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_active_variant",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_ACTIVE_VARIANT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ACTIVE_VARIANT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_move_library",
        category: "design",
        label: command_ids::CMD_PROJECT_MOVE_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_MOVE_LIBRARY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_annotate_undo",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_ANNOTATE_UNDO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_ANNOTATE_UNDO,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_version_control",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_VERSION_CONTROL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::PROJECT_VERSION_CONTROL,
        ..CommandMetadata::DEFAULT
    },
];
