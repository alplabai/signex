//! Design Notebook tab commands (#2795, mockup row 195): compile the
//! project's notebook and report it, insert what the toolbar inserts, and
//! pick the tab's view and side panel. Each names the project with
//! `document`, so a CLI or an agent reaches the tab the person sees.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NOTEBOOK: &[CommandMetadata] = &[
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
