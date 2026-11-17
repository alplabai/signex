//! Interactive tool commands: the keys that steer an armed tool while it
//! runs (cancel, accept, vertices, segment posture, wiring mode).

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

pub(super) const TOOL: &[CommandMetadata] = &[
    CommandMetadata {
        id: "cancel_current_tool",
        category: "interactive",
        label: command_ids::CMD_CANCEL_CURRENT_TOOL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "placement_accept",
        category: "interactive",
        label: command_ids::CMD_PLACEMENT_ACCEPT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "placement_properties",
        category: "interactive",
        label: command_ids::CMD_PLACEMENT_PROPERTIES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "copy_attributes_or_add_vertex",
        category: "interactive",
        label: command_ids::CMD_COPY_ATTRIBUTES_OR_ADD_VERTEX,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "remove_last_vertex",
        category: "interactive",
        label: command_ids::CMD_REMOVE_LAST_VERTEX,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "undo_last_segment",
        category: "interactive",
        label: command_ids::CMD_UNDO_LAST_SEGMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "switch_segment_posture",
        category: "interactive",
        label: command_ids::CMD_SWITCH_SEGMENT_POSTURE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_wiring_mode",
        category: "interactive",
        label: command_ids::CMD_CYCLE_WIRING_MODE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cycle_wire_bus_graphic_mode",
        category: "interactive",
        label: command_ids::CMD_CYCLE_WIRE_BUS_GRAPHIC_MODE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
