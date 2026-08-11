use super::*;

pub(super) fn view(
    state: &GerberViewerState,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    let units_menu = Item::with_menu(
        submenu_button("Units", colors),
        dropdown(vec![
            checked_leaf(
                "Inches",
                state.display_unit == GerberDisplayUnit::Inches,
                GerberViewerMessage::SetDisplayUnit(GerberDisplayUnit::Inches),
                colors,
            ),
            checked_leaf(
                "Mils",
                state.display_unit == GerberDisplayUnit::Mils,
                GerberViewerMessage::SetDisplayUnit(GerberDisplayUnit::Mils),
                colors,
            ),
            checked_leaf(
                "Millimeters",
                state.display_unit == GerberDisplayUnit::Millimetres,
                GerberViewerMessage::SetDisplayUnit(GerberDisplayUnit::Millimetres),
                colors,
            ),
        ]),
    );

    Item::with_menu(
        root_button("View", colors),
        dropdown(vec![
            leaf("Zoom In", None, GerberViewerMessage::ZoomBy(1.2), colors),
            leaf(
                "Zoom Out",
                None,
                GerberViewerMessage::ZoomBy(1.0 / 1.2),
                colors,
            ),
            leaf("Zoom to Fit", None, GerberViewerMessage::FitPage, colors),
            leaf(
                "Zoom to Selection Area",
                None,
                GerberViewerMessage::ToggleZoomSelection,
                colors,
            ),
            leaf("Refresh", None, GerberViewerMessage::RedrawViewport, colors),
            separator(colors),
            checked_leaf(
                "Show Grid",
                state.grid_visible,
                GerberViewerMessage::ToggleGridVisibility(!state.grid_visible),
                colors,
            ),
            units_menu,
            separator(colors),
            checked_leaf(
                "Sketch Flashed Items",
                state.sketch_flashes,
                GerberViewerMessage::ToggleSketchFlashes,
                colors,
            ),
            checked_leaf(
                "Sketch Lines",
                state.sketch_lines,
                GerberViewerMessage::ToggleSketchLines,
                colors,
            ),
            checked_leaf(
                "Sketch Polygons",
                state.sketch_polygons,
                GerberViewerMessage::ToggleSketchPolygons,
                colors,
            ),
            checked_leaf(
                "Show DCodes",
                state.show_d_code_labels,
                GerberViewerMessage::ToggleDCodeLabels,
                colors,
            ),
            checked_leaf(
                "Ghost Negative Objects",
                state.ghost_negative_objects,
                GerberViewerMessage::ToggleGhostNegativeObjects,
                colors,
            ),
            checked_leaf(
                "Show with Forced Opacity Mode",
                state.forced_opacity_mode,
                GerberViewerMessage::ToggleForcedOpacityMode,
                colors,
            ),
            checked_leaf(
                "Show in XOR Mode",
                state.compare_mode,
                GerberViewerMessage::ToggleCompareMode,
                colors,
            ),
            checked_leaf(
                "Interactive Layer View Mode",
                state.dim_inactive_layers,
                GerberViewerMessage::ToggleDimInactiveLayers,
                colors,
            ),
            checked_leaf(
                "Flip Gerber View",
                state.mirrored,
                GerberViewerMessage::ToggleMirrored,
                colors,
            ),
            separator(colors),
            checked_leaf(
                "Show Layers Manager",
                state.layer_manager_visible,
                GerberViewerMessage::ToggleLayerManager,
                colors,
            ),
            checked_leaf(
                "Show Highlight Panel",
                state.is_tool_panel_visible(GerberDockPanel::Highlight),
                GerberViewerMessage::ToggleHighlightPanel,
                colors,
            ),
            checked_leaf(
                "Show Grid Panel",
                state.is_tool_panel_visible(GerberDockPanel::Grid),
                GerberViewerMessage::ToggleGridPanel,
                colors,
            ),
        ]),
    )
}
