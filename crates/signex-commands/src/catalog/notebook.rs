//! Design Notebook commands (#2795, mockup rows 195-202): open the
//! project's notebook, generate one from the project, export it as PDF,
//! show or hide the Notebook badges on the schematic, and the notebook
//! tab's own commands (compile and report, insert what the toolbar inserts,
//! pick the view and side panel). The tab's commands name the project with
//! `document`, so a CLI or an agent reaches the tab the person sees.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NOTEBOOK: &[CommandMetadata] = &[
    // Project ▸ Design Notebook, Add New ▸ Design Notebook and a palette
    // search for "notebook" or "wiki" (mockup rows 195 and 60b).
    CommandMetadata {
        id: "open_design_notebook",
        category: "design",
        label: command_ids::CMD_OPEN_DESIGN_NOTEBOOK,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DESIGN_NOTEBOOK),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // Project ▸ Design Notebook ▸ Generate from Project… (mockup row 201).
    CommandMetadata {
        id: "generate_design_notebook",
        category: "design",
        label: command_ids::CMD_GENERATE_DESIGN_NOTEBOOK,
        menu_label: Some(command_ids::CMD_MENU_GENERATE_DESIGN_NOTEBOOK),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // File ▸ Export ▸ Design Notebook PDF… (mockup row 202).
    CommandMetadata {
        id: "export_notebook_pdf",
        category: "file",
        label: command_ids::CMD_EXPORT_NOTEBOOK_PDF,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_NOTEBOOK_PDF),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // View ▸ Notebook Badges (mockup row 197).
    CommandMetadata {
        id: "toggle_notebook_badges",
        category: "view",
        label: command_ids::CMD_TOGGLE_NOTEBOOK_BADGES,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_NOTEBOOK_BADGES),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // The notebook tab (mockup row 195).
    CommandMetadata {
        id: "notebook_compile",
        category: "reports",
        label: command_ids::CMD_NOTEBOOK_COMPILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_insert",
        category: "edit",
        label: command_ids::CMD_NOTEBOOK_INSERT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::NOTEBOOK_INSERT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_view",
        category: "view",
        label: command_ids::CMD_NOTEBOOK_VIEW,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_VIEW,
        ..CommandMetadata::DEFAULT
    },
];
