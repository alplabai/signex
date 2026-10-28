//! "Create the missing sheet?" confirm (#1275): a sheet symbol was descended
//! into but its file does not exist yet.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};

use super::widgets::{close_x_button, primary_button, secondary_button, wrap_modal};
use super::*;
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(in crate::app::view) fn view_create_child_sheet_confirm(&self) -> Element<'_, Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_ref() else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let answer = |create| Message::Project(ProjectMsg::CreateChildSheetAnswered { create });

        let header = container(
            row![
                text("Create Sheet")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(answer(false), self.ui_state.theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens));

        let body = format!(
            "Sheet '{}' has no file yet. Create it at\n{}\nwith one port per sheet entry?",
            confirm.name,
            confirm.path.display()
        );
        let dialog = container(
            column![
                header,
                container(text(body).size(11).color(text_muted)).padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Cancel", answer(false), text_c, border_c),
                        Space::new().width(8),
                        primary_button("Create Sheet", Some(answer(true)), border_c),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(520),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        wrap_modal(
            dialog.into(),
            (0.0, 0.0),
            self.ui_state.window_size,
            (520.0, 200.0),
        )
    }
}
