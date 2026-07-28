//! Footprint-editor and view-surface command metadata. Split from
//! `keymap/catalog.rs`; entries verbatim.

use super::{CommandGroup, CommandMetadata};

pub(super) const THREE_D: &[CommandMetadata] = &[
    CommandMetadata {
        id: "footprint_mode_pads",
        category: "library",
        label: "Switch footprint editor to Pads mode",
        menu_label: None,
        group: CommandGroup::ThreeD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_mode_sketch",
        category: "library",
        label: "Switch footprint editor to Sketch mode",
        menu_label: None,
        group: CommandGroup::ThreeD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "footprint_mode_view_3d",
        category: "library",
        label: "Switch footprint editor to 3D View mode",
        menu_label: None,
        group: CommandGroup::ThreeD,
        ..CommandMetadata::DEFAULT
    },
];
