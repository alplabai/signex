//! The Design Notebook's bench commands (mockup row 199): the readings, a
//! target's timeline and the attachments of a project's notebook, the
//! timeline written as CSV, Mark still valid, and a file copied into
//! `notebook/attachments/`. Each names the project with `document`; none
//! has a key or a menu row (the notebook's margin lane is their surface).

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NOTEBOOK_MEASUREMENTS: &[CommandMetadata] = &[
    CommandMetadata {
        id: "notebook_measurements",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_MEASUREMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_timeline",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_TIMELINE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_TIMELINE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_attachments",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_ATTACHMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // Writes the CSV `path` names: the export capability.
    CommandMetadata {
        id: "export_notebook_timeline",
        category: "reports",
        label: command_ids::CMD_EXPORT_NOTEBOOK_TIMELINE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_TIMELINE_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // Writes the notebook file: the save capability (signex-cli `SAVES`).
    CommandMetadata {
        id: "mark_notebook_attachment_valid",
        category: "design",
        label: command_ids::CMD_MARK_NOTEBOOK_ATTACHMENT_VALID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_MARK_VALID,
        ..CommandMetadata::DEFAULT
    },
    // Writes the copy and the notebook file: the save capability.
    CommandMetadata {
        id: "attach_notebook_file",
        category: "design",
        label: command_ids::CMD_ATTACH_NOTEBOOK_FILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_ATTACH,
        ..CommandMetadata::DEFAULT
    },
];
