//! PCB-surface command metadata: placement, the design rule check and the
//! board view (ratsnest, layers).

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};

pub(super) const PCB: &[CommandMetadata] = &[
    // Read-only: the report goes to the Messages panel, the board is not touched.
    CommandMetadata {
        id: "run_drc",
        category: "pcb",
        label: "Run the design rule check on the board",
        menu_label: Some("Design Rule Check"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    // Flip the selected footprints to the other side of the board (#2240).
    // No default key: a profile may bind one.
    CommandMetadata {
        id: "flip_selection",
        category: "modify",
        label: "Flip selection to the other board side",
        menu_label: Some("Flip Selection"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_ratsnest",
        category: "view",
        label: "Show or hide the ratsnest",
        menu_label: Some("Toggle Ratsnest"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "next_layer",
        category: "view",
        label: "Make the next copper layer active",
        menu_label: Some("Next Layer"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "prev_layer",
        category: "view",
        label: "Make the previous copper layer active",
        menu_label: Some("Previous Layer"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layer_visibility",
        category: "view",
        label: "Show or hide the active layer",
        menu_label: Some("Toggle Layer Visibility"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_all_layers",
        category: "view",
        label: "Show every layer",
        menu_label: Some("Show All Layers"),
        group: CommandGroup::Pcb,
        enable: Enablement::RequiresDocument(DocumentKind::Pcb),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "toggle_layers_panel",
        category: "view",
        label: "Show or hide the Layers panel",
        menu_label: Some("Layers"),
        group: CommandGroup::Pcb,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
