use super::*;

#[test]
fn file_menu_contains_requested_commands_in_order() {
    assert_eq!(
        menu::FILE_MENU_LABELS,
        [
            "New Gerber Document",
            "Open File(s) ...",
            "Open Recent File",
            "Clear All Layers",
            "Reload All Layers",
            "Export to PCB Editor ...",
            "Print ...",
            "Quit",
        ],
    );
}

#[test]
fn view_menu_contains_requested_commands_in_order() {
    assert_eq!(
        menu::VIEW_MENU_LABELS,
        [
            "Zoom In",
            "Zoom Out",
            "Zoom to Fit",
            "Zoom to Selection Area",
            "Refresh",
            "Show Grid",
            "Units",
            "Sketch Flashed Items",
            "Sketch Lines",
            "Sketch Polygons",
            "Show DCodes",
            "Ghost Negative Objects",
            "Show with Forced Opacity Mode",
            "Show in XOR Mode",
            "Inactive Layer View Mode",
            "Flip Gerber View",
            "Show Layers Manager",
            "Show Highlight Panel",
            "Show Grid Panel",
        ],
    );
}

#[test]
fn tools_menu_contains_requested_commands_in_order() {
    assert_eq!(
        menu::TOOLS_MENU_LABELS,
        [
            "List DCodes ...",
            "Show Source ...",
            "Measure Tool",
            "Edit Grids ...",
            "Clear Current Layer ...",
        ],
    );
}
