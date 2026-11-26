//! Library commands: the library browser and Components panel, editing a
//! library symbol, and the footprint editor modes.

use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, Operand, OperandKind,
};
use signex_i18n_ids::command_ids;

const fn operand(name: &'static str, kind: OperandKind, required: bool) -> Operand {
    Operand {
        name,
        kind,
        required,
    }
}

/// `edit_symbol_pins` (#2817): the open `.snxsym` and, when it holds more
/// than one, the `symbol` by name; then `pins`, a JSON object of pin number
/// to the fields to set (or null to remove the pin), or `paste`, pin rows as
/// the Pin table's Paste rows reads them (`\n` between rows, `\t` or two
/// spaces between cells) with the column map `fields` and the paste `mode`
/// (`update_by_number`, `append`, `replace_all`). `dry_run` answers without
/// changing the symbol.
const SYMBOL_PINS: &[Operand] = &[
    operand("document", OperandKind::Document, true),
    operand("symbol", OperandKind::Text, false),
    operand("pins", OperandKind::Text, false),
    operand("paste", OperandKind::Text, false),
    operand("fields", OperandKind::Text, false),
    operand("mode", OperandKind::Text, false),
    operand("dry_run", OperandKind::Bool, false),
];

pub(super) const LIBRARY: &[CommandMetadata] = &[
    CommandMetadata {
        id: "open_library_browser",
        category: "library",
        label: command_ids::CMD_OPEN_LIBRARY_BROWSER,
        menu_label: Some(command_ids::CMD_MENU_OPEN_LIBRARY_BROWSER),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_library_symbol",
        category: "library",
        label: command_ids::CMD_EDIT_LIBRARY_SYMBOL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "edit_selected_symbol_in_symbol_editor",
        category: "library",
        label: command_ids::CMD_EDIT_SELECTED_SYMBOL_IN_SYMBOL_EDITOR,
        menu_label: Some(command_ids::CMD_MENU_EDIT_SELECTED_SYMBOL_IN_SYMBOL_EDITOR),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_components_panel",
        category: "panels",
        label: command_ids::CMD_OPEN_COMPONENTS_PANEL,
        menu_label: Some(command_ids::CMD_MENU_OPEN_COMPONENTS_PANEL),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_mode_pads",
        category: "library",
        label: command_ids::CMD_FOOTPRINT_MODE_PADS,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_mode_sketch",
        category: "library",
        label: command_ids::CMD_FOOTPRINT_MODE_SKETCH,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_mode_view_3d",
        category: "library",
        label: command_ids::CMD_FOOTPRINT_MODE_VIEW_3D,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_recompute_courtyard_outline",
        category: "library",
        label: command_ids::CMD_FOOTPRINT_RECOMPUTE_COURTYARD_OUTLINE,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2817: the Pin table tab's bulk edit and Paste rows (mockup row 122b)
    // for an agent or the CLI; one undo step on the symbol file. A person
    // uses the tab. No default key.
    CommandMetadata {
        id: "edit_symbol_pins",
        category: "edit",
        label: command_ids::CMD_EDIT_SYMBOL_PINS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresDocument(DocumentKind::Symbol),
        flags: CommandFlags::MUTATES,
        operands: SYMBOL_PINS,
        ..CommandMetadata::DEFAULT
    },
];
