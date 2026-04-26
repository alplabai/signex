//! Where-Used tab — list every project / sheet / instance using
//! this component. Phase 1: read from
//! [`signex_library::WhereUsedIndex`] which is populated on project
//! open/save (Phase 2 wires the ingest path).

use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::LibraryMessage;
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    _editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    // The index lives on `LibraryState`, but the editor view only
    // gets the editor state in Phase 1. Pull from the editor's
    // owned `component.where_used` once we add it (Phase 2). For
    // now we render the empty state with a TODO note.
    // TODO(v0.9-phase-2): plumb LibraryState.where_used through the
    // editor view so this list is live. For now we render the empty
    // state with the TODO note + manual jump form.

    let header = row![
        text("Project")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("Sheet")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text("Instance")
            .size(10)
            .color(muted)
            .width(Length::Fixed(96.0)),
        text("Pinned Version")
            .size(10)
            .color(muted)
            .width(Length::Fixed(120.0)),
    ]
    .padding([4, 4]);

    let mut col = column![header].spacing(2);
    // Phase 1: empty list — Phase 2 plumbs `LibraryState.where_used`
    // through the editor view so this populates live. The component
    // editor state itself does NOT carry use-sites; the index lives
    // on `LibraryState`.
    col = col.push(
        container(
            text("No use sites yet for this component.")
                .size(11)
                .color(muted),
        )
        .padding([10, 4]),
    );
    col = col.push(
        container(
            text(
                "TODO(v0.9-phase-2): pipe LibraryState.where_used into the editor view; click a \
                 row → LibraryMessage::JumpToUseSite(use_site).",
            )
            .size(10)
            .color(muted),
        )
        .padding([4, 4]),
    );

    container(scrollable(col).height(Length::Fill).width(Length::Fill))
        .style(crate::styles::modal_card(tokens))
        .padding(14)
        .into()
}
