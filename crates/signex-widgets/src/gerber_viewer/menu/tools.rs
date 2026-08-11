use super::*;

pub(super) fn view(
    state: &GerberViewerState,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    Item::with_menu(
        root_button("Tools", colors),
        dropdown(vec![
            leaf_if(
                "List DCodes ...",
                None,
                GerberViewerMessage::ToggleDCodeList,
                !state.layers.is_empty(),
                colors,
            ),
            leaf_if(
                "Show Source ...",
                None,
                GerberViewerMessage::ToggleSourceView,
                state.active_layer.is_some(),
                colors,
            ),
            leaf(
                "Measure Tool",
                None,
                GerberViewerMessage::ToggleMeasurement,
                colors,
            ),
            leaf(
                "Edit Grids ...",
                None,
                GerberViewerMessage::OpenGridEditor,
                colors,
            ),
            leaf_if(
                "Clear Current Layer ...",
                None,
                GerberViewerMessage::ClearCurrentLayer,
                state.active_layer.is_some(),
                colors,
            ),
        ]),
    )
}
