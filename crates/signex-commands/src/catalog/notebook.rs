//! Design Notebook commands (#2795; mockup rows 195-202).

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const NOTEBOOK: &[CommandMetadata] = &[
    // #2945, mockup row 201: the design report from requirements, checks,
    // decisions, BOM, simulation and outputs, as its own notebook file. With
    // no operands it opens the options dialog for the project in view.
    CommandMetadata {
        id: "generate_design_notebook",
        category: "reports",
        label: command_ids::CMD_GENERATE_DESIGN_NOTEBOOK,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::GENERATE_DESIGN_NOTEBOOK,
        ..CommandMetadata::DEFAULT
    },
];
