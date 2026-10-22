use super::{CommandGroup, CommandMetadata};

pub(super) const GERBER: &[CommandMetadata] = &[
    CommandMetadata {
        id: "gerber_next_layer",
        category: "layers",
        label: "Next Gerber layer",
        menu_label: Some("Next Layer"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_previous_layer",
        category: "layers",
        label: "Previous Gerber layer",
        menu_label: Some("Previous Layer"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_move_layer_up",
        category: "layers",
        label: "Move active Gerber layer up",
        menu_label: Some("Move Layer Up"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_move_layer_down",
        category: "layers",
        label: "Move active Gerber layer down",
        menu_label: Some("Move Layer Down"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_sketch_flashes",
        category: "display",
        label: "Sketch Gerber flashed items",
        menu_label: Some("Sketch Flashed Items"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_sketch_lines",
        category: "display",
        label: "Sketch Gerber line items",
        menu_label: Some("Sketch Lines"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_sketch_polygons",
        category: "display",
        label: "Sketch Gerber polygon items",
        menu_label: Some("Sketch Polygons"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_show_d_codes",
        category: "display",
        label: "Show Gerber D-code labels",
        menu_label: Some("Show D-Codes"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_compare_layers",
        category: "display",
        label: "Compare visible Gerber layers",
        menu_label: Some("Show in XOR Mode"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_dim_inactive_layers",
        category: "display",
        label: "Dim inactive Gerber layers",
        menu_label: Some("Dim Inactive Layers"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_forced_opacity",
        category: "display",
        label: "Show Gerber layers with forced opacity",
        menu_label: Some("Show with Forced Opacity Mode"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_flip_view",
        category: "display",
        label: "Mirror the Gerber view",
        menu_label: Some("Flip Gerber View"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_clear_highlight",
        category: "display",
        label: "Clear Gerber highlight",
        menu_label: Some("Clear Highlight"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_export_native_pcb",
        category: "file",
        label: "Export Gerber artwork to native PCB",
        menu_label: Some("Export to PCB Editor"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_print",
        category: "file",
        label: "Print visible Gerber layers",
        menu_label: Some("Print"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "gerber_quit",
        category: "file",
        label: "Close the Gerber viewer",
        menu_label: Some("Quit"),
        group: CommandGroup::Gerber,
        ..CommandMetadata::DEFAULT
    },
];
