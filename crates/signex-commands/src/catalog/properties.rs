//! Object property commands: edit an object's properties, designator,
//! value, footprint field and text, place its fields, open its datasheet,
//! swap the component and open the parameter manager.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const PROPERTIES: &[CommandMetadata] = &[
    CommandMetadata {
        id: "edit_object_properties",
        category: "edit",
        label: command_ids::CMD_EDIT_OBJECT_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_selected_object_properties",
        category: "edit",
        label: command_ids::CMD_EDIT_SELECTED_OBJECT_PROPERTIES,
        menu_label: Some(command_ids::CMD_MENU_EDIT_SELECTED_OBJECT_PROPERTIES),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_reference_designator",
        category: "edit",
        label: command_ids::CMD_EDIT_REFERENCE_DESIGNATOR,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_value",
        category: "edit",
        label: command_ids::CMD_EDIT_VALUE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_footprint_field",
        category: "edit",
        label: command_ids::CMD_EDIT_FOOTPRINT_FIELD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_text_in_place",
        category: "edit",
        label: command_ids::CMD_EDIT_TEXT_IN_PLACE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "autoplace_fields",
        category: "edit",
        label: command_ids::CMD_AUTOPLACE_FIELDS,
        menu_label: Some(command_ids::CMD_MENU_AUTOPLACE_FIELDS),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        operands: operands::PART_ON_DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_datasheet",
        category: "edit",
        label: command_ids::CMD_OPEN_DATASHEET,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DATASHEET),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "change_component",
        category: "edit",
        label: command_ids::CMD_CHANGE_COMPONENT,
        menu_label: Some(command_ids::CMD_MENU_CHANGE_COMPONENT),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_parameter_manager",
        category: "design",
        label: command_ids::CMD_OPEN_PARAMETER_MANAGER,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PARAMETER_MANAGER),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #1295: the Parameter Manager footer's Export… and Import… (no key, no
    // menu row). Import opens a preview; its Apply is the undo step.
    CommandMetadata {
        id: "export_parameters",
        category: "design",
        label: command_ids::CMD_EXPORT_PARAMETERS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "import_parameters",
        category: "design",
        label: command_ids::CMD_IMPORT_PARAMETERS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_symbol_parameter",
        category: "edit",
        label: command_ids::CMD_SET_SYMBOL_PARAMETER,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SYMBOL_PARAMETER,
        ..CommandMetadata::DEFAULT
    },
    // A sheet variable `${NAME}` and expressions resolve; the Properties
    // panel's Sheet Variables rows run this too.
    CommandMetadata {
        id: "set_sheet_variable",
        category: "edit",
        label: command_ids::CMD_SET_SHEET_VARIABLE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SHEET_VARIABLE,
        ..CommandMetadata::DEFAULT
    },
    // #2407: lock or unlock the selected tracks and vias (the board menu's
    // rows), or the ones `ids` names on `document`. One undo step.
    CommandMetadata {
        id: "lock_board_items",
        category: "modify",
        label: command_ids::CMD_LOCK_BOARD_ITEMS,
        menu_label: Some(command_ids::CMD_MENU_LOCK_BOARD_ITEMS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "unlock_board_items",
        category: "modify",
        label: command_ids::CMD_UNLOCK_BOARD_ITEMS,
        menu_label: Some(command_ids::CMD_MENU_UNLOCK_BOARD_ITEMS),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        operands: super::operands::BOARD_ITEMS,
        ..CommandMetadata::DEFAULT
    },
];
