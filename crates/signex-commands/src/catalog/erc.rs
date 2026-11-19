//! Electrical rule check commands: run the ERC, check it headless and open
//! its dialog.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const ERC: &[CommandMetadata] = &[
    CommandMetadata {
        id: "run_erc",
        category: "validation",
        label: command_ids::CMD_RUN_ERC,
        menu_label: Some(command_ids::CMD_MENU_RUN_ERC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "check_erc",
        category: "validation",
        label: command_ids::CMD_CHECK_ERC,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::CHECK_ERC,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_erc_dialog",
        category: "validation",
        label: command_ids::CMD_OPEN_ERC_DIALOG,
        menu_label: Some(command_ids::CMD_MENU_OPEN_ERC_DIALOG),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // A dot on every pin end of the sheet that lies inside a wire with none
    // (2026-11-18); opening a sheet only reports such contacts. A CLI or an
    // agent's edit list names the sheet with `document`.
    CommandMetadata {
        id: "repair_pin_junctions",
        category: "validation",
        label: command_ids::CMD_REPAIR_PIN_JUNCTIONS,
        menu_label: Some(command_ids::CMD_MENU_REPAIR_PIN_JUNCTIONS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::OPTIONAL_DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
];
