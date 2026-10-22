use super::*;

pub(super) fn view(
    state: &GerberViewerState,
    colors: MenuColors,
) -> Item<'static, GerberViewerMessage, Theme, Renderer> {
    let recent_files = recent_files_menu("Open Recent File", colors);

    Item::with_menu(
        root_button("File", colors),
        dropdown(vec![
            leaf(
                "New Gerber Document",
                None,
                GerberViewerMessage::NewDocument,
                colors,
            ),
            separator(colors),
            leaf_if(
                "Open File(s) ...",
                None,
                GerberViewerMessage::OpenFiles,
                !state.loading,
                colors,
            ),
            recent_files,
            separator(colors),
            leaf_if(
                "Clear All Layers",
                None,
                GerberViewerMessage::ClearAllLayers,
                !state.layers.is_empty(),
                colors,
            ),
            leaf_if(
                "Reload All Layers",
                None,
                GerberViewerMessage::ReloadAllLayers,
                !state.loading && !state.layers.is_empty(),
                colors,
            ),
            separator(colors),
            leaf_if(
                "Export to PCB Editor ...",
                None,
                GerberViewerMessage::ExportNativePcb,
                !state.layers.is_empty(),
                colors,
            ),
            separator(colors),
            leaf_if(
                "Print ...",
                Some("Ctrl+P"),
                GerberViewerMessage::PrintVisibleLayers,
                print::has_visible_layers(state),
                colors,
            ),
            separator(colors),
            leaf(
                "Quit",
                Some("Ctrl+Q"),
                GerberViewerMessage::CloseRequested,
                colors,
            ),
        ]),
    )
}
