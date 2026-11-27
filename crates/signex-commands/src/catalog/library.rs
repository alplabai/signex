//! Library commands: the library browser and Components panel, editing a
//! library symbol, the footprint editor modes and its pad moves, and the
//! actions of the library dialogs.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use super::{Operand, OperandKind};
use signex_i18n_ids::command_ids;

const fn required(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: true,
    }
}

const fn optional(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: false,
    }
}

/// Move Selection By on named footprint pads (#3013): the open footprint
/// `document`, the `pads` by number (comma separated) and the offset `by`.
const FOOTPRINT_MOVE_PADS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("pads", OperandKind::Text),
    required("by", OperandKind::Point),
];

/// Align Selection on named footprint pads (#3013): `horizontal` is `left`,
/// `center`, `right`, `space` or `none`; `vertical` is `top`, `center`,
/// `bottom`, `space` or `none`. Left out: `none`.
const FOOTPRINT_ALIGN_PADS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("pads", OperandKind::Text),
    optional("horizontal", OperandKind::Text),
    optional("vertical", OperandKind::Text),
];

/// Document Options (mockup row 132): the open `library` (its id or
/// `.snxlib` path) and the settings to change; one left out keeps its value.
const LIBRARY_DOCUMENT_OPTIONS: &[Operand] = &[
    required("library", OperandKind::Text),
    optional("sheet_color", OperandKind::Text),
    optional("grid", OperandKind::Bool),
    optional("grid_spacing", OperandKind::Length),
    optional("unit", OperandKind::Text),
];

/// Library Updates (mockup row 135): the sheet whose updates wait, the
/// parts to update by designator (or `all`), and the conflicts that take
/// the library's value (`<designator>:<field>`).
const LIBRARY_UPDATES: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("designators", OperandKind::Text),
    optional("use_library", OperandKind::Text),
];

/// Copy to Library (mockup row 134): the stock primitive by `name` and
/// `kind`, and the writable library `path` the copy goes to.
const STOCK_COPY: &[Operand] = &[
    required("name", OperandKind::Text),
    required("kind", OperandKind::Text),
    required("path", OperandKind::Path),
];

/// Library recovery (mockup row 138): the library `path`, the `action`,
/// and for `locate` the library found at another place.
const LIBRARY_RECOVERY: &[Operand] = &[
    required("path", OperandKind::Path),
    required("action", OperandKind::Text),
    optional("located", OperandKind::Path),
];

/// New Library (mockup row 133): the project, the `.snxlib` path and the
/// version-control options, off when left out.
const NEW_LIBRARY: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("git", OperandKind::Bool),
    optional("lfs", OperandKind::Bool),
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
    // #3013: the edits of Move Selection By and Align Selection (mockup row
    // 140) on the pads a caller names. One undo step of the footprint; the
    // file is written only by a save.
    CommandMetadata {
        id: "footprint_move_pads",
        category: "modify",
        label: command_ids::CMD_FOOTPRINT_MOVE_PADS,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::MUTATES,
        operands: FOOTPRINT_MOVE_PADS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_align_pads",
        category: "modify",
        label: command_ids::CMD_FOOTPRINT_ALIGN_PADS,
        menu_label: None,
        group: CommandGroup::ThreeD,
        enable: Enablement::RequiresDocument(DocumentKind::Footprint),
        flags: CommandFlags::MUTATES,
        operands: FOOTPRINT_ALIGN_PADS,
        ..CommandMetadata::DEFAULT
    },
    // #3013: the library dialogs' actions (mockup rows 132-138), each run
    // through the dialog's own messages. See docs/CLI.md, "Library dialogs".
    CommandMetadata {
        id: "set_library_document_options",
        category: "view",
        label: command_ids::CMD_SET_LIBRARY_DOCUMENT_OPTIONS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: LIBRARY_DOCUMENT_OPTIONS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_library_updates",
        category: "modify",
        label: command_ids::CMD_APPLY_LIBRARY_UPDATES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: LIBRARY_UPDATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "copy_stock_to_library",
        category: "file",
        label: command_ids::CMD_COPY_STOCK_TO_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: STOCK_COPY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "recover_library",
        category: "file",
        label: command_ids::CMD_RECOVER_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: LIBRARY_RECOVERY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "new_library",
        category: "file",
        label: command_ids::CMD_NEW_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: NEW_LIBRARY,
        ..CommandMetadata::DEFAULT
    },
];
