//! Design rule check commands: run the DRC on the board in view and check
//! it headless.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const DRC: &[CommandMetadata] = &[
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
    CommandMetadata {
        id: "check_drc",
        category: "validation",
        label: command_ids::CMD_CHECK_DRC,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_DRC,
        ..CommandMetadata::DEFAULT
    },
];
