//! Design rule check commands: run the DRC on the board in view and check
//! it headless; and check the project's requirements headless.

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
    // #2939: each part's height and where it came from, read-only.
    CommandMetadata {
        id: "query_part_heights",
        category: "validation",
        label: command_ids::CMD_QUERY_PART_HEIGHTS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // #2339: the project's requirements list run as checks, headless.
    CommandMetadata {
        id: "check_requirements",
        category: "validation",
        label: command_ids::CMD_CHECK_REQUIREMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_REQUIREMENTS,
        ..CommandMetadata::DEFAULT
    },
    // #2399: the DRC results panel and what it does. A violation is named by
    // its waiver key (`id`), the same in every run. No default keys.
    CommandMetadata {
        id: "open_drc_panel",
        category: "pcb",
        label: command_ids::CMD_OPEN_DRC_PANEL,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DRC_PANEL),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_drc_violation",
        category: "pcb",
        label: command_ids::CMD_SHOW_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: operands::DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "waive_drc_violation",
        category: "pcb",
        label: command_ids::CMD_WAIVE_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::WAIVE_DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "unwaive_drc_violation",
        category: "pcb",
        label: command_ids::CMD_UNWAIVE_DRC_VIOLATION,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: operands::DRC_VIOLATION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "filter_drc_results",
        category: "pcb",
        label: command_ids::CMD_FILTER_DRC_RESULTS,
        menu_label: None,
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::NONE,
        operands: operands::DRC_FILTER,
        ..CommandMetadata::DEFAULT
    },
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
