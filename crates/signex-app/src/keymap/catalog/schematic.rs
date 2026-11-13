//! Schematic-surface command metadata — placement, net, annotation,
//! sheet-navigation. Split from `keymap/catalog.rs`; entries verbatim.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, operands};
use signex_i18n::command_ids;

pub(super) const SCHEMATIC: &[CommandMetadata] = &[
    CommandMetadata {
        id: "annotate_schematic",
        category: "design",
        label: command_ids::CMD_ANNOTATE_SCHEMATIC,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "annotate_schematic_quietly",
        category: "design",
        label: command_ids::CMD_ANNOTATE_SCHEMATIC_QUIETLY,
        menu_label: Some(command_ids::CMD_MENU_ANNOTATE_SCHEMATIC_QUIETLY),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "assign_footprints",
        category: "design",
        label: command_ids::CMD_ASSIGN_FOOTPRINTS,
        menu_label: Some(command_ids::CMD_MENU_ASSIGN_FOOTPRINTS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_net_highlighting",
        category: "select",
        label: command_ids::CMD_CLEAR_NET_HIGHLIGHTING,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_graphic_line",
        category: "place",
        label: command_ids::CMD_DRAW_GRAPHIC_LINE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::SEGMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_hierarchical_sheet",
        category: "place",
        label: command_ids::CMD_DRAW_HIERARCHICAL_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "force_annotate_all_schematics",
        category: "design",
        label: command_ids::CMD_FORCE_ANNOTATE_ALL_SCHEMATICS,
        menu_label: Some(command_ids::CMD_MENU_FORCE_ANNOTATE_ALL_SCHEMATICS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "go_to_net_partner",
        category: "navigation",
        label: command_ids::CMD_GO_TO_NET_PARTNER,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_net_under_cursor",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_NET_UNDER_CURSOR,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_related_net_objects",
        category: "select",
        label: command_ids::CMD_HIGHLIGHT_RELATED_NET_OBJECTS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "create_sheet_from_sheet_symbol",
        category: "design",
        label: command_ids::CMD_CREATE_SHEET_FROM_SHEET_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_CREATE_SHEET_FROM_SHEET_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "leave_sheet",
        category: "navigation",
        label: command_ids::CMD_LEAVE_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "navigate_up_hierarchy",
        category: "navigation",
        label: command_ids::CMD_NAVIGATE_UP_HIERARCHY,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_highlighted_net_item",
        category: "select",
        label: command_ids::CMD_NEXT_HIGHLIGHTED_NET_ITEM,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_sheet",
        category: "navigation",
        label: command_ids::CMD_NEXT_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_child_sheet",
        category: "navigation",
        label: command_ids::CMD_OPEN_CHILD_SHEET,
        menu_label: Some(command_ids::CMD_MENU_OPEN_CHILD_SHEET),
        group: CommandGroup::Schematic,
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
        id: "open_net_color_palette",
        category: "view",
        label: command_ids::CMD_OPEN_NET_COLOR_PALETTE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus",
        category: "place",
        label: command_ids::CMD_PLACE_BUS,
        menu_label: Some(command_ids::CMD_MENU_PLACE_BUS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::BUS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_compile_mask",
        category: "place",
        label: command_ids::CMD_PLACE_COMPILE_MASK,
        menu_label: Some(command_ids::CMD_MENU_PLACE_COMPILE_MASK),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_design_block",
        category: "place",
        label: command_ids::CMD_PLACE_DESIGN_BLOCK,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_global_label",
        category: "place",
        label: command_ids::CMD_PLACE_GLOBAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_GLOBAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_hierarchical_label",
        category: "place",
        label: command_ids::CMD_PLACE_HIERARCHICAL_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_HIERARCHICAL_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_junction",
        category: "place",
        label: command_ids::CMD_PLACE_JUNCTION,
        menu_label: Some(command_ids::CMD_MENU_PLACE_JUNCTION),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::JUNCTION,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_local_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_LOCAL_NET_LABEL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_net_label",
        category: "place",
        label: command_ids::CMD_PLACE_NET_LABEL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NET_LABEL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::LABEL,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_connect",
        category: "place",
        label: command_ids::CMD_PLACE_NO_CONNECT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_no_erc",
        category: "place",
        label: command_ids::CMD_PLACE_NO_ERC,
        menu_label: Some(command_ids::CMD_MENU_PLACE_NO_ERC),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::AT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_power_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_POWER_SYMBOL,
        menu_label: Some(command_ids::CMD_MENU_PLACE_POWER_SYMBOL),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::POWER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_symbol",
        category: "place",
        label: command_ids::CMD_PLACE_SYMBOL,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text",
        category: "place",
        label: command_ids::CMD_PLACE_TEXT,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::TEXT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE,
        menu_label: Some(command_ids::CMD_MENU_PLACE_WIRE),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: operands::WIRE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_wire_to_bus_entry",
        category: "place",
        label: command_ids::CMD_PLACE_WIRE_TO_BUS_ENTRY,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_highlighted_net_item",
        category: "select",
        label: command_ids::CMD_PREVIOUS_HIGHLIGHTED_NET_ITEM,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_sheet",
        category: "navigation",
        label: command_ids::CMD_PREVIOUS_SHEET,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
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
        id: "open_erc_dialog",
        category: "validation",
        label: command_ids::CMD_OPEN_ERC_DIALOG,
        menu_label: Some(command_ids::CMD_MENU_OPEN_ERC_DIALOG),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
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
        id: "number_schematic_sheets",
        category: "design",
        label: command_ids::CMD_NUMBER_SCHEMATIC_SHEETS,
        menu_label: Some(command_ids::CMD_MENU_NUMBER_SCHEMATIC_SHEETS),
        group: CommandGroup::Schematic,
        // A project sheet, not the active tab, is what it needs: the root
        // sheet stands in with no document open, so the gate is
        // `Needs::ProjectSheet` (`app/command/needs.rs`).
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "reset_schematic_designators",
        category: "design",
        label: command_ids::CMD_RESET_SCHEMATIC_DESIGNATORS,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sheet_navigation_back",
        category: "navigation",
        label: command_ids::CMD_SHEET_NAVIGATION_BACK,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sheet_navigation_forward",
        category: "navigation",
        label: command_ids::CMD_SHEET_NAVIGATION_FORWARD,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "sync_sheet_pins",
        category: "design",
        label: command_ids::CMD_SYNC_SHEET_PINS,
        menu_label: Some(command_ids::CMD_MENU_SYNC_SHEET_PINS),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        operands: operands::SYNC_SHEET,
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
        id: "open_project_options",
        category: "file",
        label: command_ids::CMD_OPEN_PROJECT_OPTIONS,
        menu_label: Some(command_ids::CMD_MENU_OPEN_PROJECT_OPTIONS),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #1312: the Update PCB dialog, from a schematic or (Import Changes) a
    // board of a project. Execute is one undo step on the board.
    CommandMetadata {
        id: "update_pcb_from_schematic",
        category: "pcb_sync",
        label: command_ids::CMD_UPDATE_PCB_FROM_SCHEMATIC,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::APPLY,
        ..CommandMetadata::DEFAULT
    },
    // #1312: Back Annotate, from a schematic or (Update Schematics) a board
    // of a project. Execute is one undo group across the changed sheets.
    // No default key.
    CommandMetadata {
        id: "back_annotate_schematics",
        category: "pcb_sync",
        label: command_ids::CMD_BACK_ANNOTATE_SCHEMATICS,
        menu_label: Some(command_ids::CMD_MENU_BACK_ANNOTATE_SCHEMATICS),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
