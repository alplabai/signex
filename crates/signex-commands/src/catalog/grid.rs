//! Grid commands: the fast and snap grids, the grid picker and properties,
//! and the electrical and visible grids.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const GRID: &[CommandMetadata] = &[
    CommandMetadata {
        id: "cycle_fast_grid",
        category: "view",
        label: command_ids::CMD_CYCLE_FAST_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_snap_grid_backward",
        category: "view",
        label: command_ids::CMD_CYCLE_SNAP_GRID_BACKWARD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_snap_grid_forward",
        category: "view",
        // The key opens the snap-grid picker (footprint editor); no size steps (#525).
        label: command_ids::CMD_CYCLE_SNAP_GRID_FORWARD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "fast_grid_1",
        category: "view",
        label: command_ids::CMD_FAST_GRID_1,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "fast_grid_2",
        category: "view",
        label: command_ids::CMD_FAST_GRID_2,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_grid",
        category: "view",
        label: command_ids::CMD_NEXT_GRID,
        menu_label: Some(command_ids::CMD_MENU_NEXT_GRID),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "previous_grid",
        category: "view",
        label: command_ids::CMD_PREVIOUS_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_grid_picker",
        category: "view",
        label: command_ids::CMD_OPEN_GRID_PICKER,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "open_grid_properties",
        category: "view",
        label: command_ids::CMD_OPEN_GRID_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_electrical_grid",
        category: "view",
        label: command_ids::CMD_TOGGLE_ELECTRICAL_GRID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_visible_grid",
        category: "view",
        label: command_ids::CMD_TOGGLE_VISIBLE_GRID,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_VISIBLE_GRID),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
