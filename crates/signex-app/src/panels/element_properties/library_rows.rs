//! The Library / out-of-date rows shared by the symbol and power-port
//! Properties views (#912).

use super::super::*;

/// The "Symbol" pick list of a placed symbol whose library row offers
/// alternate drawings (#1230). Nothing when there is only the default.
pub(super) fn push_symbol_choice_row<'a>(
    c: Column<'a, PanelMsg>,
    id: uuid::Uuid,
    choices: &[SymbolChoiceItem],
    muted: Color,
) -> Column<'a, PanelMsg> {
    let Some(current) = choices.iter().find(|i| i.selected) else {
        return c;
    };
    if choices.len() < 2 {
        return c;
    }
    let labels: Vec<String> = choices.iter().map(|i| i.label.clone()).collect();
    let items = choices.to_vec();
    c.push(form_pick_row(
        "Symbol",
        labels,
        current.label.clone(),
        move |label| {
            let lib_id = items
                .iter()
                .find(|i| i.label == label)
                .map(|i| i.lib_id.clone())
                .unwrap_or_default();
            PanelMsg::SwitchSymbolChoice(id, lib_id)
        },
        muted,
    ))
}

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
