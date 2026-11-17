//! The Project tab's section commands (#1689-#1693): `project_settings`
//! reads every section as JSON; each edit is one undo step on the project's
//! General history, the same commit path as the tab's own controls. The
//! person reaches them through the tab (Project Options); an agent or the
//! CLI names the project with `document`.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const PROJECT: &[CommandMetadata] = &[
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
