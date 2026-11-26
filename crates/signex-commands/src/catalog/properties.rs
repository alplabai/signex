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
    // #1114, mockup row 203: a text note's Format row, Plain or Rich text.
    CommandMetadata {
        id: "set_text_note_format",
        category: "edit",
        label: command_ids::CMD_SET_TEXT_NOTE_FORMAT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT_NOTE_FORMAT,
        ..CommandMetadata::DEFAULT
    },
    // #1114, mockup row 203: a rich note's Frame and Size sections, Apply of
    // its in-place editor and the editor's actions (open, Cancel, a toolbar
    // button, Ctrl+click on a link), and its compile with what it found.
    CommandMetadata {
        id: "set_text_note_frame",
        category: "edit",
        label: command_ids::CMD_SET_TEXT_NOTE_FRAME,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT_NOTE_FRAME,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "set_text_note_size",
        category: "edit",
        label: command_ids::CMD_SET_TEXT_NOTE_SIZE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT_NOTE_SIZE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_rich_note_edit",
        category: "edit",
        label: command_ids::CMD_APPLY_RICH_NOTE_EDIT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::RICH_NOTE_APPLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_rich_note",
        category: "edit",
        label: command_ids::CMD_EDIT_RICH_NOTE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: operands::RICH_NOTE_EDIT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cancel_rich_note_edit",
        category: "edit",
        label: command_ids::CMD_CANCEL_RICH_NOTE_EDIT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: operands::RICH_NOTE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "wrap_rich_note_selection",
        category: "edit",
        label: command_ids::CMD_WRAP_RICH_NOTE_SELECTION,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: operands::RICH_NOTE_WRAP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_rich_note_link",
        category: "edit",
        label: command_ids::CMD_OPEN_RICH_NOTE_LINK,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: operands::RICH_NOTE_LINK,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "compile_rich_note",
        category: "edit",
        label: command_ids::CMD_COMPILE_RICH_NOTE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: operands::RICH_NOTE,
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
