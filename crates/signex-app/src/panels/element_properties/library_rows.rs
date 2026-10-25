//! The Library / out-of-date rows shared by the symbol and power-port
//! Properties views (#912).

use super::super::*;

/// Append the readable `Library` row and, when the sheet's embedded copy
/// differs from the library, the "out of date" row with its Update button.
/// Update replaces the sheet's shared copy, so it affects every instance of
/// the part on the sheet - the tooltip says so.
pub(super) fn push_library_rows<'a>(
    mut c: Column<'a, PanelMsg>,
    id: uuid::Uuid,
    library_name: &str,
    drifted: bool,
    muted: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Column<'a, PanelMsg> {
    if !library_name.is_empty() {
        c = c.push(form_input_row(
            "Library",
            library_name,
            muted,
            input_bg,
            input_bdr,
        ));
    }
    if drifted {
        let update = iced::widget::button(iced::widget::text("Update from library").size(11))
            .padding([2, 8])
            .on_press(PanelMsg::UpdateSymbolFromLibrary(id));
        c = c.push(
            iced::widget::row![
                iced::widget::text("Library: out of date")
                    .size(11)
                    .color(muted),
                iced::widget::tooltip(
                    update,
                    iced::widget::text("Updates every instance of this part on the sheet").size(11),
                    iced::widget::tooltip::Position::Bottom,
                ),
            ]
            .spacing(8)
            .padding([2, 8])
            .align_y(iced::Alignment::Center),
        );
    }
    c
}
