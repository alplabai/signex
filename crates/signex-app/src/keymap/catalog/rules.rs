//! The Rules manager commands (#2398): the Rules tab, and the `rules_*`
//! commands an agent or the CLI drives it with. The reads answer with data;
//! each edit is one undo step on the project's rule history.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n::command_ids;

pub(super) const RULES: &[CommandMetadata] = &[
    CommandMetadata {
        id: "open_rules",
        category: "design",
        label: command_ids::CMD_OPEN_RULES,
        menu_label: Some(command_ids::CMD_MENU_OPEN_RULES),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_list",
        category: "validation",
        label: command_ids::CMD_RULES_LIST,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RULES_LIST,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_get",
        category: "validation",
        label: command_ids::CMD_RULES_GET,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RULES_ONE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_add",
        category: "design",
        label: command_ids::CMD_RULES_ADD,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_set",
        category: "design",
        label: command_ids::CMD_RULES_SET,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_remove",
        category: "design",
        label: command_ids::CMD_RULES_REMOVE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_ONE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_enable",
        category: "design",
        label: command_ids::CMD_RULES_ENABLE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_ENABLE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_effective",
        category: "validation",
        label: command_ids::CMD_RULES_EFFECTIVE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RULES_EFFECTIVE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_explain",
        category: "validation",
        label: command_ids::CMD_RULES_EXPLAIN,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RULES_EXPLAIN,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_conflicts",
        category: "validation",
        label: command_ids::CMD_RULES_CONFLICTS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // Steps 5 and 6: the class matrix, named rule sets and presets.
    CommandMetadata {
        id: "rules_matrix_get",
        category: "validation",
        label: command_ids::CMD_RULES_MATRIX_GET,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_matrix_set",
        category: "design",
        label: command_ids::CMD_RULES_MATRIX_SET,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_MATRIX_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_set_create",
        category: "design",
        label: command_ids::CMD_RULES_SET_CREATE,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_SET_CREATE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_set_edit",
        category: "design",
        label: command_ids::CMD_RULES_SET_EDIT,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_SET_EDIT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_set_apply",
        category: "design",
        label: command_ids::CMD_RULES_SET_APPLY,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_SET_APPLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_preset_diff",
        category: "validation",
        label: command_ids::CMD_RULES_PRESET_DIFF,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RULES_PRESET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rules_preset_apply",
        category: "design",
        label: command_ids::CMD_RULES_PRESET_APPLY,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::RULES_PRESET,
        ..CommandMetadata::DEFAULT
    },
];
