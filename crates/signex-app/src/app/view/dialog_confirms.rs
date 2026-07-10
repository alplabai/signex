//! Confirmation modals — Remove-from-Project, Close-Project confirm, and
//! Quit-app confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::dialog_widgets::{
    close_x_button, draggable_header, primary_button, secondary_button, wrap_modal,
};
use super::dialogs::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(super) fn view_remove_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_remove_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::state::ModalId::RemoveDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (560.0, 260.0))
    }

    fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Remove from Project")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::Remove(RemoveMsg::Close), theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = draggable_header(
            header_content,
            super::super::state::ModalId::RemoveDialog,
            self.interaction_state.last_mouse_pos,
        );

        let option_card =
            |title: &'static str, subtitle: &'static str, msg: Message| -> Element<'_, Message> {
                let title_owned = title.to_string();
                let subtitle_owned = subtitle.to_string();
                button(
                    column![
                        text(format!("\u{2192} {}", title_owned))
                            .size(12)
                            .color(text_c),
                        text(subtitle_owned).size(10).color(text_muted),
                    ]
                    .spacing(4)
                    .padding([2, 0]),
                )
                .on_press(msg)
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        }
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.02),
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 4.0.into(),
                            color: border_c,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
            };

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "Please choose how to remove file \"{}\"",
                        st.display_name
                    ))
                    .size(11)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(
                    column![
                        option_card(
                            "Delete file",
                            "File will be removed from project and permanently deleted.",
                            Message::Remove(RemoveMsg::Confirm(
                                crate::app::RemoveChoice::DeleteFile,
                            )),
                        ),
                        option_card(
                            "Exclude from project",
                            "File will be excluded from project but left in local folder. Not recommended for projects stored in version control.",
                            Message::Remove(RemoveMsg::Confirm(
                                crate::app::RemoveChoice::ExcludeFromProject,
                            )),
                        ),
                    ]
                    .spacing(8)
                )
                .padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Remove(RemoveMsg::Close),
                            text_c,
                            border_c,
                        ),
                    ],
                )
                .padding([14, 14]),
            ]
            .width(560),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        // Reuse the rename modal's offset slot — `RenameDialog` is
        // never up at the same time as project-close confirm, and
        // they're roughly the same footprint, so reusing the slot
        // keeps modal-offset state simpler.
        let offset = (0.0, 0.0);
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 360.0))
    }

    fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Close Project — Unsaved Edits")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = draggable_header(
            header_content,
            super::super::state::ModalId::RenameDialog,
            self.interaction_state.last_mouse_pos,
        );

        // List of dirty filenames, scrollable in case the project
        // has dozens of unsaved sheets. Filenames only — paths would
        // overflow narrow modals and the project context already
        // implies the parent dir.
        let mut file_rows: Vec<Element<'_, Message>> = Vec::new();
        for path in &st.dirty_paths {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed)")
                .to_string();
            file_rows.push(
                container(text(name).size(11).color(text_c))
                    .padding([4, 8])
                    .into(),
            );
        }
        let file_list = scrollable(column(file_rows).spacing(0))
            .height(Length::Fixed(160.0))
            .width(Length::Fill);

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "'{}' has {} unsaved file(s). What would you like to do?",
                        st.project_name,
                        st.dirty_paths.len()
                    ))
                    .size(11)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(file_list).padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        secondary_button(
                            "Discard All",
                            Message::Project(ProjectMsg::CloseConfirm(
                                ProjectCloseChoice::DiscardAll,
                            )),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Save All",
                            Some(Message::Project(ProjectMsg::CloseConfirm(
                                ProjectCloseChoice::SaveAll,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(520),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(super) fn view_app_quit_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_app_quit_confirm_body();
        // Reuse the rename modal's centred offset slot — the app-quit
        // confirm is never up at the same time as the rename dialog.
        let offset = (0.0, 0.0);
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 360.0))
    }

    fn view_app_quit_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.app_quit_confirm else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Exit Signex — Unsaved Edits")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::Cancel)),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = draggable_header(
            header_content,
            super::super::state::ModalId::RenameDialog,
            self.interaction_state.last_mouse_pos,
        );

        // List of dirty filenames across the whole workspace,
        // scrollable in case many files are unsaved. Basenames only —
        // full paths would overflow the modal.
        let mut file_rows: Vec<Element<'_, Message>> = Vec::new();
        for path in &st.dirty_paths {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed)")
                .to_string();
            file_rows.push(
                container(text(name).size(11).color(text_c))
                    .padding([4, 8])
                    .into(),
            );
        }
        let file_list = scrollable(column(file_rows).spacing(0))
            .height(Length::Fixed(160.0))
            .width(Length::Fill);

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "You have {} unsaved file(s). Save before exiting?",
                        st.dirty_paths.len()
                    ))
                    .size(11)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(file_list).padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::Cancel
                            )),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        secondary_button(
                            "Discard All",
                            Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::DiscardAll,
                            )),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Save All",
                            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::SaveAll,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(520),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
