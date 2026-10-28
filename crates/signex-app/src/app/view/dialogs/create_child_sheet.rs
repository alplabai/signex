//! The "Create Sheet" dialog (#1275): where to write the file of a sheet
//! symbol whose file does not exist yet. A confirm-class dialog: it opens in
//! its own window and locks the main window (docs/DETACHED_MODALS.md).

use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Element, Length};

use super::widgets::{close_x_button, primary_button, secondary_button, wrap_modal};
use super::*;
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(in crate::app::view) fn view_create_child_sheet_confirm(&self) -> Element<'_, Message> {
        wrap_modal(
            self.view_create_child_sheet_body(),
            (0.0, 0.0),
            self.ui_state.window_size,
            (560.0, 260.0),
        )
    }

    /// The dialog body: the card in the main window, or the content of its
    /// own window (no header; the native title bar has the title).
    pub(in crate::app::view) fn view_create_child_sheet_body(&self) -> Element<'_, Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_ref() else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let error_c = iced::Color::from_rgb(0.90, 0.35, 0.30);
        fn msg(m: ProjectMsg) -> Message {
            Message::Project(m)
        }
        let later = msg(ProjectMsg::CreateChildSheetAnswered { create: false });
        let create = msg(ProjectMsg::CreateChildSheetAnswered { create: true });

        let header_content: Element<'_, Message> = container(
            row![
                text("Create Sheet")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(later.clone(), self.ui_state.theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::CreateChildSheet,
        );

        // The folder the path is relative to, shown from the project folder
        // down; the full path is the tooltip.
        let parent_dir = confirm.parent.parent().unwrap_or(std::path::Path::new(""));
        let full_folder = parent_dir.display().to_string();
        let folder = match parent_dir.strip_prefix(&confirm.project_dir) {
            Ok(rel) if rel.as_os_str().is_empty() => "the project folder".to_owned(),
            Ok(rel) => format!("{}/", rel.display()),
            Err(_) => full_folder.clone(),
        };
        let field = text_input("sheet.snxsch", &confirm.path_text)
            .on_input(|t| msg(ProjectMsg::CreateChildSheetPathChanged(t)))
            .on_submit(create.clone())
            .padding([6, 8])
            .size(12)
            .width(Length::Fill);
        let note: Element<'_, Message> = match &confirm.error {
            Some(error) => text(error.clone()).size(11).color(error_c).into(),
            None => Space::new().height(0).into(),
        };
        let confirm_button: Element<'_, Message> = if confirm.exists {
            primary_button(
                "Open it",
                Some(msg(ProjectMsg::CreateChildSheetOpenExisting)),
                border_c,
            )
        } else {
            primary_button("Create", Some(create), border_c)
        };

        container(
            column![
                header,
                container(
                    column![
                        text(format!(
                            "Sheet '{}' has no file yet. Where should it be created?",
                            confirm.name
                        ))
                        .size(11)
                        .color(text_muted),
                        iced::widget::tooltip(
                            text(format!("Relative to {folder}"))
                                .size(10)
                                .color(text_muted),
                            text(full_folder).size(10),
                            iced::widget::tooltip::Position::Bottom,
                        ),
                        row![
                            field,
                            secondary_button(
                                "Browse...",
                                msg(ProjectMsg::CreateChildSheetBrowse),
                                text_c,
                                border_c,
                            ),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                        note,
                    ]
                    .spacing(8),
                )
                .padding([14, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button("Later", later, text_c, border_c),
                        Space::new().width(8),
                        confirm_button,
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([10, 14]),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }
}
