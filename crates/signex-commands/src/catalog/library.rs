//! Library commands: the library browser and Components panel, editing a
//! library symbol, and the footprint editor modes.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

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
];
