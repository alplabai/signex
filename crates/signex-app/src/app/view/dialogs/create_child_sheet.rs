//! The "Create Sheet" dialog (#1275): where to write the file of a sheet
//! symbol whose file does not exist yet. A confirm-class dialog: it opens in
//! its own window and locks the main window (docs/DETACHED_MODALS.md).

use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_widgets::{raw, text, text_input};

use super::widgets::modal_title_header;
use super::*;
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer};

impl Signex {
    pub(in crate::app::view) fn view_create_child_sheet_confirm(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(
            self.view_create_child_sheet_body(),
            super::sizes::CREATE_CHILD_SHEET,
        )
    }

    /// The dialog body: the card in the main window, or the content of its
    /// own window (no header; the native title bar has the title).
    pub(in crate::app::view) fn view_create_child_sheet_body(&self) -> Element<'_, Message> {
        let Some(confirm) = self.ui_state.create_child_sheet_confirm.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);
        fn msg(m: ProjectMsg) -> Message {
            Message::Project(m)
        }
        let later = msg(ProjectMsg::CreateChildSheetAnswered { create: false });
        let create = msg(ProjectMsg::CreateChildSheetAnswered { create: true });

        let header_content: Element<'_, Message> =
            modal_title_header("Create Sheet", later.clone(), tokens);
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
        let field = text_input(raw("sheet.snxsch"), &confirm.path_text)
            .style(crate::panels::form_look::text_input_style)
            .on_input(|t| msg(ProjectMsg::CreateChildSheetPathChanged(t)))
            .on_submit(create.clone())
            .padding([6, 8])
            .size(TS.label)
            .width(Length::Fill);
        let note: Element<'_, Message> = match &confirm.error {
            Some(error) => text(raw(error.clone()))
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

        // In the main window the card is its design width and as tall as its
        // content; in its own window it fills that window.
        let modal = super::super::super::state::ModalId::CreateChildSheet;
        let width = self.dialog_width(modal, super::sizes::CREATE_CHILD_SHEET);
        let height = if self.modal_detached(modal) {
            Length::Fill
        } else {
            Length::Shrink
        };
        container(
            column![
                header,
                dialog_body(
                    column![
                        crate::styles::dialog_intro(
                            crate::styles::DialogIcon::File,
                            format!("Create a file for sheet '{}'", confirm.name),
                            Some(format!(
                                "Sheet '{}' has no file yet. Where should it be created?",
                                confirm.name
                            )),
                            tokens,
                        ),
                        signex_widgets::tooltip(
                            text(raw(format!("Relative to {folder}")))
                                .size(TS.caption)
                                .color(text_muted),
                            text(raw(full_folder)).size(TS.caption),
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
                    dialog_button("Later", DialogButtonKind::Cancel, Some(later), tokens),
                    confirm_button,
                ]),
            ]
            .width(Length::Fill)
            .height(height),
        )
        .width(width)
        .height(height)
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}
