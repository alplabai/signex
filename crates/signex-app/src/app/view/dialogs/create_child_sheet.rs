//! The "Create Sheet" dialog (#1275): where to write the file of a sheet
//! symbol whose file does not exist yet. A confirm-class dialog: it opens in
//! its own window and locks the main window (docs/DETACHED_MODALS.md).

use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Element, Length, Widget};

use super::widgets::{close_x_button, wrap_modal};
use super::*;
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_message};

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
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
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
                close_x_button(
                    later.clone(),
                    self.ui_state.effective_theme_id(),
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .boxed();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::CreateChildSheet,
        );

        // The folder the path is relative to, shown from the project folder
        // down; the full path is the tooltip.
        let parent_dir = confirm.parent.parent().unwrap_or(std::path::Path::new(""));
        let full_folder = parent_dir.display().to_string();
        let fold = crate::app::state::project_case::fold_of(
            self.document_state.document_project(&confirm.parent),
        );
        let folder = match crate::app::handlers::canvas::relative_within(
            &confirm.project_dir,
            parent_dir,
            fold,
        ) {
            Some(rel) if rel.as_os_str().is_empty() => "the project folder".to_owned(),
            Some(rel) => format!("{}/", rel.display()),
            None => full_folder.clone(),
        };
        let field = text_input("sheet.snxsch", &confirm.path_text)
            .on_input(|t| msg(ProjectMsg::CreateChildSheetPathChanged(t)))
            .on_submit(create.clone())
            .padding([6, 8])
            .size(TS.label)
            .width(Length::Fill);
        let note: Element<'_, Message> = match &confirm.error {
            Some(error) => text(error.clone())
                .size(TS.body)
                .color(crate::styles::tokens::danger(tokens))
                .boxed(),
            None => Space::new().height(0).boxed(),
        };
        let confirm_button: Element<'_, Message> = if confirm.exists {
            dialog_button(
                "Open it",
                DialogButtonKind::Primary,
                Some(msg(ProjectMsg::CreateChildSheetOpenExisting)),
                tokens,
            )
        } else {
            dialog_button("Create", DialogButtonKind::Primary, Some(create), tokens)
        };

        container(
            column![
                header,
                dialog_body(
                    column![
                        dialog_message(
                            format!(
                                "Sheet '{}' has no file yet. Where should it be created?",
                                confirm.name
                            ),
                            tokens,
                        ),
                        iced::widget::tooltip(
                            text(format!("Relative to {folder}"))
                                .size(TS.caption)
                                .color(text_muted),
                            text(full_folder).size(TS.caption),
                            iced::widget::tooltip::Position::Bottom,
                        ),
                        row![
                            field,
                            dialog_button(
                                "Browse...",
                                DialogButtonKind::Secondary,
                                Some(msg(ProjectMsg::CreateChildSheetBrowse)),
                                tokens,
                            ),
                        ]
                        .spacing(SP.m)
                        .align_y(iced::Alignment::Center),
                        note,
                    ]
                    .spacing(SP.m),
                ),
                dialog_footer(vec![
                    dialog_button("Later", DialogButtonKind::Secondary, Some(later), tokens),
                    confirm_button,
                ]),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
