//! Command metadata for the schematic Active Bar's actions.
//!
//! Split out of `schematic.rs` rather than appended to it: the Active Bar
//! contributes more commands than every other schematic surface combined,
//! and `schematic.rs` would land near the 1000-line file cap. Entries are
//! `CommandGroup::Schematic` all the same — the file boundary is about
//! size, not grouping.
//!
//! The parameterised families are deliberately absent: `PlacePower*` (11),
//! `NetColor*` (8) and the four selection-arm modes each enumerate a value
//! domain and want one command plus an argument, which needs the
//! `CommandArgs` boundary (#367 / #366). Minting 23 frozen ids for them now
//! would be 23 names to deprecate later. See
//! `docs/audit/command-registry-action-surface-2026-07-25.md` §4.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};

pub(super) const ACTIVE_BAR: &[CommandMetadata] = &[
    CommandMetadata {
        id: "tool_select",
        category: "select",
        label: "Tool select",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "lasso_select",
        category: "select",
        label: "Lasso select",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "select_connection",
        category: "select",
        label: "Select connection",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "drag",
        category: "modify",
        label: "Drag",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_selection_xy",
        category: "modify",
        label: "Move selection x y",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "drag_selection",
        category: "modify",
        label: "Drag selection",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "move_to_front",
        category: "modify",
        label: "Move to front",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_selection",
        category: "modify",
        label: "Rotate selection",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rotate_selection_cw",
        category: "modify",
        label: "Rotate selection c w",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_selected_x",
        category: "modify",
        label: "Flip selected x",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_selected_y",
        category: "modify",
        label: "Flip selected y",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_to_front",
        category: "modify",
        label: "Bring to front",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_to_back",
        category: "modify",
        label: "Send to back",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bring_to_front_of",
        category: "modify",
        label: "Bring to front of",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "send_to_back_of",
        category: "modify",
        label: "Send to back of",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_sheet_symbol",
        category: "place",
        label: "Place sheet symbol",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_sheet_entry",
        category: "place",
        label: "Place sheet entry",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_device_sheet_symbol",
        category: "place",
        label: "Place device sheet symbol",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_reuse_block",
        category: "place",
        label: "Place reuse block",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_left",
        category: "modify",
        label: "Align left",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_right",
        category: "modify",
        label: "Align right",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_horizontal_centers",
        category: "modify",
        label: "Align horizontal centers",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "distribute_horizontally",
        category: "modify",
        label: "Distribute horizontally",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_top",
        category: "modify",
        label: "Align top",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_bottom",
        category: "modify",
        label: "Align bottom",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_vertical_centers",
        category: "modify",
        label: "Align vertical centers",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "distribute_vertically",
        category: "modify",
        label: "Distribute vertically",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "align_to_grid",
        category: "modify",
        label: "Align to grid",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_wire",
        category: "place",
        label: "Draw wire",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bus",
        category: "place",
        label: "Draw bus",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_bus_entry",
        category: "place",
        label: "Place bus entry",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_signal_harness",
        category: "place",
        label: "Place signal harness",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_connector",
        category: "place",
        label: "Place harness connector",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_harness_entry",
        category: "place",
        label: "Place harness entry",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_port",
        category: "place",
        label: "Place port",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_off_sheet_connector",
        category: "place",
        label: "Place off sheet connector",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_parameter_set",
        category: "place",
        label: "Place parameter set",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_diff_pair",
        category: "place",
        label: "Place diff pair",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_blanket",
        category: "place",
        label: "Place blanket",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_string",
        category: "place",
        label: "Place text string",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_text_frame",
        category: "place",
        label: "Place text frame",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_note",
        category: "place",
        label: "Place note",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_arc",
        category: "place",
        label: "Draw arc",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_full_circle",
        category: "place",
        label: "Draw full circle",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_elliptical_arc",
        category: "place",
        label: "Draw elliptical arc",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_ellipse",
        category: "place",
        label: "Draw ellipse",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_line",
        category: "place",
        label: "Draw line",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_rectangle",
        category: "place",
        label: "Draw rectangle",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_round_rectangle",
        category: "place",
        label: "Draw round rectangle",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_polygon",
        category: "place",
        label: "Draw polygon",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "draw_bezier",
        category: "place",
        label: "Draw bezier",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_graphic",
        category: "place",
        label: "Place graphic",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_net_color",
        category: "view",
        label: "Clear net color",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "clear_all_net_colors",
        category: "view",
        label: "Clear all net colors",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "place_component",
        category: "place",
        label: "Place component",
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
