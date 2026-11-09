//! PCB-surface command metadata — placement now; routing / layer / via /
//! DRC later.

use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};

pub(super) const PCB: &[CommandMetadata] = &[CommandMetadata {
    // Flip the selected footprints to the other side of the board (#2240).
    // No default key: a profile may bind one.
    id: "flip_selection",
    category: "modify",
    label: "Flip selection to the other board side",
    menu_label: Some("Flip Selection"),
    group: CommandGroup::Pcb,
    enable: Enablement::RequiresDocument(DocumentKind::Pcb),
    flags: CommandFlags::MUTATES,
    ..CommandMetadata::DEFAULT
}];
